//! A minimal JS value model used to back the `externref` values dart2wasm
//! exchanges with its host.
//!
//! dart2wasm (with js-string builtins + js-interop runtime) treats every
//! host-provided `externref` as an opaque JS value: strings, numbers,
//! booleans, objects, arrays, functions and regexes. The helpers in `shims`
//! implement the JS primitives the compiled dart_style code needs on top of
//! this model.
//!
//! Strings are stored as UTF-16 code units because every JS string operation
//! (and therefore every `String` operation dart_style performs) is indexed in
//! UTF-16 code units. Keeping that representation makes `length`, `charCodeAt`
//! and `substring` O(1)/O(k) instead of O(n) rescans of UTF-8.

use std::collections::BTreeMap;

use fancy_regex::Regex;

/// A JS string: a sequence of UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JsString {
	pub units: Vec<u16>,
}

impl JsString {
	pub fn from_rust(s: &str) -> JsString {
		JsString {
			units: s.encode_utf16().collect(),
		}
	}

	pub fn from_units(units: Vec<u16>) -> JsString {
		JsString { units }
	}

	/// UTF-16 code-unit length
	pub fn len(&self) -> usize {
		self.units.len()
	}

	/// Convert to a Rust string (surrogate pairs become their scalar value;
	/// lone surrogates are replaced, which cannot happen for text read from
	/// valid UTF-8 source files)
	pub fn to_rust(&self) -> String {
		String::from_utf16_lossy(&self.units)
	}

	pub fn char_at(&self, index: usize) -> Option<u16> {
		self.units.get(index).copied()
	}

	/// JS `String.prototype.substring`: start/end clamped, swapped when
	/// start > end
	pub fn substring(&self, start: i32, end: i32) -> JsString {
		let (mut a, mut b) = (start.max(0), end.max(0));
		if a > b {
			std::mem::swap(&mut a, &mut b);
		}
		let a = (a as usize).min(self.len());
		let b = (b as usize).min(self.len());
		JsString {
			units: self.units[a..b].to_vec(),
		}
	}

	pub fn concat(&self, other: &JsString) -> JsString {
		let mut units = self.units.clone();
		units.extend_from_slice(&other.units);
		JsString { units }
	}

	/// JS `===` on strings: UTF-16 code-unit equality
	pub fn equals(&self, other: &JsString) -> bool {
		self.units == other.units
	}

	/// JS relational comparison of strings: UTF-16 code-unit order
	pub fn compare(&self, other: &JsString) -> i32 {
		if self.units == other.units {
			return 0;
		}
		if self.units < other.units {
			-1
		} else {
			1
		}
	}

	/// JS `String.prototype.indexOf`: UTF-16 index or -1
	pub fn index_of(&self, token: &JsString, from: i32) -> i32 {
		if token.units.is_empty() {
			let start = from.clamp(0, self.len() as i32);
			return start;
		}
		let from = from.max(0) as usize;
		if from > self.len() {
			return -1;
		}
		find_units(&self.units[from..], &token.units)
			.map(|i| (i + from) as i32)
			.unwrap_or(-1)
	}

	/// JS `String.prototype.lastIndexOf`: last occurrence at or before
	/// `from` (a UTF-16 index), or -1
	pub fn last_index_of(&self, token: &JsString, from: i32) -> i32 {
		let len = self.len() as i32;
		let token_len = token.len() as i32;
		let mut end = from;
		if end >= len {
			end = len - 1;
		}
		if end < 0 {
			return -1;
		}
		let mut i = end;
		while i >= 0 {
			if i + token_len <= len
				&& self.units[i as usize..(i + token_len) as usize]
					== token.units[..]
			{
				return i;
			}
			i -= 1;
		}
		-1
	}

	/// JS `String.prototype.split` with a string separator (no limit)
	pub fn split(&self, token: &JsString) -> Vec<JsString> {
		if token.units.is_empty() {
			// JS splits into individual UTF-16 code units
			return self
				.units
				.iter()
				.map(|&u| JsString { units: vec![u] })
				.collect();
		}
		let mut parts = Vec::new();
		let mut start = 0usize;
		let mut i = 0usize;
		while i + token.units.len() <= self.units.len() {
			if self.units[i..i + token.units.len()] == token.units[..] {
				parts.push(JsString {
					units: self.units[start..i].to_vec(),
				});
				i += token.units.len();
				start = i;
			} else {
				i += 1;
			}
		}
		parts.push(JsString {
			units: self.units[start..].to_vec(),
		});
		parts
	}

	pub fn repeat(&self, times: i32) -> JsString {
		let mut units = Vec::with_capacity(self.len() * times.max(0) as usize);
		for _ in 0..times.max(0) {
			units.extend_from_slice(&self.units);
		}
		JsString { units }
	}

	/// JS `String.prototype.replaceAll` with a plain string replacement
	/// (the loader wraps the replacement in a function, so `$` has no
	/// special meaning)
	pub fn replace_all(
		&self,
		token: &JsString,
		replacement: &JsString,
	) -> JsString {
		if token.units.is_empty() {
			// JS inserts the replacement between every unit and at both ends
			let mut units = replacement.units.clone();
			for &u in &self.units {
				units.push(u);
				units.extend_from_slice(&replacement.units);
			}
			return JsString { units };
		}
		let mut units = Vec::with_capacity(self.len());
		let mut i = 0usize;
		while i < self.units.len() {
			if i + token.units.len() <= self.units.len()
				&& self.units[i..i + token.units.len()] == token.units[..]
			{
				units.extend_from_slice(&replacement.units);
				i += token.units.len();
			} else {
				units.push(self.units[i]);
				i += 1;
			}
		}
		JsString { units }
	}

	/// JS `String.prototype.trim` (Unicode whitespace plus BOM)
	pub fn trim(&self) -> JsString {
		let is_ws = |u: u16| matches!(u, 0x0009..=0x000D | 0x0020 | 0x00A0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF);
		let start = self.units.iter().position(|u| !is_ws(*u));
		let Some(start) = start else {
			return JsString::default();
		};
		let end = self.units.iter().rposition(|u| !is_ws(*u)).unwrap();
		JsString {
			units: self.units[start..=end].to_vec(),
		}
	}

	pub fn to_upper(&self) -> JsString {
		JsString::from_rust(&self.to_rust().to_uppercase())
	}

	pub fn to_lower(&self) -> JsString {
		JsString::from_rust(&self.to_rust().to_lowercase())
	}

	/// Parse like `parseFloat`: longest valid float prefix, NaN when none
	pub fn parse_float(&self) -> f64 {
		parse_float_prefix(&self.to_rust())
	}

	/// Parse like `Number(s)` / unary `+s`
	pub fn to_number(&self) -> f64 {
		let s = self.trim().to_rust();
		if s.is_empty() {
			return 0.0;
		}
		if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))
		{
			return u32::from_str_radix(hex, 16)
				.map(|v| v as f64)
				.unwrap_or(f64::NAN);
		}
		match s.as_str() {
			"Infinity" | "+Infinity" => f64::INFINITY,
			"-Infinity" => f64::NEG_INFINITY,
			_ => {
				if s.chars().all(|c| {
					c.is_ascii_digit()
						|| matches!(c, '.' | 'e' | 'E' | '+' | '-')
				}) {
					s.parse::<f64>().unwrap_or(f64::NAN)
				} else {
					f64::NAN
				}
			}
		}
	}
}

fn find_units(haystack: &[u16], needle: &[u16]) -> Option<usize> {
	haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_float_prefix(s: &str) -> f64 {
	let s = s.trim_start();
	let bytes = s.as_bytes();
	let mut end = 0;
	let mut seen_digit = false;
	let mut seen_dot = false;
	let mut seen_exp = false;
	while end < bytes.len() {
		let c = bytes[end];
		match c {
			b'+' | b'-'
				if end == 0 || matches!(bytes[end - 1], b'e' | b'E') => {}
			b'.' if !seen_dot && !seen_exp => seen_dot = true,
			b'e' | b'E' if seen_digit && !seen_exp => seen_exp = true,
			b'0'..=b'9' => seen_digit = true,
			_ => break,
		}
		end += 1;
	}
	if !seen_digit {
		return f64::NAN;
	}
	s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `Number.prototype.toString` semantics for doubles
pub fn js_number_to_string(v: f64) -> String {
	if v.is_nan() {
		return "NaN".to_string();
	}
	if v.is_infinite() {
		return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
	}
	if v == 0.0 {
		return "0".to_string();
	}
	if v.fract() == 0.0 && v.abs() < 1e21 {
		return format!("{}", v as i64);
	}
	// Rust's Display gives the shortest round-trip decimal, like JS, but
	// never switches to exponent notation; the extreme magnitudes where JS
	// differs cannot appear in text dart_style produces
	format!("{}", v)
}

/// JS `Number.prototype.toString(radix)` semantics for integers
pub fn js_int_to_string_radix(mut v: i64, radix: i32) -> String {
	if !(2..=36).contains(&radix) {
		return js_number_to_string(v as f64);
	}
	if v == 0 {
		return "0".to_string();
	}
	let negative = v < 0;
	v = v.abs();
	const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
	let mut out = Vec::new();
	while v > 0 {
		out.push(DIGITS[(v % radix as i64) as usize]);
		v /= radix as i64;
	}
	if negative {
		out.push(b'-');
	}
	out.reverse();
	String::from_utf8(out).unwrap()
}

/// A JS object: string-keyed fields (insertion order is not observable for
/// the fields dart_style uses)
#[derive(Debug, Clone, Default)]
pub struct JsObject {
	pub fields: BTreeMap<String, JsVal>,
}

impl JsObject {
	pub fn get(&self, key: &str) -> JsVal {
		self.fields.get(key).cloned().unwrap_or(JsVal::Undefined)
	}
}

/// A JS RegExp backed by the `fancy-regex` crate (JS-flavored patterns with
/// look-around; the analyzer package's parser uses lookaheads)
#[derive(Debug, Clone)]
pub struct JsRegExp {
	pub flags: String,
	pub global: bool,
	pub sticky: bool,
	pub re: Regex,
	/// JS `lastIndex` in UTF-16 code units (only used for g/y flags)
	pub last_index: i32,
}

impl JsRegExp {
	pub fn new(source: &str, flags: &str) -> Result<JsRegExp, String> {
		let mut rust_pattern = String::with_capacity(source.len() + 8);
		// JS ^/$ are line anchors only with the m flag; Rust always treats
		// them as text anchors unless (?m) is set - dart_style's patterns
		// are single-line so the default matches
		rust_pattern.push_str(source);
		let mut rust_flags = String::new();
		if flags.contains('i') {
			rust_flags.push('i');
		}
		if flags.contains('m') {
			rust_flags.push('m');
		}
		if flags.contains('s') {
			rust_flags.push('s');
		}
		if !rust_flags.is_empty() {
			rust_pattern = format!("(?{}){}", rust_flags, rust_pattern);
		}
		let re = Regex::new(&rust_pattern)
			.map_err(|e| format!("invalid RegExp /{source}/{flags}: {e}"))?;
		Ok(JsRegExp {
			flags: flags.to_string(),
			global: flags.contains('g'),
			sticky: flags.contains('y'),
			re,
			last_index: 0,
		})
	}
}

/// A JS value.
#[derive(Debug, Clone, Default)]
pub enum JsVal {
	#[default]
	Undefined,
	Null,
	Bool(bool),
	Number(f64),
	String(JsString),
	Object(JsObject),
	Array(Vec<JsVal>),
	/// A Dart closure wrapped for JS: index into the per-thread function
	/// registry in `shims`
	Function(usize),
	RegExp(JsRegExp),
}

impl JsVal {
	pub fn string(s: &str) -> JsVal {
		JsVal::String(JsString::from_rust(s))
	}

	/// JS `String(value)`
	pub fn to_js_string(&self) -> JsString {
		match self {
			JsVal::Undefined => JsString::from_rust("undefined"),
			JsVal::Null => JsString::from_rust("null"),
			JsVal::Bool(b) => {
				JsString::from_rust(if *b { "true" } else { "false" })
			}
			JsVal::Number(n) => JsString::from_rust(&js_number_to_string(*n)),
			JsVal::String(s) => s.clone(),
			JsVal::Array(items) => {
				let parts: Vec<String> =
					items.iter().map(|v| v.to_js_string().to_rust()).collect();
				JsString::from_rust(&parts.join(","))
			}
			JsVal::Object(_) => JsString::from_rust("[object Object]"),
			JsVal::Function(_) => {
				JsString::from_rust("function () { [native code] }")
			}
			JsVal::RegExp(r) => {
				JsString::from_rust(&format!("/{}/{}", r.re.as_str(), r.flags))
			}
		}
	}

	/// JS `JSON.stringify`
	pub fn to_json(&self) -> JsString {
		JsString::from_rust(&self.to_json_string())
	}

	fn to_json_string(&self) -> String {
		match self {
			JsVal::Undefined | JsVal::Null | JsVal::Function(_) => {
				"null".to_string()
			}
			JsVal::Bool(b) => if *b { "true" } else { "false" }.to_string(),
			JsVal::Number(n) => js_number_to_string(*n),
			JsVal::String(s) => {
				let escaped: String = s
					.to_rust()
					.chars()
					.flat_map(|c| match c {
						'"' => vec!['\\', '"'],
						'\\' => vec!['\\', '\\'],
						'\n' => vec!['\\', 'n'],
						'\r' => vec!['\\', 'r'],
						'\t' => vec!['\\', 't'],
						c => vec![c],
					})
					.collect();
				format!("\"{escaped}\"")
			}
			JsVal::Array(items) => {
				let parts: Vec<String> =
					items.iter().map(|v| v.to_json_string()).collect();
				format!("[{}]", parts.join(","))
			}
			JsVal::Object(o) => {
				let parts: Vec<String> = o
					.fields
					.iter()
					.map(|(k, v)| {
						format!(
							"\"{}\":{}",
							k.replace('"', "\\\""),
							v.to_json_string()
						)
					})
					.collect();
				format!("{{{}}}", parts.join(","))
			}
			JsVal::RegExp(_) => "{}".to_string(),
		}
	}

	/// JS `typeof`-style type tag used by dart2wasm's `bB` helper
	pub fn type_tag(&self) -> i32 {
		match self {
			JsVal::Undefined => 1,
			JsVal::Bool(_) => 2,
			JsVal::Number(_) => 3,
			JsVal::String(_) => 4,
			JsVal::Array(_) => 5,
			_ => 19,
		}
	}
}
