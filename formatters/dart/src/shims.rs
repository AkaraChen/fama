//! Host-side implementations of the runtime helpers the dart2wasm-compiled
//! dart_style module imports.
//!
//! The module built from wasm-fmt/dart_fmt has two kinds of imports:
//!
//! 1. `dart2wasm::*` functions - the js-interop runtime helpers dart2wasm
//!    emits (property access on JS objects, closures wrapped for JS, string
//!    utilities not covered by js-string builtins, RegExp, stack traces...).
//!    Their single-letter names come from the loader the Dart SDK generates
//!    next to the module (`dart compile wasm`); the semantics here mirror the
//!    JavaScript implementations in that loader.
//! 2. `wasm:js-string::*` functions - the WebAssembly js-string builtins,
//!    implemented natively here over UTF-16 strings since wasmtime has no
//!    JS engine to provide them.
//!
//! Every JS value crossing the boundary is an `externref` wrapping a `JsVal`
//! payload. The wasm side also imports ~4200 constant strings as
//! `(ref extern)` globals whose import name is the string content; those are
//! defined by `lib.rs`.
//!
//! Helpers dart_style never reaches (typed-data DataView accessors, promises,
//! Math interop) are intentionally loud traps so a surprise shows up as a
//! formatting error instead of silently wrong output.

use std::collections::BTreeMap;

use wasmtime::{
	AsContext, AsContextMut, Caller, ExternRef, Func, IntoFunc, Linker, Rooted,
	Val, ValType,
};

use crate::jsval::{
	js_int_to_string_radix, js_number_to_string, JsObject, JsRegExp, JsString,
	JsVal,
};
use crate::StoreCtx;

/// Host-side per-thread wasm state: the `format` closure dart2wasm registers
/// during `main` plus every closure wrapped for JS.
pub struct HostState {
	/// Registry index of the closure assigned to `globalThis.format`
	pub format_fn: Option<usize>,
	/// id -> (trampoline funcref, Dart closure object backing the wrapper)
	pub registry: Vec<(Func, Option<wasmtime::OwnedRooted<ExternRef>>)>,
}

thread_local! {
	static HOST_STATE: std::cell::RefCell<HostState> = const {
		std::cell::RefCell::new(HostState {
			format_fn: None,
			registry: Vec::new(),
		})
	};
	static HOST_RANDOM: std::cell::Cell<u32> =
		const { std::cell::Cell::new(0x2545_F491) };
	// Cheap profiling counters (used only by the test latency benchmarks)
	#[cfg(test)]
	static HOST_STATS: std::cell::RefCell<BTreeMap<&'static str, (u64, u64)>> =
		const { std::cell::RefCell::new(BTreeMap::new()) };
}

/// Record a helper hit: name, call count, and total nanoseconds spent
#[cfg(test)]
pub fn profile(name: &'static str, _work: u64) {
	HOST_STATS.with(|s| {
		let mut s = s.borrow_mut();
		let entry = s.entry(name).or_insert((0, 0));
		entry.0 += 1;
		entry.1 += 0;
	});
}

#[cfg(not(test))]
#[inline(always)]
pub fn profile(_name: &'static str, _work: u64) {}

/// Snapshot of the recorded profile counters
#[cfg(test)]
pub fn profile_snapshot() -> Vec<(&'static str, u64, u64)> {
	HOST_STATS
		.with(|s| s.borrow().iter().map(|(k, v)| (*k, v.0, v.1)).collect())
}

/// Clear the profile counters
#[cfg(test)]
pub fn profile_reset() {
	HOST_STATS.with(|s| s.borrow_mut().clear());
}

/// Run `f` with the thread-local host state. A trap in a helper cannot poison
/// this cell: host helpers return `Err` instead of panicking.
pub fn with_host<R>(f: impl FnOnce(&mut HostState) -> R) -> R {
	HOST_STATE.with(|state| f(&mut state.borrow_mut()))
}

/// Reset per-instance state before a new instance is created (the registry
/// holds GC handles into the old store).
pub fn reset_host_state() {
	with_host(|state| {
		state.format_fn = None;
		state.registry.clear();
	});
}

/// The registry id of the `format` closure captured by the `M` helper
pub fn format_fn_id() -> Option<usize> {
	with_host(|state| state.format_fn)
}

/// Look up a closure registered by the CB/DB/L wrappers
pub fn registered_fn(
	id: usize,
) -> Option<(Func, Option<wasmtime::OwnedRooted<ExternRef>>)> {
	with_host(|state| state.registry.get(id).map(|(f, c)| (*f, c.clone())))
}

// ---------------------------------------------------------------------------
// Value plumbing
// ---------------------------------------------------------------------------

/// Borrow a host-backed `JsVal` behind an `externref` without cloning.
/// Returns `Ok(None)` when the externref is an `i31` (JS number) with no
/// host payload.
fn js_ref<'a>(
	ctx: &'a impl AsContext<Data = StoreCtx>,
	r: &Option<Rooted<ExternRef>>,
) -> wasmtime::Result<Option<&'a JsVal>> {
	let r = r.as_ref().ok_or_else(|| {
		wasmtime::Error::msg("dart_fmt: unexpected null externref")
	})?;
	let Some(data) = r.data(ctx.as_context())? else {
		return Ok(None);
	};
	data.downcast_ref::<JsVal>()
		.map(Some)
		.ok_or_else(|| wasmtime::Error::msg("dart_fmt: externref payload mismatch"))
}

/// Read the `JsVal` behind an `externref` parameter, by value.
///
/// dart2wasm passes JS-ready small values (numbers, booleans) as `i31`
/// references wrapped with `extern.convert_any`; in the JS embedding those
/// materialize as plain numbers, so they are surfaced here as
/// `JsVal::Number`.
fn js(
	ctx: &mut impl AsContextMut<Data = StoreCtx>,
	r: &Option<Rooted<ExternRef>>,
) -> wasmtime::Result<JsVal> {
	#[cfg(test)]
	let __t = std::time::Instant::now();
	let __out = (|| {
		let r = r.as_ref().ok_or_else(|| {
			#[cfg(debug_assertions)]
			if std::env::var_os("FAMA_DART_TRACE").is_some() {
				eprintln!(
					"dart_fmt: null externref at:\n{}",
					std::backtrace::Backtrace::force_capture()
				);
			}
			wasmtime::Error::msg("dart_fmt: unexpected null externref")
		})?;
		if let Some(data) = r.data(ctx.as_context())? {
			return data.downcast_ref::<JsVal>().cloned().ok_or_else(|| {
				wasmtime::Error::msg("dart_fmt: externref payload mismatch")
			});
		}
		// i31 externref: read the payload as a JS number
		let anyref = wasmtime::AnyRef::convert_extern(ctx.as_context_mut(), *r)
			.map_err(|e| {
				wasmtime::Error::msg(format!("dart_fmt: i31 read: {e}"))
			})?;
		let value = anyref
			.as_i31(ctx.as_context())?
			.map(|i| i.get_i32() as f64)
			.ok_or_else(|| {
				wasmtime::Error::msg("dart_fmt: externref has no payload")
			})?;
		Ok(JsVal::Number(value))
	})();
	#[cfg(test)]
	HOST_STATS.with(|s| {
		let mut s = s.borrow_mut();
		let e = s.entry("js.payload").or_insert((0, 0));
		e.0 += 1;
		e.1 += __t.elapsed().as_nanos() as u64;
	});
	__out
}

/// Mutable access to the payload (object/array/regex mutation helpers)
fn js_mut<'a>(
	ctx: &'a mut Caller<StoreCtx>,
	r: &Option<Rooted<ExternRef>>,
) -> wasmtime::Result<&'a mut JsVal> {
	let r = r.as_ref().ok_or_else(|| {
		wasmtime::Error::msg("dart_fmt: unexpected null externref")
	})?;
	let data = r.data_mut(ctx.as_context_mut())?.ok_or_else(|| {
		wasmtime::Error::msg("dart_fmt: unexpected i31 externref")
	})?;
	data.downcast_mut::<JsVal>().ok_or_else(|| {
		wasmtime::Error::msg("dart_fmt: externref payload mismatch")
	})
}

/// Clone a string-typed value (JS strings are primitives, so string helpers
/// hand out owned content)
fn string_of(
	ctx: &mut impl AsContextMut<Data = StoreCtx>,
	r: &Option<Rooted<ExternRef>>,
) -> wasmtime::Result<JsString> {
	match js(ctx, r)? {
		JsVal::String(s) => Ok(s),
		other => Err(wasmtime::Error::msg(format!(
			"dart_fmt: expected a JS string, found {other:?}"
		))),
	}
}

fn alloc(
	caller: &mut Caller<StoreCtx>,
	value: JsVal,
) -> wasmtime::Result<Rooted<ExternRef>> {
	ExternRef::new(caller.as_context_mut(), value).map_err(|e| {
		wasmtime::Error::msg(format!(
			"dart_fmt: failed to allocate externref: {e}"
		))
	})
}

fn alloc_string(
	caller: &mut Caller<StoreCtx>,
	s: JsString,
) -> wasmtime::Result<Rooted<ExternRef>> {
	alloc(caller, JsVal::String(s))
}

fn unimplemented(name: &str) -> wasmtime::Error {
	wasmtime::Error::msg(format!(
		"dart_fmt: reached unimplemented runtime helper `{name}`"
	))
}

/// Wrap a bool as a JS boolean externref
fn js_bool(
	caller: &mut Caller<StoreCtx>,
	b: bool,
) -> wasmtime::Result<Rooted<ExternRef>> {
	alloc(caller, JsVal::Bool(b))
}

/// JS object property get used by the array/string/object indexing helpers
fn index_get(value: &JsVal, index: i32) -> JsVal {
	match value {
		JsVal::Array(items) => items
			.get(index as usize)
			.cloned()
			.unwrap_or(JsVal::Undefined),
		JsVal::String(s) => match s.char_at(index as usize) {
			Some(unit) => JsVal::String(JsString::from_units(vec![unit])),
			None => JsVal::Undefined,
		},
		JsVal::Object(o) => o.get(&index.to_string()),
		_ => JsVal::Undefined,
	}
}

/// Byte offset -> UTF-16 code-unit offset (RegExp match indices are UTF-16)
fn byte_to_utf16(s: &str, byte: usize) -> usize {
	s[..byte].encode_utf16().count()
}

/// UTF-16 code-unit offset -> byte offset
fn utf16_to_byte(s: &str, units: usize) -> usize {
	let mut count = 0usize;
	for (byte, c) in s.char_indices() {
		if count >= units {
			return byte;
		}
		count += c.len_utf16();
	}
	s.len()
}

// ---------------------------------------------------------------------------
// RegExp helpers
// ---------------------------------------------------------------------------

/// JS `RegExp.prototype.exec`: match object with numeric group keys, `index`,
/// `input` and `length`, honoring `lastIndex` for global/sticky regexes
fn regexp_exec(reg: &mut JsRegExp, subject: &JsString) -> JsVal {
	let hay = subject.to_rust();
	let stateful = reg.global || reg.sticky;

	let start_utf16 = if stateful { reg.last_index.max(0) } else { 0 };
	if start_utf16 as usize > subject.len() {
		if reg.global {
			reg.last_index = 0;
		}
		return JsVal::Null;
	}
	let start_byte = utf16_to_byte(&hay, start_utf16 as usize);

	let caps = reg.re.captures_from_pos(&hay, start_byte).ok().flatten();
	let caps = if reg.sticky {
		caps.filter(|c| {
			c.get(0).map(|m| m.start()).unwrap_or(usize::MAX) == start_byte
		})
	} else {
		caps
	};

	let Some(caps) = caps else {
		if reg.global {
			reg.last_index = 0;
		}
		return JsVal::Null;
	};

	let whole = caps.get(0).unwrap();
	let mut fields = BTreeMap::new();
	fields.insert("0".to_string(), JsVal::string(whole.as_str()));
	for (i, group) in caps.iter().enumerate().skip(1) {
		let value = group
			.map(|g| JsVal::string(g.as_str()))
			.unwrap_or(JsVal::Undefined);
		fields.insert(i.to_string(), value);
	}
	fields.insert(
		"index".to_string(),
		JsVal::Number(byte_to_utf16(&hay, whole.start()) as f64),
	);
	fields.insert("input".to_string(), JsVal::String(subject.clone()));
	fields.insert("length".to_string(), JsVal::Number(caps.len() as f64));
	if stateful {
		reg.last_index = byte_to_utf16(&hay, whole.end()) as i32;
	}
	JsVal::Object(JsObject { fields })
}

/// JS `RegExp.prototype.test`, honoring `lastIndex` for global/sticky regexes
fn regexp_test(reg: &mut JsRegExp, subject: &JsString) -> bool {
	let hay = subject.to_rust();
	let stateful = reg.global || reg.sticky;

	let start_utf16 = if stateful { reg.last_index.max(0) } else { 0 };
	if start_utf16 as usize > subject.len() {
		if reg.global {
			reg.last_index = 0;
		}
		return false;
	}
	let start_byte = utf16_to_byte(&hay, start_utf16 as usize);

	let caps = reg.re.captures_from_pos(&hay, start_byte).ok().flatten();
	let at_start = caps
		.as_ref()
		.map(|c| {
			c.get(0).map(|m| m.start()).unwrap_or(usize::MAX) == start_byte
		})
		.unwrap_or(false);
	let matched = if reg.sticky { at_start } else { caps.is_some() };
	if matched {
		let end = caps
			.as_ref()
			.and_then(|c| c.get(0))
			.map(|m| m.end())
			.unwrap_or(start_byte);
		reg.last_index = byte_to_utf16(&hay, end) as i32;
	} else if reg.global {
		reg.last_index = 0;
	}
	matched
}

/// Run `f` with mutable access to the RegExp inside an externref plus the
/// subject string, writing the (possibly advanced) `lastIndex` back
fn with_regex<R>(
	caller: &mut Caller<StoreCtx>,
	regex: &Option<Rooted<ExternRef>>,
	subject: &Option<Rooted<ExternRef>>,
	f: impl FnOnce(&mut JsRegExp, &JsString) -> wasmtime::Result<R>,
) -> wasmtime::Result<R> {
	let subject = string_of(caller, subject)?;
	let mut reg = match js(caller, regex)? {
		JsVal::RegExp(reg) => reg,
		other => {
			return Err(wasmtime::Error::msg(format!(
				"dart_fmt: expected RegExp, found {other:?}"
			)))
		}
	};
	let result = f(&mut reg, &subject)?;
	match js_mut(caller, regex)? {
		JsVal::RegExp(slot) => *slot = reg,
		_ => {
			return Err(wasmtime::Error::msg(
				"dart_fmt: RegExp replaced mid-call",
			))
		}
	}
	Ok(result)
}

/// Read-only flag getter on a RegExp returning a JS boolean
fn regex_flag_value(reg: &JsRegExp, flag: &str) -> bool {
	match flag {
		"unicode" => reg.flags.contains('u'),
		"dotAll" => reg.flags.contains('s'),
		"ignoreCase" => reg.flags.contains('i'),
		"multiline" => reg.flags.contains('m'),
		_ => false,
	}
}

/// Build a property setter (`x0.<field> = x1`) for the FormatResult fields
// the Fn signature is unavoidably wide; a `dyn` alias can't name an `impl` bound
#[allow(clippy::type_complexity)]
fn set_property(
	field: &'static str,
) -> impl Fn(
	Caller<StoreCtx>,
	Option<Rooted<ExternRef>>,
	Option<Rooted<ExternRef>>,
) -> wasmtime::Result<()> {
	move |mut c: Caller<StoreCtx>,
	      obj: Option<Rooted<ExternRef>>,
	      value: Option<Rooted<ExternRef>>| {
		let value = js(&mut c, &value)?;
		match js_mut(&mut c, &obj)? {
			JsVal::Object(o) => {
				o.fields.insert(field.to_string(), value);
				Ok(())
			}
			other => Err(wasmtime::Error::msg(format!(
				"dart_fmt: property `{field}` set on non-object {other:?}"
			))),
		}
	}
}

/// SameValue identity for reference-typed values: two externrefs pointing at
/// the same GC object share one host payload allocation; i31 externrefs are
/// JS numbers and compare by value
fn externref_identical(
	c: &mut Caller<StoreCtx>,
	a: &Option<Rooted<ExternRef>>,
	b: &Option<Rooted<ExternRef>>,
) -> bool {
	let (Some(a), Some(b)) = (a.as_ref(), b.as_ref()) else {
		return a.is_none() && b.is_none();
	};
	let da = a.data::<crate::StoreCtx>(c.as_context());
	let db = b.data::<crate::StoreCtx>(c.as_context());
	match (da, db) {
		(Ok(Some(x)), Ok(Some(y))) => std::ptr::eq(x, y),
		// both without host payload: i31 refs (or anyref-backed values) -
		// compare their numeric i31 payloads
		(Ok(None), Ok(None)) => {
			let ia = wasmtime::AnyRef::convert_extern(c.as_context_mut(), *a)
				.ok()
				.and_then(|r| r.as_i31(c.as_context()).ok().flatten());
			let ib = wasmtime::AnyRef::convert_extern(c.as_context_mut(), *b)
				.ok()
				.and_then(|r| r.as_i31(c.as_context()).ok().flatten());
			match (ia, ib) {
				(Some(x), Some(y)) => x.get_u32() == y.get_u32(),
				_ => false,
			}
		}
		_ => false,
	}
}

/// JS `Object.is`
fn object_is_impl(
	mut c: Caller<StoreCtx>,
	a: Option<Rooted<ExternRef>>,
	b: Option<Rooted<ExternRef>>,
) -> wasmtime::Result<i32> {
	let va = js(&mut c, &a)?;
	let vb = js(&mut c, &b)?;
	Ok(match (va, vb) {
		(JsVal::String(x), JsVal::String(y)) => x.equals(&y) as i32,
		(JsVal::Number(x), JsVal::Number(y)) => {
			(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())) as i32
		}
		(JsVal::Bool(x), JsVal::Bool(y)) => (x == y) as i32,
		(JsVal::Undefined, JsVal::Undefined) | (JsVal::Null, JsVal::Null) => 1,
		_ => externref_identical(&mut c, &a, &b) as i32,
	})
}

/// JS `===`: unlike Object.is, strings compare by value and NaN === NaN is
/// false
fn strict_equals_impl(
	mut c: Caller<StoreCtx>,
	a: Option<Rooted<ExternRef>>,
	b: Option<Rooted<ExternRef>>,
) -> wasmtime::Result<i32> {
	let va = js(&mut c, &a)?;
	let vb = js(&mut c, &b)?;
	Ok(match (va, vb) {
		(JsVal::String(x), JsVal::String(y)) => x.equals(&y) as i32,
		(JsVal::Number(x), JsVal::Number(y)) => {
			(x.to_bits() == y.to_bits()) as i32
		}
		(JsVal::Bool(x), JsVal::Bool(y)) => (x == y) as i32,
		(JsVal::Undefined, JsVal::Undefined) | (JsVal::Null, JsVal::Null) => 1,
		(JsVal::Function(x), JsVal::Function(y)) => (x == y) as i32,
		_ => externref_identical(&mut c, &a, &b) as i32,
	})
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

fn reg<P, R>(
	linker: &mut Linker<StoreCtx>,
	module: &str,
	name: &str,
	f: impl IntoFunc<StoreCtx, P, R>,
) -> Result<(), String> {
	linker
		.func_wrap(module, name, f)
		.map(|_| ())
		.map_err(|e| format!("Failed to define {module}::{name}: {e}"))
}

/// Register every imported helper on the linker
pub fn define(
	linker: &mut Linker<StoreCtx>,
	engine: &wasmtime::Engine,
) -> Result<(), String> {
	define_dart2wasm(linker)?;
	define_js_string(linker, engine)?;
	Ok(())
}

fn define_dart2wasm(linker: &mut Linker<StoreCtx>) -> Result<(), String> {
	// B: printToConsole - dart_style never prints on the format path
	reg(
		linker,
		"dart2wasm",
		"B",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			if let Ok(value) = js(&mut c, &v) {
				eprintln!("dart_fmt: {}", value.to_js_string().to_rust());
			} else {
				eprintln!("dart_fmt: <unprintable>");
			}
		},
	)?;

	// C/D/H: Number.prototype.toString with and without radix
	reg(
		linker,
		"dart2wasm",
		"C",
		|mut c: Caller<StoreCtx>, v: i32, radix: i32| {
			alloc_string(
				&mut c,
				JsString::from_rust(&js_int_to_string_radix(v as i64, radix)),
			)
			.map(Some)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"D",
		|mut c: Caller<StoreCtx>, v: i64, radix: i32| {
			alloc_string(
				&mut c,
				JsString::from_rust(&js_int_to_string_radix(v, radix)),
			)
			.map(Some)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"H",
		|mut c: Caller<StoreCtx>, v: f64| {
			alloc_string(&mut c, JsString::from_rust(&js_number_to_string(v)))
				.map(Some)
		},
	)?;

	// E: drop the JS engine's leading frames from an error stack string
	reg(
		linker,
		"dart2wasm",
		"E",
		|mut c: Caller<StoreCtx>, stack: Option<Rooted<ExternRef>>| {
			let raw = match js(&mut c, &stack)? {
				JsVal::String(s) => s.to_rust(),
				_ => String::new(),
			};
			let mut lines: Vec<String> =
				raw.split('\n').map(String::from).collect();
			let drop = if lines.first().is_some_and(|f| f.starts_with("Error"))
			{
				5
			} else {
				4
			};
			if lines.len() > drop {
				lines.drain(0..drop);
			} else {
				lines.clear();
			}
			let frames = lines.join("\n");
			alloc_string(&mut c, JsString::from_rust(&frames)).map(Some)
		},
	)?;

	// F: new Error().stack - stack traces have no host meaning here
	reg(linker, "dart2wasm", "F", |mut c: Caller<StoreCtx>| {
		alloc_string(&mut c, JsString::default()).map(Some)
	})?;

	// G: JSON.stringify
	reg(
		linker,
		"dart2wasm",
		"G",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			let value = js(&mut c, &v)?.to_json();
			alloc_string(&mut c, value)
		},
	)?;

	// I: String.prototype.indexOf
	reg(
		linker,
		"dart2wasm",
		"I",
		|mut c: Caller<StoreCtx>,
		 haystack: Option<Rooted<ExternRef>>,
		 token: Option<Rooted<ExternRef>>,
		 from: i32| {
			profile("helper.I", 0);
			let hay = string_of(&mut c, &haystack)?;
			let token = string_of(&mut c, &token)?;
			Ok(hay.index_of(&token, from) as f64)
		},
	)?;

	// J: exn.stack for JS errors - no JS exceptions exist in this host
	reg(
		linker,
		"dart2wasm",
		"J",
		|_c: Caller<StoreCtx>, _exn: Option<Rooted<ExternRef>>| {
			Err::<Option<Rooted<ExternRef>>, _>(unimplemented("J"))
		},
	)?;

	// K: o === undefined
	reg(
		linker,
		"dart2wasm",
		"K",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(matches!(js(&mut c, &v)?, JsVal::Undefined) as i32)
		},
	)?;

	// L: wrap a 3-arg Dart closure for JS calls; the trampoline funcref is
	// invoked later as (closure, argc, arg0, arg1, arg2)
	reg(
		linker,
		"dart2wasm",
		"L",
		|mut c: Caller<StoreCtx>,
		 trampoline: Func,
		 closure: Rooted<ExternRef>| {
			wrap_with_arity(&mut c, trampoline, closure)
		},
	)?;

	// M: globalThis.format = x0 - capture the format closure
	reg(
		linker,
		"dart2wasm",
		"M",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			let id = match js(&mut c, &v)? {
				JsVal::Function(id) => id,
				other => {
					return Err(wasmtime::Error::msg(format!(
					"dart_fmt: globalThis.format set to non-function {other:?}"
				)))
				}
			};
			with_host(|state| state.format_fn = Some(id));
			Ok(())
		},
	)?;

	// N: new Object() - the FormatResult instance
	reg(linker, "dart2wasm", "N", |mut c: Caller<StoreCtx>| {
		alloc(&mut c, JsVal::Object(JsObject::default())).map(Some)
	})?;

	// O/P/y: FormatResult field assignment
	reg(linker, "dart2wasm", "O", set_property("error"))?;
	reg(linker, "dart2wasm", "P", set_property("code"))?;
	reg(linker, "dart2wasm", "y", set_property("success"))?;

	// Q: String.prototype.repeat
	reg(
		linker,
		"dart2wasm",
		"Q",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>, n: i32| {
			profile("helper.Q", 0);
			let s = string_of(&mut c, &s)?;
			alloc_string(&mut c, s.repeat(n)).map(Some)
		},
	)?;

	// U: globalThis.Math, R: Math.random() - reached via dart:math usage in
	// the analyzer's parser; the value is irrelevant to formatting
	reg(linker, "dart2wasm", "U", |mut c: Caller<StoreCtx>| {
		let mut fields = std::collections::BTreeMap::new();
		fields.insert("__math__".to_string(), JsVal::Bool(true));
		alloc(&mut c, JsVal::Object(JsObject { fields })).map(Some)
	})?;
	reg(
		linker,
		"dart2wasm",
		"R",
		|mut c: Caller<StoreCtx>, m: Option<Rooted<ExternRef>>| {
			let is_math = matches!(js(&mut c, &m)?, JsVal::Object(_));
			if !is_math {
				return Err(wasmtime::Error::msg(
					"dart_fmt: Math.random on non-Math receiver",
				));
			}
			// Cheap xorshift; JS Math.random semantics are unspecified anyway
			let mut state = HOST_RANDOM.with(|r| r.get());
			state ^= state << 13;
			state ^= state >> 17;
			state ^= state << 5;
			HOST_RANDOM.with(|r| r.set(state));
			let value = (state & 0x001f_ffff) as f64 / 0x0020_0000 as f64;
			alloc(&mut c, JsVal::Number(value)).map(Some)
		},
	)?;

	// S: JS number value -> f64
	reg(
		linker,
		"dart2wasm",
		"S",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| match js(
			&mut c, &v,
		)? {
			JsVal::Number(n) => Ok(n),
			JsVal::Bool(b) => Ok(b as i32 as f64),
			other => Err(wasmtime::Error::msg(format!(
				"dart_fmt: expected JS number, found {other:?}"
			))),
		},
	)?;

	// T/X: JS number/boolean type discriminators
	reg(
		linker,
		"dart2wasm",
		"T",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(match js(&mut c, &v)? {
				JsVal::Undefined | JsVal::Null => 0,
				JsVal::Number(_) => 1,
				_ => 2,
			})
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"X",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(match js(&mut c, &v)? {
				JsVal::Undefined | JsVal::Null => 0,
				JsVal::Bool(_) => 1,
				_ => 2,
			})
		},
	)?;

	// V: RegExp.prototype.test
	reg(
		linker,
		"dart2wasm",
		"V",
		|mut c: Caller<StoreCtx>,
		 regex: Option<Rooted<ExternRef>>,
		 subject: Option<Rooted<ExternRef>>| {
			let matched =
				with_regex(&mut c, &regex, &subject, |reg, subject| {
					Ok(regexp_test(reg, subject))
				})?;
			js_bool(&mut c, matched).map(Some)
		},
	)?;

	// W: JS number/bool value -> i32
	reg(
		linker,
		"dart2wasm",
		"W",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(match js(&mut c, &v)? {
				JsVal::Number(n) => n as i32,
				JsVal::Bool(b) => b as i32,
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: expected JS number/bool, found {other:?}"
					)))
				}
			})
		},
	)?;

	// Y/e/f/g: RegExp flag getters (unicode/dotAll/ignoreCase/multiline)
	reg(linker, "dart2wasm", "Y", regex_flag_getter("unicode"))?;
	reg(linker, "dart2wasm", "e", regex_flag_getter("dotAll"))?;
	reg(linker, "dart2wasm", "f", regex_flag_getter("ignoreCase"))?;
	reg(linker, "dart2wasm", "g", regex_flag_getter("multiline"))?;

	// h: RegExp flags string
	reg(
		linker,
		"dart2wasm",
		"h",
		|mut c: Caller<StoreCtx>, r: Option<Rooted<ExternRef>>| {
			let flags = match js(&mut c, &r)? {
				JsVal::RegExp(reg) => reg.flags.clone(),
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: expected RegExp, found {other:?}"
					)))
				}
			};
			alloc_string(&mut c, JsString::from_rust(&flags)).map(Some)
		},
	)?;

	// Z: match.index
	reg(
		linker,
		"dart2wasm",
		"Z",
		|mut c: Caller<StoreCtx>, m: Option<Rooted<ExternRef>>| {
			let index = match js(&mut c, &m)? {
				JsVal::Object(o) => o.get("index"),
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: expected match object, found {other:?}"
					)))
				}
			};
			alloc(&mut c, index).map(Some)
		},
	)?;

	// ZB/cB: o[i] element access on arrays, strings and objects
	reg(
		linker,
		"dart2wasm",
		"ZB",
		|mut c: Caller<StoreCtx>, o: Option<Rooted<ExternRef>>, i: i32| {
			profile("helper.ZB", 0);
			let value = index_get(&js(&mut c, &o)?, i);
			alloc(&mut c, value).map(Some)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"cB",
		|mut c: Caller<StoreCtx>, o: Option<Rooted<ExternRef>>, i: i32| {
			profile("helper.cB", 0);
			let value = index_get(&js(&mut c, &o)?, i);
			alloc(&mut c, value).map(Some)
		},
	)?;

	// a: o[p] property access by string key (JS also allows numeric keys,
	// which dart2wasm passes as i31-backed numbers)
	reg(
		linker,
		"dart2wasm",
		"a",
		|mut c: Caller<StoreCtx>,
		 o: Option<Rooted<ExternRef>>,
		 key: Option<Rooted<ExternRef>>| {
			profile("helper.a", 0);
			let key_val = js(&mut c, &key)?;
			let key_str = match &key_val {
				JsVal::String(s) => s.to_rust(),
				JsVal::Number(n) => {
					// Numeric property access: JS stringifies the key
					if n.fract() == 0.0 && *n >= 0.0 {
						format!("{}", *n as i64)
					} else {
						n.to_string()
					}
				}
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: property get with key {other:?}"
					)))
				}
			};
			let value = match js(&mut c, &o)? {
				JsVal::Object(o) => o.get(&key_str),
				JsVal::Array(items) => {
					if key_str == "length" {
						JsVal::Number(items.len() as f64)
					} else if let Ok(index) = key_str.parse::<usize>() {
						items.get(index).cloned().unwrap_or(JsVal::Undefined)
					} else {
						JsVal::Undefined
					}
				}
				JsVal::String(s) => {
					if key_str == "length" {
						JsVal::Number(s.len() as f64)
					} else if let Ok(index) = key_str.parse::<usize>() {
						match s.char_at(index) {
							Some(unit) => {
								JsVal::String(JsString::from_units(vec![unit]))
							}
							None => JsVal::Undefined,
						}
					} else {
						JsVal::Undefined
					}
				}
				_ => JsVal::Undefined,
			};
			alloc(&mut c, value).map(Some)
		},
	)?;

	// b: f64 -> JS number
	reg(
		linker,
		"dart2wasm",
		"b",
		|mut c: Caller<StoreCtx>, v: f64| {
			alloc(&mut c, JsVal::Number(v)).map(Some)
		},
	)?;

	// c: RegExp.prototype.exec
	reg(
		linker,
		"dart2wasm",
		"c",
		|mut c: Caller<StoreCtx>,
		 regex: Option<Rooted<ExternRef>>,
		 subject: Option<Rooted<ExternRef>>| {
			let result =
				with_regex(&mut c, &regex, &subject, |reg, subject| {
					Ok(regexp_exec(reg, subject))
				})?;
			alloc(&mut c, result).map(Some)
		},
	)?;

	// d: regex.lastIndex = v
	reg(
		linker,
		"dart2wasm",
		"d",
		|mut c: Caller<StoreCtx>,
		 regex: Option<Rooted<ExternRef>>,
		 value: Option<Rooted<ExternRef>>| {
			let index = match js(&mut c, &value)? {
				JsVal::Number(n) => n as i32,
				JsVal::Undefined => 0,
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: lastIndex = {other:?}"
					)))
				}
			};
			match js_mut(&mut c, &regex)? {
				JsVal::RegExp(reg) => {
					reg.last_index = index;
					Ok(())
				}
				other => Err(wasmtime::Error::msg(format!(
					"dart_fmt: lastIndex set on non-RegExp {other:?}"
				))),
			}
		},
	)?;

	// i: new RegExp(pattern, flags), errors surface as strings like in JS
	reg(
		linker,
		"dart2wasm",
		"i",
		|mut c: Caller<StoreCtx>,
		 source: Option<Rooted<ExternRef>>,
		 flags: Option<Rooted<ExternRef>>| {
			let source = string_of(&mut c, &source)?.to_rust();
			let flags = string_of(&mut c, &flags)?.to_rust();
			match JsRegExp::new(&source, &flags) {
				Ok(reg) => alloc(&mut c, JsVal::RegExp(reg)).map(Some),
				Err(e) => {
					alloc_string(&mut c, JsString::from_rust(&e)).map(Some)
				}
			}
		},
	)?;

	// j: o instanceof RegExp
	reg(
		linker,
		"dart2wasm",
		"j",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(matches!(js(&mut c, &v)?, JsVal::RegExp(_)) as i32)
		},
	)?;

	// k: array.pop()
	reg(
		linker,
		"dart2wasm",
		"k",
		|mut c: Caller<StoreCtx>, a: Option<Rooted<ExternRef>>| {
			let popped = match js_mut(&mut c, &a)? {
				JsVal::Array(items) => items.pop().unwrap_or(JsVal::Undefined),
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: pop on non-array {other:?}"
					)))
				}
			};
			alloc(&mut c, popped).map(Some)
		},
	)?;

	// l/m/u: String.prototype.trim/toUpperCase/toLowerCase
	reg(
		linker,
		"dart2wasm",
		"l",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("helper.l", 0);
			let s = string_of(&mut c, &s)?;
			alloc_string(&mut c, s.trim()).map(Some)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"m",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("helper.m", 0);
			let s = string_of(&mut c, &s)?;
			alloc_string(&mut c, s.to_upper()).map(Some)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"u",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("helper.u", 0);
			let s = string_of(&mut c, &s)?;
			alloc_string(&mut c, s.to_lower()).map(Some)
		},
	)?;

	// n: Object.is / HB: strict equality
	reg(linker, "dart2wasm", "n", object_is_impl)?;
	reg(linker, "dart2wasm", "HB", strict_equals_impl)?;

	// o: String.prototype.replaceAll with a function replacement (no $-patterns)
	reg(
		linker,
		"dart2wasm",
		"o",
		|mut c: Caller<StoreCtx>,
		 s: Option<Rooted<ExternRef>>,
		 from: Option<Rooted<ExternRef>>,
		 to: Option<Rooted<ExternRef>>| {
			profile("helper.o", 0);
			let s = string_of(&mut c, &s)?;
			let from = string_of(&mut c, &from)?;
			let to = string_of(&mut c, &to)?;
			alloc_string(&mut c, s.replace_all(&from, &to)).map(Some)
		},
	)?;

	// p: o.length read as a JS value
	reg(
		linker,
		"dart2wasm",
		"p",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			profile("helper.p", 0);
			let value = match js(&mut c, &v)? {
				JsVal::Array(items) => JsVal::Number(items.len() as f64),
				JsVal::String(s) => JsVal::Number(s.len() as f64),
				JsVal::Object(o) => o.get("length"),
				_ => JsVal::Undefined,
			};
			alloc(&mut c, value).map(Some)
		},
	)?;

	// q: String.prototype.split
	reg(
		linker,
		"dart2wasm",
		"q",
		|mut c: Caller<StoreCtx>,
		 s: Option<Rooted<ExternRef>>,
		 token: Option<Rooted<ExternRef>>| {
			profile("helper.q", 0);
			let s = string_of(&mut c, &s)?;
			let token = string_of(&mut c, &token)?;
			let parts: Vec<JsVal> =
				s.split(&token).into_iter().map(JsVal::String).collect();
			alloc(&mut c, JsVal::Array(parts)).map(Some)
		},
	)?;

	// r: String.prototype.lastIndexOf
	reg(
		linker,
		"dart2wasm",
		"r",
		|mut c: Caller<StoreCtx>,
		 haystack: Option<Rooted<ExternRef>>,
		 token: Option<Rooted<ExternRef>>,
		 from: f64| {
			profile("helper.r", 0);
			let hay = string_of(&mut c, &haystack)?;
			let token = string_of(&mut c, &token)?;
			Ok(hay.last_index_of(&token, from as i32) as f64)
		},
	)?;

	// s: String(value)
	reg(
		linker,
		"dart2wasm",
		"s",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			profile("helper.s", 0);
			let s = js(&mut c, &v)?.to_js_string();
			alloc_string(&mut c, s).map(Some)
		},
	)?;

	// t: running on Windows -> false; v: location.href -> null
	reg(linker, "dart2wasm", "t", |_c: Caller<StoreCtx>| Ok(0))?;
	reg(linker, "dart2wasm", "v", |_c: Caller<StoreCtx>| {
		Ok(None::<Rooted<ExternRef>>)
	})?;

	// w: parseFloat / z: unary +s
	reg(
		linker,
		"dart2wasm",
		"w",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("helper.w", 0);
			match js(&mut c, &s)? {
				JsVal::String(s) => Ok(s.parse_float()),
				JsVal::Number(n) => Ok(n),
				_ => Ok(f64::NAN),
			}
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"z",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("helper.z", 0);
			match js(&mut c, &s)? {
				JsVal::String(s) => Ok(s.to_number()),
				JsVal::Number(n) => Ok(n),
				JsVal::Bool(b) => Ok(b as i32 as f64),
				JsVal::Null => Ok(0.0),
				_ => Ok(f64::NAN),
			}
		},
	)?;

	// x: i32 -> JS boolean
	reg(
		linker,
		"dart2wasm",
		"x",
		|mut c: Caller<StoreCtx>, v: i32| {
			alloc(&mut c, JsVal::Bool(v != 0)).map(Some)
		},
	)?;

	// AB: is a JS-wrapped Dart function
	reg(
		linker,
		"dart2wasm",
		"AB",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(matches!(js(&mut c, &v)?, JsVal::Function(_)) as i32)
		},
	)?;

	// BB/CB/DB/EB/FB: wrapped-function introspection, 1/2-arg wrappers,
	// promises and microtasks - unreachable for dart_style
	reg(
		linker,
		"dart2wasm",
		"BB",
		|_c: Caller<StoreCtx>, _v: Option<Rooted<ExternRef>>| {
			Err::<Option<Rooted<wasmtime::AnyRef>>, _>(unimplemented("BB"))
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"CB",
		|mut c: Caller<StoreCtx>,
		 trampoline: Func,
		 closure: Rooted<ExternRef>| {
			wrap_with_arity(&mut c, trampoline, closure)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"DB",
		|mut c: Caller<StoreCtx>,
		 trampoline: Func,
		 closure: Rooted<ExternRef>| {
			wrap_with_arity(&mut c, trampoline, closure)
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"EB",
		|_c: Caller<StoreCtx>,
		 _a: Option<Rooted<ExternRef>>,
		 _b: Option<Rooted<ExternRef>>,
		 _d: Option<Rooted<ExternRef>>| {
			Err::<(), _>(unimplemented("EB"))
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"FB",
		|_c: Caller<StoreCtx>, _cb: Option<Rooted<wasmtime::AnyRef>>| {
			Err::<(), _>(unimplemented("FB"))
		},
	)?;

	// IB + GB..YB: DataView construction and accessors (dart:typed_data) -
	// dart_style never touches typed data
	reg(
		linker,
		"dart2wasm",
		"IB",
		|_c: Caller<StoreCtx>, _v: Option<Rooted<ExternRef>>| {
			Err::<Option<Rooted<ExternRef>>, _>(unimplemented("IB"))
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"GB",
		|_c: Caller<StoreCtx>, _v: Option<Rooted<ExternRef>>| {
			Err::<f64, _>(unimplemented("GB"))
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"JB",
		|_c: Caller<StoreCtx>,
		 _v: Option<Rooted<ExternRef>>,
		 _a: i32,
		 _b: i32| { Err::<f64, _>(unimplemented("JB")) },
	)?;
	reg(
		linker,
		"dart2wasm",
		"KB",
		|_c: Caller<StoreCtx>,
		 _v: Option<Rooted<ExternRef>>,
		 _a: i32,
		 _b: f64,
		 _d: i32| { Err::<(), _>(unimplemented("KB")) },
	)?;
	reg(
		linker,
		"dart2wasm",
		"LB",
		|_c: Caller<StoreCtx>,
		 _v: Option<Rooted<ExternRef>>,
		 _a: i32,
		 _b: i32| { Err::<f32, _>(unimplemented("LB")) },
	)?;
	reg(
		linker,
		"dart2wasm",
		"MB",
		|_c: Caller<StoreCtx>,
		 _v: Option<Rooted<ExternRef>>,
		 _a: i32,
		 _b: f32,
		 _d: i32| { Err::<(), _>(unimplemented("MB")) },
	)?;
	for name in ["NB", "PB", "RB", "TB"] {
		reg(
			linker,
			"dart2wasm",
			name,
			|_c: Caller<StoreCtx>,
			 _v: Option<Rooted<ExternRef>>,
			 _a: i32,
			 _b: i32| { Err::<i32, _>(unimplemented(name)) },
		)?;
	}
	for name in ["OB", "QB", "SB", "UB"] {
		reg(
			linker,
			"dart2wasm",
			name,
			|_c: Caller<StoreCtx>,
			 _v: Option<Rooted<ExternRef>>,
			 _a: i32,
			 _b: i32,
			 _d: i32| { Err::<(), _>(unimplemented(name)) },
		)?;
	}
	// 8-bit setters take no little-endian flag
	for name in ["WB", "YB"] {
		reg(
			linker,
			"dart2wasm",
			name,
			|_c: Caller<StoreCtx>,
			 _v: Option<Rooted<ExternRef>>,
			 _a: i32,
			 _b: i32| { Err::<(), _>(unimplemented(name)) },
		)?;
	}
	reg(
		linker,
		"dart2wasm",
		"VB",
		|_c: Caller<StoreCtx>, _v: Option<Rooted<ExternRef>>, _a: i32| {
			Err::<i32, _>(unimplemented("VB"))
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"XB",
		|_c: Caller<StoreCtx>, _v: Option<Rooted<ExternRef>>, _a: i32| {
			Err::<i32, _>(unimplemented("XB"))
		},
	)?;

	// aB: o.length as an int; dB: array length as a double
	reg(
		linker,
		"dart2wasm",
		"aB",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			profile("helper.aB", 0);
			Ok(match js(&mut c, &v)? {
				JsVal::Array(items) => items.len() as i32,
				JsVal::String(s) => s.len() as i32,
				JsVal::Object(o) => match o.get("length") {
					JsVal::Number(n) => n as i32,
					_ => 0,
				},
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: length of {other:?}"
					)))
				}
			})
		},
	)?;
	reg(
		linker,
		"dart2wasm",
		"dB",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			profile("helper.dB", 0);
			Ok(match js(&mut c, &v)? {
				JsVal::Array(items) => items.len() as f64,
				JsVal::String(s) => s.len() as f64,
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: f64 length of {other:?}"
					)))
				}
			})
		},
	)?;

	// bB: JS value type tag
	reg(
		linker,
		"dart2wasm",
		"bB",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(js(&mut c, &v)?.type_tag())
		},
	)?;

	// eB: o instanceof Array
	reg(
		linker,
		"dart2wasm",
		"eB",
		|mut c: Caller<StoreCtx>, v: Option<Rooted<ExternRef>>| {
			Ok(matches!(js(&mut c, &v)?, JsVal::Array(_)) as i32)
		},
	)?;

	// fB: a[i] = v
	reg(
		linker,
		"dart2wasm",
		"fB",
		|mut c: Caller<StoreCtx>,
		 a: Option<Rooted<ExternRef>>,
		 i: i32,
		 v: Option<Rooted<ExternRef>>| {
			let value = js(&mut c, &v)?;
			match js_mut(&mut c, &a)? {
				JsVal::Array(items) => {
					let idx = i.max(0) as usize;
					if idx >= items.len() {
						items.resize(idx + 1, JsVal::Undefined);
					}
					items[idx] = value;
					Ok(())
				}
				other => Err(wasmtime::Error::msg(format!(
					"dart_fmt: index set on non-array {other:?}"
				))),
			}
		},
	)?;

	// gB: a.push(v)
	reg(
		linker,
		"dart2wasm",
		"gB",
		|mut c: Caller<StoreCtx>,
		 a: Option<Rooted<ExternRef>>,
		 v: Option<Rooted<ExternRef>>| {
			let value = js(&mut c, &v)?;
			match js_mut(&mut c, &a)? {
				JsVal::Array(items) => {
					items.push(value);
					Ok(())
				}
				other => Err(wasmtime::Error::msg(format!(
					"dart_fmt: push on non-array {other:?}"
				))),
			}
		},
	)?;

	Ok(())
}

/// Shared body of the CB/DB/L closure wrappers (the wasm module only imports
/// L on the format path, but CB/DB come along in the runtime table)
fn wrap_with_arity(
	caller: &mut Caller<StoreCtx>,
	trampoline: Func,
	closure: Rooted<ExternRef>,
) -> wasmtime::Result<Rooted<ExternRef>> {
	let closure =
		closure
			.to_owned_rooted(caller.as_context_mut())
			.map_err(|e| {
				wasmtime::Error::msg(format!("dart_fmt: rooting closure: {e}"))
			})?;
	let id = with_host(|state| {
		state.registry.push((trampoline, Some(closure)));
		state.registry.len() - 1
	});
	alloc(caller, JsVal::Function(id))
}

fn regex_flag_getter(
	flag: &'static str,
) -> impl Fn(
	Caller<StoreCtx>,
	Option<Rooted<ExternRef>>,
) -> wasmtime::Result<Option<Rooted<ExternRef>>> {
	move |mut c: Caller<StoreCtx>, r: Option<Rooted<ExternRef>>| {
		let value = match js(&mut c, &r)? {
			JsVal::RegExp(reg) => regex_flag_value(&reg, flag),
			other => {
				return Err(wasmtime::Error::msg(format!(
					"dart_fmt: expected RegExp, found {other:?}"
				)))
			}
		};
		js_bool(&mut c, value).map(Some)
	}
}

fn define_js_string(
	linker: &mut Linker<StoreCtx>,
	engine: &wasmtime::Engine,
) -> Result<(), String> {
	// length: traps on non-strings per the js-string builtins spec
	reg(
		linker,
		"wasm:js-string",
		"length",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("js.length", 0);
			if s.is_none() {
				// dart2wasm reaches `length(null)` on defensive internal paths;
				// JS engines trap per spec but dart_style's output is unaffected
				// by treating the null string as empty here
				return Ok(0);
			}
			if let Some(JsVal::String(s)) = js_ref(&c, &s)? {
				return Ok(s.len() as i32);
			}
			let other = js(&mut c, &s)?;
			Err(wasmtime::Error::msg(format!(
				"dart_fmt: js-string length on non-string {other:?}"
			)))
		},
	)?;

	reg(
		linker,
		"wasm:js-string",
		"charCodeAt",
		|mut c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>, index: i32| {
			profile("js.charCodeAt", 0);
			if let Some(JsVal::String(s)) = js_ref(&c, &s)? {
				let index = index as u32 as usize;
				return if index >= s.len() {
					// The spec traps out-of-bounds; dart_style's own
					// bounds checks keep this unreachable in practice
					Err(wasmtime::Error::msg(format!(
						"dart_fmt: js-string charCodeAt out of bounds ({index} >= {})",
						s.len()
					)))
				} else {
					Ok(s.char_at(index).unwrap() as i32)
				};
			}
			let other = js(&mut c, &s)?;
			Err(wasmtime::Error::msg(format!(
				"dart_fmt: js-string charCodeAt on non-string {other:?}"
			)))
		},
	)?;

	// substring: clamped, swapped ranges per JS semantics
	reg(
		linker,
		"wasm:js-string",
		"substring",
		|mut c: Caller<StoreCtx>,
		 s: Option<Rooted<ExternRef>>,
		 start: i32,
		 end: i32| {
			profile("substring", 0);
			let s = string_of(&mut c, &s)?;
			let (start, end) = (start as u32 as i32, end as u32 as i32);
			if start > end || start as usize > s.len() {
				return alloc_string(&mut c, JsString::default());
			}
			alloc_string(&mut c, s.substring(start, end))
		},
	)?;

	reg(
		linker,
		"wasm:js-string",
		"concat",
		|mut c: Caller<StoreCtx>,
		 a: Option<Rooted<ExternRef>>,
		 b: Option<Rooted<ExternRef>>| {
			profile("concat", 0);
			let a = string_of(&mut c, &a)?;
			let b = string_of(&mut c, &b)?;
			alloc_string(&mut c, a.concat(&b))
		},
	)?;

	// equals: null is explicitly allowed and compares equal to itself
	reg(
		linker,
		"wasm:js-string",
		"equals",
		|mut c: Caller<StoreCtx>,
		 a: Option<Rooted<ExternRef>>,
		 b: Option<Rooted<ExternRef>>| {
			profile("js.equals", 0);
			match (&a, &b) {
				(None, None) => Ok(1),
				(None, Some(_)) | (Some(_), None) => Ok(0),
				_ => match (js_ref(&c, &a)?, js_ref(&c, &b)?) {
					(Some(JsVal::String(a)), Some(JsVal::String(b))) => {
						Ok(a.equals(b) as i32)
					}
					(Some(JsVal::String(_)), Some(JsVal::Null))
					| (Some(JsVal::Null), Some(JsVal::String(_))) => Ok(0),
					_ => match (js(&mut c, &a)?, js(&mut c, &b)?) {
						(JsVal::String(a), JsVal::String(b)) => {
							Ok(a.equals(&b) as i32)
						}
						(JsVal::String(_), JsVal::Null)
						| (JsVal::Null, JsVal::String(_)) => Ok(0),
						(other, _) => Err(wasmtime::Error::msg(format!(
							"dart_fmt: js-string equals on non-string {other:?}"
						))),
					},
				},
			}
		},
	)?;

	reg(
		linker,
		"wasm:js-string",
		"compare",
		|mut c: Caller<StoreCtx>,
		 a: Option<Rooted<ExternRef>>,
		 b: Option<Rooted<ExternRef>>| {
			profile("js.compare", 0);
			if let (Some(JsVal::String(a)), Some(JsVal::String(b))) =
				(js_ref(&c, &a)?, js_ref(&c, &b)?)
			{
				return Ok(a.compare(b));
			}
			match (js(&mut c, &a)?, js(&mut c, &b)?) {
				(JsVal::String(a), JsVal::String(b)) => Ok(a.compare(&b)),
				(other, _) => Err(wasmtime::Error::msg(format!(
					"dart_fmt: js-string compare on non-string {other:?}"
				))),
			}
		},
	)?;

	// test: 1 when the value is a string (null is not a string)
	reg(
		linker,
		"wasm:js-string",
		"test",
		|c: Caller<StoreCtx>, s: Option<Rooted<ExternRef>>| {
			profile("js.test", 0);
			let is_string = match s {
				None => false,
				Some(_) => matches!(js_ref(&c, &s)?, Some(JsVal::String(_))),
			};
			Ok(is_string as i32)
		},
	)?;

	// fromCharCodeArray / intoCharCodeArray take a reference to the module's
	// *concrete* `(array (mut i16))` type, which cannot be expressed as a
	// statically-typed host parameter. Build the structurally identical type
	// here so the engine canonicalizes it to the same type index and links
	// exactly, and host them dynamically via `func_new`.
	let array_ty = wasmtime::ArrayType::new(
		engine,
		wasmtime::FieldType::new(
			wasmtime::Mutability::Var,
			wasmtime::StorageType::I16,
		),
	);
	let array_ref = ValType::Ref(wasmtime::RefType::new(
		true,
		wasmtime::HeapType::ConcreteArray(array_ty),
	));
	let extern_nonnull =
		ValType::Ref(wasmtime::RefType::new(false, wasmtime::HeapType::Extern));

	let from_array_ty = wasmtime::FuncType::new(
		engine,
		[array_ref.clone(), ValType::I32, ValType::I32],
		[extern_nonnull],
	);
	reg_dynamic(
		linker,
		"wasm:js-string",
		"fromCharCodeArray",
		from_array_ty,
		|mut caller, params, results| {
			#[cfg(test)]
			let start = std::time::Instant::now();
			let r = (|| {
				let wasmtime::Val::AnyRef(array) = &params[0] else {
					return Err(wasmtime::Error::msg(
						"dart_fmt: fromCharCodeArray: expected array reference",
					));
				};
				let (start, end) =
					match (&params[1], &params[2]) {
						(wasmtime::Val::I32(a), wasmtime::Val::I32(b)) => {
							(*a as u32 as usize, *b as u32 as usize)
						}
						_ => return Err(wasmtime::Error::msg(
							"dart_fmt: fromCharCodeArray: expected i32 range",
						)),
					};
				let units = char_code_array_range(
					&mut caller,
					array.as_ref(),
					start,
					end,
				)?;
				let value =
					alloc_string(&mut caller, JsString::from_units(units))?;
				results[0] = Val::ExternRef(Some(value));
				Ok(())
			})();
			#[cfg(test)]
			HOST_STATS.with(|s| {
				let mut s = s.borrow_mut();
				let e = s.entry("js.fromCharCodeArray").or_insert((0, 0));
				e.0 += 1;
				e.1 += start.elapsed().as_nanos() as u64;
			});
			r
		},
	)?;

	let into_array_ty = wasmtime::FuncType::new(
		engine,
		[ValType::EXTERNREF, array_ref, ValType::I32],
		[ValType::I32],
	);
	reg_dynamic(
		linker,
		"wasm:js-string",
		"intoCharCodeArray",
		into_array_ty,
		|mut caller, params, results| {
			profile("js.intoCharCodeArray", 0);
			let wasmtime::Val::ExternRef(s) = &params[0] else {
				return Err(wasmtime::Error::msg(
					"dart_fmt: intoCharCodeArray: expected string",
				));
			};
			let wasmtime::Val::AnyRef(array) = &params[1] else {
				return Err(wasmtime::Error::msg(
					"dart_fmt: intoCharCodeArray: expected array reference",
				));
			};
			let start = match &params[2] {
				wasmtime::Val::I32(v) => *v as u32 as usize,
				_ => {
					return Err(wasmtime::Error::msg(
						"dart_fmt: intoCharCodeArray: expected i32 start",
					))
				}
			};
			let s = match js(&mut caller, s)? {
				JsVal::String(s) => s,
				other => {
					return Err(wasmtime::Error::msg(format!(
						"dart_fmt: js-string intoCharCodeArray on non-string {other:?}"
					)))
				}
			};
			let Some(array) = array.as_ref() else {
				return Err(wasmtime::Error::msg(
					"dart_fmt: intoCharCodeArray(null)",
				));
			};
			let array =
				array.as_array(caller.as_context())?.ok_or_else(|| {
					wasmtime::Error::msg(
						"dart_fmt: intoCharCodeArray on non-array",
					)
				})?;
			let len = array.len(caller.as_context())? as usize;
			if start + s.len() > len {
				return Err(wasmtime::Error::msg(
					"dart_fmt: intoCharCodeArray does not fit",
				));
			}
			for (i, &unit) in s.units.iter().enumerate() {
				array.set(
					caller.as_context_mut(),
					(start + i) as u32,
					Val::I32(unit as i32),
				)?;
			}
			results[0] = Val::I32(s.len() as i32);
			Ok(())
		},
	)?;

	Ok(())
}

/// Read code units from `[start, end)` of a wasm i16 array into a Vec
fn char_code_array_range(
	caller: &mut impl AsContextMut<Data = StoreCtx>,
	array: Option<&Rooted<wasmtime::AnyRef>>,
	start: usize,
	end: usize,
) -> wasmtime::Result<Vec<u16>> {
	let Some(array) = array else {
		return Err(wasmtime::Error::msg("dart_fmt: fromCharCodeArray(null)"));
	};
	let array = array.as_array(caller.as_context())?.ok_or_else(|| {
		wasmtime::Error::msg("dart_fmt: fromCharCodeArray on non-array")
	})?;
	let len = array.len(caller.as_context())? as usize;
	if start > end || end > len {
		return Err(wasmtime::Error::msg(format!(
			"dart_fmt: fromCharCodeArray range [{start}, {end}) outside array of {len}"
		)));
	}
	let mut units = Vec::with_capacity(end.saturating_sub(start));
	for i in start..end {
		match array.get(caller.as_context_mut(), i as u32)? {
			Val::I32(v) => units.push(v as u16),
			other => {
				return Err(wasmtime::Error::msg(format!(
					"dart_fmt: unexpected char array element {other:?}"
				)))
			}
		}
	}
	Ok(units)
}

/// Register a dynamically-typed host function (used where the import type
/// contains concrete GC types that cannot be expressed as Rust parameters)
fn reg_dynamic(
	linker: &mut Linker<StoreCtx>,
	module: &str,
	name: &str,
	ty: wasmtime::FuncType,
	f: impl Fn(Caller<'_, StoreCtx>, &[Val], &mut [Val]) -> wasmtime::Result<()>
		+ Send
		+ Sync
		+ 'static,
) -> Result<(), String> {
	linker
		.func_new(module, name, ty, f)
		.map(|_| ())
		.map_err(|e| format!("Failed to define {module}::{name}: {e}"))
}
