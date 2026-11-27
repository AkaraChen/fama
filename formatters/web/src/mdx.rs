//! Native pure-Rust MDX (`.mdx`) formatter using `markdown-rs`, Biome, and
//! `dprint-plugin-markdown`.

use biome_diagnostics::Diagnostic as _;
use biome_js_parser::{parse as parse_js, JsParserOptions};
use biome_js_syntax::{
	AnyJsExpression, AnyJsModuleItem, AnyJsRoot, AnyJsStatement, AnyJsxChild,
	AnyJsxTag, JsFileSource, JsxExpressionChild, JsxTagExpression,
};
use biome_rowan::{AstNode, AstNodeList};
use fama_common::{Formatter, LineEnding, CONFIG};
use markdown::mdast::{
	MdxJsxFlowElement, MdxJsxTextElement, Node, Toml, Yaml,
};
use markdown::{Constructs, MdxExpressionKind, MdxSignal, ParseOptions};
use std::path::Path;

/// Formatter implementation for MDX (`.mdx`) files.
pub struct MdxFormatter;

impl Formatter for MdxFormatter {
	fn name(&self) -> &'static str {
		"mdx"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["mdx"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_mdx(content)
	}
}

/// Format an MDX document.
pub fn format_mdx(source: &str) -> Result<String, String> {
	let normalized = normalize_line_endings(source);
	if normalized.trim().is_empty() {
		return Ok(String::new());
	}

	let formatted = format_mdx_inner(&normalized, true, true)?;
	let mut out = if formatted.is_empty() {
		String::new()
	} else {
		let mut s = formatted.trim_end_matches('\n').to_string();
		s.push('\n');
		s
	};

	if matches!(CONFIG.line_ending, LineEnding::Crlf) {
		out = out.replace('\n', "\r\n");
	}
	Ok(out)
}

fn normalize_line_endings(source: &str) -> String {
	if source.contains('\r') {
		source.replace("\r\n", "\n").replace('\r', "\n")
	} else {
		source.to_string()
	}
}

fn mdx_parse_options(allow_frontmatter: bool, allow_esm: bool) -> ParseOptions {
	let mdx = Constructs::mdx();
	ParseOptions {
		constructs: Constructs {
			frontmatter: allow_frontmatter,
			mdx_esm: allow_esm,
			gfm_autolink_literal: true,
			gfm_footnote_definition: true,
			gfm_label_start_footnote: true,
			gfm_strikethrough: true,
			gfm_table: true,
			gfm_task_list_item: true,
			..mdx
		},
		mdx_esm_parse: if allow_esm {
			Some(Box::new(parse_esm_signal))
		} else {
			None
		},
		mdx_expression_parse: Some(Box::new(parse_expression_signal)),
		..ParseOptions::mdx()
	}
}

fn parse_mdx_ast(
	source: &str,
	allow_frontmatter: bool,
	allow_esm: bool,
) -> Result<Node, String> {
	let options = mdx_parse_options(allow_frontmatter, allow_esm);
	markdown::to_mdast(source, &options)
		.map_err(|e| format!("MDX parse error: {e}"))
}

fn parse_esm_signal(value: &str) -> MdxSignal {
	let parsed = parse_js(value, JsFileSource::jsx(), JsParserOptions::default());

	// Check whether any already-parsed top-level item is not import/export.
	if let Some(module) = parsed.tree().as_js_module() {
		for item in module.items() {
			let allowed = matches!(
				item,
				AnyJsModuleItem::JsImport(_)
					| AnyJsModuleItem::JsExport(_)
					| AnyJsModuleItem::AnyJsStatement(
						AnyJsStatement::JsEmptyStatement(_)
					)
			);
			let bogus = matches!(
				item,
				AnyJsModuleItem::AnyJsStatement(
					AnyJsStatement::JsBogusStatement(_)
				)
			);
			if !allowed && (!bogus || !parsed.has_errors()) {
				let offset =
					usize::from(item.syntax().text_trimmed_range().start());
				return MdxSignal::Error(
					"Unexpected non-import/export statement in MDX ESM block"
						.to_string(),
					offset,
					Box::new("biome".to_string()),
					Box::new("esm-statement".to_string()),
				);
			}
		}
	}

	if parsed.has_errors() {
		let diag = parsed
			.diagnostics()
			.iter()
			.find(|d| d.is_error())
			.or_else(|| parsed.diagnostics().first());
		let msg = diag
			.map(|d| format!("{}", d.message))
			.unwrap_or_else(|| "ESM parse error".to_string());
		let span = diag.and_then(|d| d.location().span);
		let trimmed_end = value.trim_end().len();
		let at_eof = span.is_none_or(|r| {
			usize::from(r.start()) >= trimmed_end
				|| usize::from(r.end()) >= value.len()
		});
		if at_eof {
			return MdxSignal::Eof(
				msg,
				Box::new("biome".to_string()),
				Box::new("parse-error".to_string()),
			);
		}
		let offset = span.map_or(0, |r| usize::from(r.start()));
		return MdxSignal::Error(
			msg,
			offset,
			Box::new("biome".to_string()),
			Box::new("parse-error".to_string()),
		);
	}

	MdxSignal::Ok
}

fn parse_expression_signal(value: &str, kind: &MdxExpressionKind) -> MdxSignal {
	// MDX allows comment-only attribute expressions like `<Comp {/* note */} />`.
	if matches!(kind, MdxExpressionKind::AttributeExpression)
		&& is_empty_or_comment_only_jsx_expr(value)
	{
		return MdxSignal::Ok;
	}

	let (prefix, suffix) = match kind {
		MdxExpressionKind::Expression => ("<>{", "}</>"),
		MdxExpressionKind::AttributeExpression => ("<_ {", "} />"),
		MdxExpressionKind::AttributeValueExpression => ("<_ _={", "} />"),
	};

	let wrapped = format!("{prefix}{value}{suffix}");
	let parsed =
		parse_js(&wrapped, JsFileSource::jsx(), JsParserOptions::default());

	if !parsed.has_errors() {
		return MdxSignal::Ok;
	}

	let diag = parsed
		.diagnostics()
		.iter()
		.find(|d| d.is_error())
		.or_else(|| parsed.diagnostics().first());
	let msg = diag
		.map(|d| format!("{}", d.message))
		.unwrap_or_else(|| "Expression parse error".to_string());
	let span = diag.and_then(|d| d.location().span);
	let value_end = prefix.len() + value.len();

	let at_or_past_end = span.is_none_or(|r| {
		usize::from(r.start()) >= value_end || usize::from(r.end()) > value_end
	});

	if at_or_past_end {
		MdxSignal::Eof(
			msg,
			Box::new("biome".to_string()),
			Box::new("parse-error".to_string()),
		)
	} else {
		let offset = span
			.map(|r| usize::from(r.start()).saturating_sub(prefix.len()))
			.unwrap_or(0);
		MdxSignal::Error(
			msg,
			offset,
			Box::new("biome".to_string()),
			Box::new("parse-error".to_string()),
		)
	}
}

fn is_empty_or_comment_only_jsx_expr(value: &str) -> bool {
	let wrapped = format!("<>{{{value}}}</>");
	let parsed =
		parse_js(&wrapped, JsFileSource::jsx(), JsParserOptions::default());
	if parsed.has_errors() {
		return false;
	}
	extract_jsx_expression_child(&parsed.tree())
		.map(|child| child.expression().is_none())
		.unwrap_or(false)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SegmentKind {
	Frontmatter,
	Esm,
	JsxFlow,
	Markdown,
}

struct FormattedSegment {
	kind: SegmentKind,
	text: String,
	orig_start: usize,
	orig_end: usize,
}

fn format_mdx_inner(
	source: &str,
	allow_frontmatter: bool,
	allow_esm: bool,
) -> Result<String, String> {
	if source.trim().is_empty() {
		return Ok(String::new());
	}

	let ast = parse_mdx_ast(source, allow_frontmatter, allow_esm)?;
	let children = match ast {
		Node::Root(root) => root.children,
		_ => return Err("Expected MDX Root node".to_string()),
	};

	if children.is_empty() {
		return Ok(String::new());
	}

	let mut segments: Vec<FormattedSegment> = Vec::new();
	let mut idx = 0;

	while idx < children.len() {
		match &children[idx] {
			Node::Yaml(yaml) => {
				let pos = yaml
					.position
					.as_ref()
					.ok_or_else(|| "Missing position on YAML node".to_string())?;
				let text = format_yaml_frontmatter(yaml)?;
				segments.push(FormattedSegment {
					kind: SegmentKind::Frontmatter,
					text,
					orig_start: pos.start.offset,
					orig_end: pos.end.offset,
				});
				idx += 1;
			}
			Node::Toml(toml) => {
				let pos = toml
					.position
					.as_ref()
					.ok_or_else(|| "Missing position on TOML node".to_string())?;
				let text = format_toml_frontmatter(toml)?;
				segments.push(FormattedSegment {
					kind: SegmentKind::Frontmatter,
					text,
					orig_start: pos.start.offset,
					orig_end: pos.end.offset,
				});
				idx += 1;
			}
			Node::MdxjsEsm(esm) => {
				let pos = esm
					.position
					.as_ref()
					.ok_or_else(|| "Missing position on ESM node".to_string())?;
				let text = format_esm_with_biome(&esm.value)?;
				segments.push(FormattedSegment {
					kind: SegmentKind::Esm,
					text,
					orig_start: pos.start.offset,
					orig_end: pos.end.offset,
				});
				idx += 1;
			}
			Node::MdxJsxFlowElement(el) => {
				let pos = el.position.as_ref().ok_or_else(|| {
					"Missing position on MdxJsxFlowElement".to_string()
				})?;
				let raw = source
					.get(pos.start.offset..pos.end.offset)
					.ok_or_else(|| {
						"Invalid byte range for MdxJsxFlowElement".to_string()
					})?;
				let text = format_jsx_flow_element(raw, el)?;
				segments.push(FormattedSegment {
					kind: SegmentKind::JsxFlow,
					text,
					orig_start: pos.start.offset,
					orig_end: pos.end.offset,
				});
				idx += 1;
			}
			_ => {
				// Group all consecutive Markdown / MdxFlowExpression nodes so
				// dprint-plugin-markdown formats them together with full context.
				let group_start_idx = idx;
				while idx < children.len()
					&& !matches!(
						children[idx],
						Node::Yaml(_)
							| Node::Toml(_) | Node::MdxjsEsm(_)
							| Node::MdxJsxFlowElement(_)
					) {
					idx += 1;
				}
				let group = &children[group_start_idx..idx];
				let first_pos = group
					.first()
					.and_then(Node::position)
					.ok_or_else(|| "Missing position on Markdown node".to_string())?;
				let last_pos = group
					.last()
					.and_then(Node::position)
					.ok_or_else(|| "Missing position on Markdown node".to_string())?;
				let chunk_start = first_pos.start.offset;
				let chunk_end = last_pos.end.offset;
				let text = format_markdown_chunk(
					source,
					chunk_start,
					chunk_end,
					group,
				)?;
				segments.push(FormattedSegment {
					kind: SegmentKind::Markdown,
					text,
					orig_start: chunk_start,
					orig_end: chunk_end,
				});
			}
		}
	}

	// Reassemble segments with appropriate inter-block separators.
	let mut out = String::new();
	for (i, seg) in segments.iter().enumerate() {
		if i > 0 {
			let prev = &segments[i - 1];
			let gap = source.get(prev.orig_end..seg.orig_start).unwrap_or("\n\n");
			let sep = match (prev.kind, seg.kind) {
				(SegmentKind::Frontmatter, _)
				| (SegmentKind::Esm, _)
				| (_, SegmentKind::Esm) => "\n\n",
				(SegmentKind::JsxFlow, SegmentKind::JsxFlow) => {
					if has_blank_line(gap) {
						"\n\n"
					} else {
						"\n"
					}
				}
				_ => "\n\n",
			};
			out.push_str(sep);
		}
		out.push_str(&seg.text);
	}

	Ok(out)
}

fn has_blank_line(gap: &str) -> bool {
	gap.bytes().filter(|&b| b == b'\n').count() >= 2
}

fn format_yaml_frontmatter(yaml: &Yaml) -> Result<String, String> {
	if yaml.value.trim().is_empty() {
		return Ok("---\n---".to_string());
	}
	let formatted = crate::dprint::format_yaml(&yaml.value, "frontmatter.yaml")?;
	let body = formatted.trim_end_matches(['\r', '\n']);
	Ok(format!("---\n{body}\n---"))
}

fn format_toml_frontmatter(toml: &Toml) -> Result<String, String> {
	if toml.value.trim().is_empty() {
		return Ok("+++\n+++".to_string());
	}
	let formatted = match fama_common::format_embedded(
		&toml.value,
		Path::new("frontmatter.toml"),
	) {
		Some(res) => res?,
		None => toml.value.clone(),
	};
	let body = formatted.trim_end_matches(['\r', '\n']);
	Ok(format!("+++\n{body}\n+++"))
}

fn format_esm_with_biome(source: &str) -> Result<String, String> {
	let source_type = JsFileSource::jsx();
	let options = crate::biome::js_format_options(source_type)
		.with_line_ending(biome_formatter::LineEnding::Lf);
	let parsed = parse_js(source, source_type, JsParserOptions::default());
	if parsed.has_errors() {
		return Err("Parse errors in MDX ESM block".to_string());
	}

	let formatted = biome_js_formatter::format_node(options, &parsed.syntax())
		.map_err(|e| format!("MDX ESM format error: {e:?}"))?;
	let code = formatted
		.print()
		.map(|p| p.into_code())
		.map_err(|e| format!("MDX ESM print error: {e:?}"))?;
	Ok(code.trim_end_matches('\n').to_string())
}

fn extract_jsx_tag_expression(root: &AnyJsRoot) -> Option<JsxTagExpression> {
	let module = root.as_js_module()?;
	if module.items().len() != 1 {
		return None;
	}
	let item = module.items().first()?;
	let stmt = item.as_any_js_statement()?.as_js_expression_statement()?;
	let mut expr = stmt.expression().ok()?;
	while let Some(paren) = expr.as_js_parenthesized_expression() {
		expr = paren.expression().ok()?;
	}
	match expr {
		AnyJsExpression::JsxTagExpression(tag) => Some(tag),
		_ => None,
	}
}

fn extract_jsx_expression_child(root: &AnyJsRoot) -> Option<JsxExpressionChild> {
	let tag_expr = extract_jsx_tag_expression(root)?;
	let tag = tag_expr.tag().ok()?;
	let fragment = match tag {
		AnyJsxTag::JsxFragment(frag) => frag,
		_ => return None,
	};
	if fragment.children().len() != 1 {
		return None;
	}
	match fragment.children().first()? {
		AnyJsxChild::JsxExpressionChild(child) => Some(child),
		_ => None,
	}
}

fn format_jsx_element_with_biome(jsx_src: &str) -> Result<String, String> {
	let source_type = JsFileSource::jsx();
	let options = crate::biome::js_format_options(source_type)
		.with_line_ending(biome_formatter::LineEnding::Lf);
	let dedented = dedent_block(jsx_src);
	let parsed = parse_js(&dedented, source_type, JsParserOptions::default());
	if parsed.has_errors() {
		return Err("Parse errors in MDX JSX element".to_string());
	}

	let tag_expr = extract_jsx_tag_expression(&parsed.tree()).ok_or_else(|| {
		"Expected single JSX expression when formatting MDX JSX".to_string()
	})?;
	let printed =
		biome_js_formatter::format_sub_tree(options, tag_expr.syntax())
			.map_err(|e| format!("MDX JSX format error: {e:?}"))?;
	Ok(printed.into_code().trim_end_matches('\n').to_string())
}

fn format_jsx_opening_tag(
	open_tag: &str,
	name: Option<&str>,
) -> Result<String, String> {
	if name.is_none() {
		return Ok("<>".to_string());
	}
	let without_gt = open_tag
		.trim()
		.strip_suffix('>')
		.ok_or_else(|| "JSX opening tag missing closing `>`".to_string())?
		.trim_end();
	let self_closing = format!("{without_gt} />");
	let formatted_sc = format_jsx_element_with_biome(&self_closing)?;
	if let Some(prefix) = formatted_sc.strip_suffix(" />") {
		Ok(format!("{prefix}>"))
	} else if let Some(prefix) = formatted_sc.strip_suffix("/>") {
		Ok(format!("{prefix}>"))
	} else {
		Err("Unexpected Biome output when formatting JSX opening tag".to_string())
	}
}

fn format_mdx_expression(value: &str) -> Result<String, String> {
	let source_type = JsFileSource::jsx();
	let options = crate::biome::js_format_options(source_type)
		.with_line_ending(biome_formatter::LineEnding::Lf);
	let wrapped = format!("<>{{{value}}}</>");
	let parsed = parse_js(&wrapped, source_type, JsParserOptions::default());
	if parsed.has_errors() {
		return Err("Parse errors in MDX expression".to_string());
	}

	let expr_child =
		extract_jsx_expression_child(&parsed.tree()).ok_or_else(|| {
			"Expected single JSX expression child in MDX expression".to_string()
		})?;

	if expr_child.expression().is_none() {
		let trimmed = value.trim();
		if trimmed.is_empty() {
			return Ok("{}".to_string());
		}
		// Preserve JSX comments cleanly without turning `// comment\n` into
		// `{// comment}` (which would comment out the closing brace).
		let check_trimmed = format!("<>{{{trimmed}}}</>");
		let reparsed =
			parse_js(&check_trimmed, source_type, JsParserOptions::default());
		if !reparsed.has_errors() {
			return Ok(format!("{{{trimmed}}}"));
		}
		return Ok(format!("{{{value}}}"));
	}

	let printed =
		biome_js_formatter::format_sub_tree(options, expr_child.syntax())
			.map_err(|e| format!("MDX expression format error: {e:?}"))?;
	let code = printed.into_code().trim_end_matches('\n').to_string();
	if !code.starts_with('{') || !code.ends_with('}') {
		return Err(
			"Unexpected Biome output when formatting MDX expression".to_string(),
		);
	}
	Ok(code)
}

fn is_pure_jsx(children: &[Node]) -> bool {
	children.iter().all(|child| match child {
		Node::MdxJsxFlowElement(el) => is_pure_jsx(&el.children),
		Node::MdxJsxTextElement(el) => is_pure_jsx(&el.children),
		Node::MdxFlowExpression(_) | Node::MdxTextExpression(_) => true,
		Node::Text(t) => t.value.chars().all(char::is_whitespace),
		_ => false,
	})
}

fn format_jsx_flow_element(
	raw: &str,
	el: &MdxJsxFlowElement,
) -> Result<String, String> {
	if is_pure_jsx(&el.children) {
		return format_jsx_element_with_biome(raw);
	}

	let open_end = find_jsx_opening_tag_end(raw)?;
	let close_start = find_jsx_closing_tag_start(raw, el.name.as_deref())?;
	if open_end > close_start {
		return Err("Overlapping JSX opening and closing tags".to_string());
	}

	let open_tag = &raw[..open_end];
	let inner = &raw[open_end..close_start];
	let formatted_open = format_jsx_opening_tag(open_tag, el.name.as_deref())?;
	let formatted_close = match &el.name {
		Some(name) => format!("</{name}>"),
		None => "</>".to_string(),
	};

	if inner.trim().is_empty() {
		return if inner.contains('\n') {
			Ok(format!("{formatted_open}\n{formatted_close}"))
		} else {
			Ok(format!("{formatted_open}{formatted_close}"))
		};
	}

	let (leading_newlines, trailing_newlines, core) =
		split_container_inner(inner);
	let dedented_core = dedent_block(core);
	let allow_esm = el.children.iter().any(|c| matches!(c, Node::MdxjsEsm(_)));
	let formatted_inner = format_mdx_inner(&dedented_core, false, allow_esm)?;
	let formatted_inner = formatted_inner.trim_end_matches('\n');

	let lead_sep = match leading_newlines {
		0 => "",
		1 => "\n",
		_ => "\n\n",
	};
	let trail_sep = match trailing_newlines {
		0 => "",
		1 => "\n",
		_ => "\n\n",
	};

	Ok(format!(
		"{formatted_open}{lead_sep}{formatted_inner}{trail_sep}{formatted_close}"
	))
}

fn format_jsx_text_element(
	raw: &str,
	el: &MdxJsxTextElement,
	full_source: &str,
	el_start_offset: usize,
) -> Result<String, String> {
	// Only format the whole inline element via Biome when it has no children
	// (self-closing or empty pair tag). Passing non-empty inline JSX trees to
	// Biome can expand nested tags onto their own lines, which MDX then
	// re-parses as flow tags and rejects inside a paragraph.
	if el.children.is_empty() {
		return format_jsx_element_with_biome(raw);
	}

	let open_end = find_jsx_opening_tag_end(raw)?;
	let close_start = find_jsx_closing_tag_start(raw, el.name.as_deref())?;
	if open_end > close_start {
		return Err(
			"Overlapping inline JSX opening and closing tags".to_string(),
		);
	}

	let formatted_open =
		format_jsx_opening_tag(&raw[..open_end], el.name.as_deref())?;
	let formatted_close = match &el.name {
		Some(name) => format!("</{name}>"),
		None => "</>".to_string(),
	};

	let inner_raw = &raw[open_end..close_start];
	let inner_base_offset = el_start_offset + open_end;

	// If the raw slice was stripped of blockquote prefixes, offsets in
	// `full_source` might not match `raw`; check length before splicing.
	let mut inner_spans: Vec<(usize, usize, String)> = Vec::new();
	if full_source
		.get(inner_base_offset..inner_base_offset + inner_raw.len())
		== Some(inner_raw)
	{
		collect_inline_mdx_spans(
			full_source,
			inner_base_offset,
			inner_base_offset + inner_raw.len(),
			&el.children,
			&mut inner_spans,
		)?;
	}

	inner_spans.sort_by_key(|(s, _, _)| *s);
	let mut formatted_inner = inner_raw.to_string();
	for (s, e, rep) in inner_spans.into_iter().rev() {
		formatted_inner.replace_range(s..e, &rep);
	}

	Ok(format!("{formatted_open}{formatted_inner}{formatted_close}"))
}

fn collect_inline_mdx_spans(
	full_source: &str,
	base_start: usize,
	base_end: usize,
	nodes: &[Node],
	out: &mut Vec<(usize, usize, String)>,
) -> Result<(), String> {
	for node in nodes {
		match node {
			Node::Code(_) | Node::InlineCode(_) => {}
			Node::MdxTextExpression(expr) => {
				let pos = expr.position.as_ref().ok_or_else(|| {
					"Missing position on MdxTextExpression".to_string()
				})?;
				if pos.start.offset >= base_start && pos.end.offset <= base_end {
					let rep = format_mdx_expression(&expr.value)?;
					out.push((
						pos.start.offset - base_start,
						pos.end.offset - base_start,
						rep,
					));
				}
			}
			Node::MdxJsxTextElement(el) => {
				let pos = el.position.as_ref().ok_or_else(|| {
					"Missing position on MdxJsxTextElement".to_string()
				})?;
				if pos.start.offset >= base_start && pos.end.offset <= base_end {
					let raw = full_source
						.get(pos.start.offset..pos.end.offset)
						.ok_or_else(|| {
							"Invalid byte range for MdxJsxTextElement"
								.to_string()
						})?;
					let rep = format_jsx_text_element(
						raw,
						el,
						full_source,
						pos.start.offset,
					)?;
					out.push((
						pos.start.offset - base_start,
						pos.end.offset - base_start,
						rep,
					));
				}
			}
			_ => {
				if let Some(children) = node.children() {
					collect_inline_mdx_spans(
						full_source,
						base_start,
						base_end,
						children,
						out,
					)?;
				}
			}
		}
	}
	Ok(())
}

struct MaskSpan {
	rel_start: usize,
	rel_end: usize,
	replacement: String,
}

fn format_markdown_chunk(
	full_source: &str,
	chunk_start: usize,
	chunk_end: usize,
	nodes: &[Node],
) -> Result<String, String> {
	let chunk = full_source
		.get(chunk_start..chunk_end)
		.ok_or_else(|| "Invalid byte range for Markdown chunk".to_string())?;

	let mut spans: Vec<MaskSpan> = Vec::new();
	collect_markdown_mask_spans(
		full_source,
		chunk_start,
		nodes,
		&mut spans,
	)?;

	if spans.is_empty() {
		let formatted = crate::dprint::format_markdown(chunk, "mdx.md")?;
		return Ok(formatted.trim_end_matches('\n').to_string());
	}

	spans.sort_by_key(|s| s.rel_start);
	for pair in spans.windows(2) {
		if pair[0].rel_end > pair[1].rel_start {
			return Err("Overlapping MDX mask spans detected".to_string());
		}
	}

	let replacements: Vec<String> =
		spans.iter().map(|s| s.replacement.clone()).collect();
	let prefix = make_placeholder_prefix(full_source, &replacements);

	let mut masked = String::with_capacity(chunk.len());
	let mut cursor = 0;
	let mut placeholders: Vec<String> = Vec::with_capacity(spans.len());

	for (i, span) in spans.iter().enumerate() {
		let token = format!("{prefix}{i}X");
		masked.push_str(&chunk[cursor..span.rel_start]);
		masked.push_str(&token);
		cursor = span.rel_end;
		placeholders.push(token);
	}
	masked.push_str(&chunk[cursor..]);

	let formatted_md = crate::dprint::format_markdown(&masked, "mdx.md")?;
	let restored =
		restore_placeholders(&formatted_md, &prefix, &placeholders, &replacements)?;
	Ok(restored.trim_end_matches('\n').to_string())
}

fn collect_markdown_mask_spans(
	full_source: &str,
	chunk_start: usize,
	nodes: &[Node],
	out: &mut Vec<MaskSpan>,
) -> Result<(), String> {
	for node in nodes {
		match node {
			Node::Code(_) | Node::InlineCode(_) => {
				// Fenced and inline code blocks must remain untouched.
			}
			Node::MdxTextExpression(expr) => {
				let pos = expr.position.as_ref().ok_or_else(|| {
					"Missing position on MdxTextExpression".to_string()
				})?;
				let replacement = format_mdx_expression(&expr.value)?;
				out.push(MaskSpan {
					rel_start: pos.start.offset - chunk_start,
					rel_end: pos.end.offset - chunk_start,
					replacement,
				});
			}
			Node::MdxFlowExpression(expr) => {
				let pos = expr.position.as_ref().ok_or_else(|| {
					"Missing position on MdxFlowExpression".to_string()
				})?;
				let replacement = format_mdx_expression(&expr.value)?;
				out.push(MaskSpan {
					rel_start: pos.start.offset - chunk_start,
					rel_end: pos.end.offset - chunk_start,
					replacement,
				});
			}
			Node::MdxJsxTextElement(el) => {
				let pos = el.position.as_ref().ok_or_else(|| {
					"Missing position on MdxJsxTextElement".to_string()
				})?;
				let raw = full_source
					.get(pos.start.offset..pos.end.offset)
					.ok_or_else(|| {
						"Invalid byte range for MdxJsxTextElement".to_string()
					})?;
				let cleaned = strip_container_continuation_prefix(
					full_source,
					pos.start.offset,
					raw,
				);
				let replacement = format_jsx_text_element(
					&cleaned,
					el,
					full_source,
					pos.start.offset,
				)?;
				out.push(MaskSpan {
					rel_start: pos.start.offset - chunk_start,
					rel_end: pos.end.offset - chunk_start,
					replacement,
				});
			}
			Node::MdxJsxFlowElement(el) => {
				// Block-level JSX nested inside a Markdown container (e.g. blockquote or list).
				let pos = el.position.as_ref().ok_or_else(|| {
					"Missing position on nested MdxJsxFlowElement".to_string()
				})?;
				let raw = full_source
					.get(pos.start.offset..pos.end.offset)
					.ok_or_else(|| {
						"Invalid byte range for nested MdxJsxFlowElement"
							.to_string()
					})?;
				let cleaned = strip_container_continuation_prefix(
					full_source,
					pos.start.offset,
					raw,
				);
				let replacement = format_jsx_flow_element(&cleaned, el)?;
				out.push(MaskSpan {
					rel_start: pos.start.offset - chunk_start,
					rel_end: pos.end.offset - chunk_start,
					replacement,
				});
			}
			_ => {
				if let Some(children) = node.children() {
					collect_markdown_mask_spans(
						full_source,
						chunk_start,
						children,
						out,
					)?;
				}
			}
		}
	}
	Ok(())
}

fn make_placeholder_prefix(source: &str, replacements: &[String]) -> String {
	let mut candidate = "FAMAMDXPH".to_string();
	let mut salt: usize = 0;
	while source.contains(&candidate)
		|| replacements.iter().any(|r| r.contains(&candidate))
	{
		salt += 1;
		candidate = format!("FAMAMDXPH{salt}Z");
	}
	candidate
}

fn restore_placeholders(
	formatted: &str,
	prefix: &str,
	placeholders: &[String],
	replacements: &[String],
) -> Result<String, String> {
	// Verify every placeholder appears strictly 1:1 in `formatted`.
	let mut occurrences: Vec<(usize, usize, usize)> =
		Vec::with_capacity(placeholders.len());
	for (idx, token) in placeholders.iter().enumerate() {
		let mut matches = formatted.match_indices(token.as_str());
		let first = matches.next().ok_or_else(|| {
			format!("MDX placeholder {idx} was dropped during Markdown formatting")
		})?;
		if matches.next().is_some() {
			return Err(format!(
				"MDX placeholder {idx} was duplicated during Markdown formatting"
			));
		}
		occurrences.push((first.0, first.0 + token.len(), idx));
	}

	occurrences.sort_by_key(|(start, _, _)| *start);

	let mut out = String::with_capacity(formatted.len());
	let mut cursor = 0;

	for (start, end, idx) in occurrences {
		out.push_str(&formatted[cursor..start]);
		let replacement = &replacements[idx];
		if replacement.contains('\n') {
			let line_start = formatted[..start]
				.rfind('\n')
				.map_or(0, |nl| nl + 1);
			let before_on_line = &formatted[line_start..start];
			let cont_prefix = continuation_prefix_for_line(before_on_line);
			let mut lines = replacement.split('\n');
			if let Some(first_line) = lines.next() {
				out.push_str(first_line);
			}
			for line in lines {
				out.push('\n');
				if line.is_empty() {
					out.push_str(cont_prefix.trim_end());
				} else {
					out.push_str(&cont_prefix);
					out.push_str(line);
				}
			}
		} else {
			out.push_str(replacement);
		}
		cursor = end;
	}
	out.push_str(&formatted[cursor..]);

	if out.contains(prefix) {
		return Err(
			"MDX placeholder token unexpectedly remained in formatted output"
				.to_string(),
		);
	}

	Ok(out)
}

/// Compute the line prefix to prepend to continuation lines when restoring a
/// multi-line placeholder inside a blockquote or indented list item.
fn continuation_prefix_for_line(before_on_line: &str) -> String {
	if before_on_line
		.chars()
		.all(|c| c == ' ' || c == '\t' || c == '>')
	{
		return before_on_line.to_string();
	}

	// If the line starts with blockquote markers (`> `), preserve those markers
	// for continuation lines even if text precedes the placeholder.
	let bytes = before_on_line.as_bytes();
	let mut i = 0;
	let mut last_gt_end = 0;
	while i < bytes.len() {
		match bytes[i] {
			b' ' | b'\t' => i += 1,
			b'>' => {
				i += 1;
				if i < bytes.len() && bytes[i] == b' ' {
					i += 1;
				}
				last_gt_end = i;
			}
			_ => break,
		}
	}
	if last_gt_end > 0 {
		return before_on_line[..last_gt_end].to_string();
	}

	String::new()
}

/// Strip blockquote `> ` (or matching container indent) prefixes from
/// continuation lines of a raw MDX slice extracted from inside a container.
fn strip_container_continuation_prefix(
	full_source: &str,
	node_start: usize,
	raw: &str,
) -> String {
	if !raw.contains('\n') {
		return raw.to_string();
	}
	let line_start = full_source[..node_start]
		.rfind('\n')
		.map_or(0, |nl| nl + 1);
	let before_on_line = &full_source[line_start..node_start];
	let prefix = continuation_prefix_for_line(before_on_line);
	if prefix.is_empty() {
		return raw.to_string();
	}

	let trimmed_prefix = prefix.trim_end();
	let mut out = String::with_capacity(raw.len());
	let mut lines = raw.split('\n');
	if let Some(first) = lines.next() {
		out.push_str(first);
	}
	for line in lines {
		out.push('\n');
		if let Some(rest) = line.strip_prefix(&prefix) {
			out.push_str(rest);
		} else if !trimmed_prefix.is_empty() {
			if let Some(rest) = line.strip_prefix(trimmed_prefix) {
				out.push_str(rest.strip_prefix(' ').unwrap_or(rest));
			} else {
				out.push_str(line);
			}
		} else {
			out.push_str(line);
		}
	}
	out
}

/// Split a container JSX element's inner substring into
/// `(leading_newlines, trailing_newlines, core_slice)`.
fn split_container_inner(inner: &str) -> (usize, usize, &str) {
	let first_non_ws = inner
		.char_indices()
		.find(|(_, c)| !c.is_whitespace())
		.map_or(inner.len(), |(i, _)| i);
	let last_non_ws_end = inner
		.char_indices()
		.rev()
		.find(|(_, c)| !c.is_whitespace())
		.map_or(0, |(i, c)| i + c.len_utf8());

	let leading_ws = &inner[..first_non_ws];
	let trailing_ws = &inner[last_non_ws_end..];
	let leading_newlines = leading_ws.bytes().filter(|&b| b == b'\n').count();
	let trailing_newlines = trailing_ws.bytes().filter(|&b| b == b'\n').count();

	// Keep horizontal indentation on the first content line so `dedent_block`
	// can compute the true common indent across all lines.
	let core_start = if let Some(last_nl) = leading_ws.rfind('\n') {
		last_nl + 1
	} else {
		first_non_ws
	};
	let core = &inner[core_start..last_non_ws_end];
	(leading_newlines, trailing_newlines, core)
}

/// Remove the minimum common leading whitespace (spaces or tabs) across all
/// non-empty lines of `text`.
fn dedent_block(text: &str) -> String {
	let mut min_indent: Option<&str> = None;
	for line in text.split('\n') {
		if line.trim().is_empty() {
			continue;
		}
		let ws_len = line
			.bytes()
			.take_while(|&b| b == b' ' || b == b'\t')
			.count();
		let ws = &line[..ws_len];
		min_indent = Some(match min_indent {
			None => ws,
			Some(prev) => common_prefix(prev, ws),
		});
	}

	let Some(indent) = min_indent else {
		return text.to_string();
	};
	if indent.is_empty() {
		return text.to_string();
	}

	let mut out = String::with_capacity(text.len());
	for (i, line) in text.split('\n').enumerate() {
		if i > 0 {
			out.push('\n');
		}
		if let Some(rest) = line.strip_prefix(indent) {
			out.push_str(rest);
		} else if line.trim().is_empty() {
			// Normalize blank lines to empty.
		} else {
			out.push_str(line);
		}
	}
	out
}

fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
	let len = a
		.bytes()
		.zip(b.bytes())
		.take_while(|(x, y)| x == y)
		.count();
	&a[..len]
}

/// Find the byte offset immediately after the opening tag's closing `>` in a
/// JSX element slice starting at `<`.
fn find_jsx_opening_tag_end(raw: &str) -> Result<usize, String> {
	let bytes = raw.as_bytes();
	if bytes.first() != Some(&b'<') {
		return Err("JSX element does not start with `<`".to_string());
	}

	let mut i = 1;
	let mut brace_depth: usize = 0;

	while i < bytes.len() {
		if brace_depth == 0 {
			match bytes[i] {
				b'>' => return Ok(i + 1),
				b'"' => {
					i += 1;
					while i < bytes.len() && bytes[i] != b'"' {
						i += 1;
					}
					i += 1;
				}
				b'\'' => {
					i += 1;
					while i < bytes.len() && bytes[i] != b'\'' {
						i += 1;
					}
					i += 1;
				}
				b'{' => {
					brace_depth = 1;
					i += 1;
				}
				_ => i += 1,
			}
		} else {
			i = skip_js_in_braces(bytes, i, &mut brace_depth);
		}
	}

	Err("Unterminated JSX opening tag".to_string())
}

/// Advance through JS tokens inside a `{ ... }` JSX attribute expression,
/// handling strings, template literals, comments, and nested braces.
fn skip_js_in_braces(
	bytes: &[u8],
	mut i: usize,
	brace_depth: &mut usize,
) -> usize {
	match bytes[i] {
		b'{' => {
			*brace_depth += 1;
			i + 1
		}
		b'}' => {
			*brace_depth -= 1;
			i + 1
		}
		b'"' | b'\'' => {
			let quote = bytes[i];
			i += 1;
			while i < bytes.len() {
				if bytes[i] == b'\\' {
					i += 2;
				} else if bytes[i] == quote {
					return i + 1;
				} else {
					i += 1;
				}
			}
			i
		}
		b'`' => skip_template_literal(bytes, i + 1),
		b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
			i += 2;
			while i < bytes.len() && bytes[i] != b'\n' {
				i += 1;
			}
			i
		}
		b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
			i += 2;
			while i + 1 < bytes.len()
				&& !(bytes[i] == b'*' && bytes[i + 1] == b'/')
			{
				i += 1;
			}
			if i + 1 < bytes.len() {
				i + 2
			} else {
				bytes.len()
			}
		}
		_ => i + 1,
	}
}

fn skip_template_literal(bytes: &[u8], mut i: usize) -> usize {
	while i < bytes.len() {
		if bytes[i] == b'\\' {
			i += 2;
		} else if bytes[i] == b'`' {
			return i + 1;
		} else if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
			i += 2;
			let mut inner_depth: usize = 1;
			while i < bytes.len() && inner_depth > 0 {
				i = skip_js_in_braces(bytes, i, &mut inner_depth);
			}
		} else {
			i += 1;
		}
	}
	i
}

/// Find the byte offset of the matching closing tag `</Name>` or `</>` at the
/// end of a non-self-closing JSX element slice.
fn find_jsx_closing_tag_start(
	raw: &str,
	expected_name: Option<&str>,
) -> Result<usize, String> {
	let trimmed = raw.trim_end();
	if !trimmed.ends_with('>') {
		return Err("JSX element does not end with `>`".to_string());
	}
	let close_start = trimmed
		.rfind("</")
		.ok_or_else(|| "JSX element missing closing tag".to_string())?;
	let inner = trimmed[close_start + 2..trimmed.len() - 1].trim();
	let normalized_inner: String =
		inner.chars().filter(|c| !c.is_whitespace()).collect();
	let expected = expected_name.unwrap_or("");
	if normalized_inner != expected {
		return Err(format!(
			"JSX closing tag `</{normalized_inner}>` does not match `<{expected}>`"
		));
	}
	Ok(close_start)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Assert formatting succeeds, output re-parses as valid MDX, and
	/// formatting is strictly idempotent (`format(format(x)) == format(x)`).
	fn assert_mdx_ok(input: &str) -> String {
		let once = format_mdx(input)
			.unwrap_or_else(|e| panic!("first format failed: {e}\ninput:\n{input}"));
		assert!(
			parse_mdx_ast(&once, true, true).is_ok(),
			"formatted output failed MDX re-parse:\n{once}"
		);
		let twice = format_mdx(&once).unwrap_or_else(|e| {
			panic!("second format failed: {e}\nfirst output:\n{once}")
		});
		assert_eq!(
			once, twice,
			"MDX formatter is not idempotent!\n--- once ---\n{once}\n--- twice ---\n{twice}"
		);
		once
	}

	#[test]
	fn test_empty_and_whitespace() {
		assert_eq!(assert_mdx_ok(""), "");
		assert_eq!(assert_mdx_ok("   \n\n\t\n"), "");
	}

	#[test]
	fn test_yaml_and_toml_frontmatter() {
		let yaml_doc = "---\ntitle:   Hello   World\ntags: [a,b]\n---\n\n#   Heading\n";
		let out = assert_mdx_ok(yaml_doc);
		assert!(out.starts_with("---\n"), "{out}");
		assert!(out.contains("title: Hello   World") || out.contains("title:"), "{out}");
		assert!(out.contains("# Heading"), "{out}");

		let toml_doc = "+++\ntitle = \"Hello\"\ndraft = false\n+++\n\n#   Heading\n";
		let out_toml = assert_mdx_ok(toml_doc);
		assert!(out_toml.starts_with("+++\n"), "{out_toml}");
		assert!(out_toml.contains("title = \"Hello\""), "{out_toml}");
		assert!(out_toml.contains("# Heading"), "{out_toml}");
	}

	#[test]
	fn test_esm_blocks_and_comments() {
		let doc = r#"import  {  b, a }   from   './mod'
import React from 'react'
// Export a JSX component
export const Demo=()=> <div   className="box" ><span>{1+2}</span></div>

export default function Layout({children}){
  // render children
  const ok = true

  return <main>{ok && children}</main>
}

# Title
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("import { b, a } from \"./mod\";"), "{out}");
		assert!(out.contains("import React from \"react\";"), "{out}");
		assert!(out.contains("// Export a JSX component"), "{out}");
		assert!(out.contains("// render children"), "{out}");
		assert!(
			out.contains("export const Demo = () => (")
				|| out.contains("export const Demo = () => <div"),
			"{out}"
		);
		assert!(out.contains("export default function Layout"), "{out}");
		assert!(out.contains("# Title"), "{out}");
	}

	#[test]
	fn test_self_closing_and_pure_nested_jsx() {
		let doc = r#"<Hero   title="Welcome"   items={[1,2,3]}   config={{a:1,b:2}}   onClick={()=>{console.log('hi')}} />

<Card   variant="primary">
  <CardHeader   title='Top' />
  <CardBody   data={[  'x',  'y'  ]} />
</Card>
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("items={[1, 2, 3]}"), "{out}");
		assert!(out.contains("config={{ a: 1, b: 2 }}"), "{out}");
		assert!(out.contains("console.log(\"hi\")"), "{out}");
		assert!(out.contains("<CardHeader title=\"Top\" />"), "{out}");
		assert!(out.contains("data={[\"x\", \"y\"]}"), "{out}");
	}

	#[test]
	fn test_container_jsx_blank_line_semantics() {
		// Inline/same-line tag: stays phrasing (no added newlines or Paragraph)
		let inline = "<Box   padding={2}>**a**</Box>\n";
		let out_inline = assert_mdx_ok(inline);
		assert_eq!(out_inline, "<Box padding={2}>**a**</Box>\n");

		// No blank line: preserves exact single-newline tightness (never inserts blank lines)
		let tight = "<Box   padding={2}>\n**a**\n</Box>\n";
		let out_tight = assert_mdx_ok(tight);
		assert_eq!(out_tight, "<Box padding={2}>\n**a**\n</Box>\n");

		// With blank lines: preserves exact blank lines (never strips blank lines)
		let loose = "<Box   padding={2}>\n\n**a**\n\n</Box>\n";
		let out_loose = assert_mdx_ok(loose);
		assert_eq!(out_loose, "<Box padding={2}>\n\n**a**\n\n</Box>\n");

		// Thematic breaks inside a container must not be misparsed as YAML frontmatter
		let with_hr = "<Box>\n\n---\n\nHello\n\n---\n\n</Box>\n";
		let out_hr = assert_mdx_ok(with_hr);
		assert!(out_hr.contains("---\n\nHello\n\n---"), "{out_hr}");
	}

	#[test]
	fn test_nested_inline_jsx_and_complex_container_attributes() {
		let doc = r#"Look at <Wrapper   a={1}><Icon   name='check' /></Wrapper> and <Badge   variant="ok">**bold** {1+2}</Badge>.

<Callout   title="a > b"   onClick={()=>{console.log(1>0)}}   description="A very long description that causes Biome to wrap attributes onto multiple lines">

-   item with {a+b}

</Callout>

- First list item

  <Callout   type="info">

  Nested **markdown** in list item.

  </Callout>

> {items.filter((x) => x > 0).map((x) => (
>   <Item   key={x}   value={x*2} />
> ))}
"#;
		let out = assert_mdx_ok(doc);
		assert!(
			out.contains("<Wrapper a={1}><Icon name=\"check\" /></Wrapper>"),
			"{out}"
		);
		assert!(
			out.contains("<Badge variant=\"ok\">**bold** {1 + 2}</Badge>"),
			"{out}"
		);
		assert!(out.contains("title=\"a > b\""), "{out}");
		assert!(out.contains("- item with {a + b}"), "{out}");
		assert!(out.contains("  <Callout type=\"info\">"), "{out}");
		assert!(out.contains("<Item key={x} value={x * 2} />"), "{out}");
	}

	#[test]
	fn test_container_jsx_with_markdown_and_nested_tabs() {
		let doc = r#"<Callout   type="warning"   title={'Heads up'}>

##   Important

*   **first** item
*   second item

```ts
const   x=1
```

</Callout>

<Tabs   defaultValue="npm">

<Tab   value="npm">

Run `npm install fama`.

</Tab>

<Tab   value="pnpm">

Run `pnpm add fama`.

</Tab>

</Tabs>
"#;
		let out = assert_mdx_ok(doc);
		assert!(
			out.contains("<Callout type=\"warning\" title={\"Heads up\"}>"),
			"{out}"
		);
		assert!(out.contains("## Important"), "{out}");
		assert!(out.contains("- **first** item"), "{out}");
		assert!(out.contains("<Tabs defaultValue=\"npm\">"), "{out}");
		assert!(out.contains("<Tab value=\"npm\">"), "{out}");
		assert!(out.contains("<Tab value=\"pnpm\">"), "{out}");
	}

	#[test]
	fn test_jsx_fragments() {
		let doc = r#"<>

#   Inside Fragment

Some **markdown** text.

</>

<>
  <Item   id={1} />
  <Item   id={2} />
</>
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("<>\n\n# Inside Fragment"), "{out}");
		assert!(out.contains("<Item id={1} />"), "{out}");
		assert!(out.contains("<Item id={2} />"), "{out}");
	}

	#[test]
	fn test_inline_jsx_and_expressions_everywhere() {
		let doc = r#"#   Heading with <Badge   variant="ok">new</Badge> and {1+2}

A paragraph with <Badge   variant='info'>beta</Badge>, {a+b}, and {items.map(x=>x*2)}.

*   List item with <Tag   color="red">hot</Tag> and {x*3}
*   Second item with <Icon   name="check" />

> Quote with <Badge   variant="ok">new</Badge> and {a+b}
> and another line with {c+d}.

| Feature | Status | Count |
| --- | --- | --- |
| MDX | <Badge   variant="ok">new</Badge> | {a+b} |
| GFM | <Icon   name="star" /> | {items.map(x=>x*2)} |
"#;
		let out = assert_mdx_ok(doc);
		assert!(
			out.contains("# Heading with <Badge variant=\"ok\">new</Badge> and {1 + 2}"),
			"{out}"
		);
		assert!(
			out.contains("<Badge variant=\"info\">beta</Badge>, {a + b}, and {items.map((x) => x * 2)}."),
			"{out}"
		);
		assert!(
			out.contains("- List item with <Tag color=\"red\">hot</Tag> and {x * 3}"),
			"{out}"
		);
		assert!(
			out.contains("> Quote with <Badge variant=\"ok\">new</Badge> and {a + b}"),
			"{out}"
		);
		assert!(out.contains("{c + d}"), "{out}");
		assert!(out.contains("| MDX"), "{out}");
	}

	#[test]
	fn test_block_expressions_and_jsx_comments() {
		let doc = r#"{/*   section note   */}

{show && <Panel   title='Hello'   count={1+2} />}

> {/* blockquote note */}
>
> {ready && <Status   ok={true} />}
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("{/*   section note   */}"), "{out}");
		assert!(
			out.contains("{show && <Panel title=\"Hello\" count={1 + 2} />}"),
			"{out}"
		);
		assert!(out.contains("> {/* blockquote note */}"), "{out}");
		assert!(out.contains("> {ready && <Status ok={true} />}"), "{out}");
	}

	#[test]
	fn test_fenced_code_blocks_untouched() {
		let doc = r#"Here is an example:

```mdx
<Unformatted   a={1+2}   b='x' />
{a+b}
{/* keep   as   is */}
```

And inline `<NotJsx   a={1+2} />` and `{not+expr}`.
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("<Unformatted   a={1+2}   b='x' />"), "{out}");
		assert!(out.contains("{a+b}"), "{out}");
		assert!(out.contains("{/* keep   as   is */}"), "{out}");
		assert!(out.contains("`<NotJsx   a={1+2} />`"), "{out}");
		assert!(out.contains("`{not+expr}`"), "{out}");
	}

	#[test]
	fn test_placeholder_collision_avoidance() {
		let doc = r#"Text containing FAMAMDXPH0X and FAMAMDXPH1X and FAMAMDXPH1Z0X before {a+b} and <Badge   kind="ok">hi</Badge>.
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.contains("FAMAMDXPH0X"), "{out}");
		assert!(out.contains("FAMAMDXPH1X"), "{out}");
		assert!(out.contains("FAMAMDXPH1Z0X"), "{out}");
		assert!(out.contains("{a + b}"), "{out}");
		assert!(out.contains("<Badge kind=\"ok\">hi</Badge>"), "{out}");
	}

	#[test]
	fn test_realistic_docs_page() {
		let doc = r#"---
title: Getting Started
description: Learn how to format MDX with Fama
---

import { Callout, Steps, Tab, Tabs } from "nextra/components"
import   {  Badge  } from './components/Badge'

export const meta = {   version: '1.0.0',   stable: true }

#   Getting Started <Badge   variant="success">v{meta.version}</Badge>

{/* Keep this intro concise */}

Fama formats **MDX** natively with zero config.

<Callout   type="info">

Make sure you have `fama` installed before running the steps below.

- Works on **Linux**, **macOS**, and **Windows**
- Formats {1+1}x faster than spawning Node

</Callout>

<Steps>

###   Step 1: Install

<Tabs   items={['curl', 'cargo']}>

<Tab>

```bash
curl -fsSL https://example.com/install.sh | sh
```

</Tab>

<Tab>

```bash
cargo install fama
```

</Tab>

</Tabs>

###   Step 2: Run

> Tip: pass `--check` in CI to verify formatting without writing files ({meta.stable ? 'recommended' : 'experimental'}).

</Steps>

{meta.stable && <Footer   year={2026} />}
"#;
		let out = assert_mdx_ok(doc);
		assert!(out.starts_with("---\n"), "{out}");
		assert!(
			out.contains("export const meta = { version: \"1.0.0\", stable: true };"),
			"{out}"
		);
		assert!(
			out.contains("# Getting Started <Badge variant=\"success\">v{meta.version}</Badge>"),
			"{out}"
		);
		assert!(out.contains("<Callout type=\"info\">"), "{out}");
		assert!(out.contains("<Tabs items={[\"curl\", \"cargo\"]}>"), "{out}");
		assert!(out.contains("{meta.stable && <Footer year={2026} />}"), "{out}");
	}

	#[test]
	fn test_invalid_mdx_and_js_returns_err() {
		for bad in [
			"<Unclosed>\n\nHello\n",
			"<Foo></Bar>\n",
			"Hello {a + * b} world\n",
			"export const x = ;\n",
			"<Comp a={1 +} />\n",
			"Hello {unclosed\n",
		] {
			let res = format_mdx(bad);
			assert!(
				res.is_err(),
				"expected error for invalid MDX:\n{bad}\ngot: {res:?}"
			);
		}
	}

	#[test]
	fn test_complex_expressions_spread_member_tags_and_crlf() {
		let doc = concat!(
			"export const items = [\r\n",
			"  1,\r\n",
			"\r\n",
			"  2,\r\n",
			"]\r\n",
			"\r\n",
			"<Comp   {...props}   a={1+2} />\r\n",
			"<Second   b='x' />\r\n",
			"\r\n",
			"<Tabs.List   className='list'>\r\n",
			"\r\n",
			"-   Value: {{ a: 1, b: 2 }} and {\"}\"}\r\n",
			"-   Block fn: {items.map((x)=>{const y=x*2;return y;})}\r\n",
			"-   Comment brace: {/* } */}\r\n",
			"\r\n",
			"</Tabs.List>\r\n",
		);
		let out = assert_mdx_ok(doc);
		assert!(!out.contains('\r'), "output should use LF: {out:?}");
		assert!(out.contains("<Comp {...props} a={1 + 2} />\n<Second b=\"x\" />"), "{out}");
		assert!(out.contains("<Tabs.List className=\"list\">"), "{out}");
		assert!(out.contains("</Tabs.List>"), "{out}");
		assert!(out.contains("{{ a: 1, b: 2 }}"), "{out}");
		assert!(out.contains("{\"}\"}"), "{out}");
		assert!(out.contains("{/* } */}"), "{out}");
	}

	#[test]
	fn test_restore_placeholders_fail_safe() {
		let prefix = "FAMAMDXPH";
		let phs = vec!["FAMAMDXPH0X".to_string(), "FAMAMDXPH1X".to_string()];
		let reps = vec!["{a}".to_string(), "{b}".to_string()];

		// Missing placeholder -> Err
		assert!(restore_placeholders("only FAMAMDXPH0X", prefix, &phs, &reps).is_err());
		// Duplicated placeholder -> Err
		assert!(
			restore_placeholders(
				"FAMAMDXPH0X FAMAMDXPH0X FAMAMDXPH1X",
				prefix,
				&phs,
				&reps
			)
			.is_err()
		);
		// Exact 1:1 -> Ok
		assert_eq!(
			restore_placeholders("FAMAMDXPH0X and FAMAMDXPH1X", prefix, &phs, &reps)
				.unwrap(),
			"{a} and {b}"
		);
	}
}
