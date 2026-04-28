// Tree-sitter symbol extraction. Returns None if the grammar fails to load,
// fails to parse, or the parse tree contains so many ERROR nodes that the
// result wouldn't be trustworthy — callers fall back to the heuristic path.

use tree_sitter::{Language, Node, Parser};

use crate::Language as Lang;

/// Returned by `extract`; mirrors `crate::Symbol` but isolated from internal types.
pub(crate) struct ParsedSymbol {
    pub line: usize,
    pub end_line: usize,
    pub indent: usize,
    pub kind: &'static str,
    pub signature: String,
}

/// Maximum fraction of source that may be inside ERROR / MISSING nodes before
/// we discard the parse and let the heuristic take over. 5% matches the spec.
const MAX_ERROR_RATIO: f64 = 0.05;

pub(crate) fn extract(lang: Lang, source: &str) -> Option<Vec<ParsedSymbol>> {
    let (ts_lang, kinds) = language_spec(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&ts_lang).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    if error_ratio(root, source.len()) > MAX_ERROR_RATIO {
        return None;
    }
    let bytes = source.as_bytes();
    let mut symbols = Vec::new();
    walk(root, bytes, source, kinds, &mut symbols);
    Some(symbols)
}

fn language_spec(lang: Lang) -> Option<(Language, &'static [(&'static str, &'static str)])> {
    match lang {
        Lang::Python => Some((
            tree_sitter_python::LANGUAGE.into(),
            &[
                ("class_definition", "class"),
                ("function_definition", "function"),
            ],
        )),
        Lang::JavaScript => Some((
            tree_sitter_javascript::LANGUAGE.into(),
            &[
                ("class_declaration", "class"),
                ("function_declaration", "function"),
                ("method_definition", "method"),
                ("generator_function_declaration", "function"),
            ],
        )),
        Lang::TypeScript => Some((
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            &[
                ("class_declaration", "class"),
                ("interface_declaration", "interface"),
                ("type_alias_declaration", "type"),
                ("enum_declaration", "enum"),
                ("function_declaration", "function"),
                ("method_definition", "method"),
                ("abstract_method_signature", "method"),
            ],
        )),
        Lang::Go => Some((
            tree_sitter_go::LANGUAGE.into(),
            &[
                ("function_declaration", "function"),
                ("method_declaration", "method"),
                ("type_declaration", "type"),
            ],
        )),
        Lang::Rust => Some((
            tree_sitter_rust::LANGUAGE.into(),
            &[
                ("struct_item", "struct"),
                ("enum_item", "enum"),
                ("trait_item", "trait"),
                ("impl_item", "impl"),
                ("function_item", "function"),
                ("function_signature_item", "function"),
                ("mod_item", "mod"),
                ("type_item", "type"),
            ],
        )),
        Lang::Java => Some((
            tree_sitter_java::LANGUAGE.into(),
            &[
                ("class_declaration", "class"),
                ("interface_declaration", "interface"),
                ("enum_declaration", "enum"),
                ("record_declaration", "record"),
                ("method_declaration", "method"),
                ("constructor_declaration", "method"),
            ],
        )),
        Lang::Ruby => Some((
            tree_sitter_ruby::LANGUAGE.into(),
            &[
                ("class", "class"),
                ("module", "module"),
                ("method", "method"),
                ("singleton_method", "method"),
            ],
        )),
        Lang::C => Some((
            tree_sitter_c::LANGUAGE.into(),
            &[
                ("function_definition", "function"),
                ("struct_specifier", "struct"),
                ("enum_specifier", "enum"),
                ("type_definition", "type"),
            ],
        )),
        Lang::Cpp => Some((
            tree_sitter_cpp::LANGUAGE.into(),
            &[
                ("function_definition", "function"),
                ("class_specifier", "class"),
                ("struct_specifier", "struct"),
                ("enum_specifier", "enum"),
                ("namespace_definition", "namespace"),
                ("type_definition", "type"),
            ],
        )),
        Lang::CSharp => Some((
            tree_sitter_c_sharp::LANGUAGE.into(),
            &[
                ("class_declaration", "class"),
                ("interface_declaration", "interface"),
                ("struct_declaration", "struct"),
                ("enum_declaration", "enum"),
                ("record_declaration", "record"),
                ("method_declaration", "method"),
                ("constructor_declaration", "method"),
            ],
        )),
        Lang::Php => Some((
            tree_sitter_php::LANGUAGE_PHP.into(),
            &[
                ("class_declaration", "class"),
                ("interface_declaration", "interface"),
                ("trait_declaration", "trait"),
                ("function_definition", "function"),
                ("method_declaration", "method"),
            ],
        )),
        Lang::Swift => Some((
            tree_sitter_swift::LANGUAGE.into(),
            &[
                ("class_declaration", "class"),
                ("protocol_declaration", "protocol"),
                ("function_declaration", "function"),
                ("init_declaration", "method"),
            ],
        )),
        Lang::Kotlin => Some((
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            &[
                ("class_declaration", "class"),
                ("function_declaration", "function"),
                ("object_declaration", "object"),
            ],
        )),
        Lang::Scala => Some((
            tree_sitter_scala::LANGUAGE.into(),
            &[
                ("class_definition", "class"),
                ("trait_definition", "trait"),
                ("object_definition", "object"),
                ("function_definition", "function"),
                ("function_declaration", "function"),
            ],
        )),
        Lang::Lua => Some((
            tree_sitter_lua::LANGUAGE.into(),
            &[
                ("function_declaration", "function"),
                ("function_definition", "function"),
            ],
        )),
        Lang::Bash => Some((
            tree_sitter_bash::LANGUAGE.into(),
            &[("function_definition", "function")],
        )),
        Lang::Unknown => None,
    }
}

fn walk(
    node: Node,
    bytes: &[u8],
    source: &str,
    kinds: &'static [(&'static str, &'static str)],
    out: &mut Vec<ParsedSymbol>,
) {
    let kind = node.kind();
    if let Some((_, label)) = kinds.iter().find(|(k, _)| *k == kind) {
        if let Some(symbol) = make_symbol(node, bytes, source, label) {
            out.push(symbol);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, bytes, source, kinds, out);
    }
}

fn make_symbol(
    node: Node,
    bytes: &[u8],
    source: &str,
    kind: &'static str,
) -> Option<ParsedSymbol> {
    let start = node.start_position();
    let end = node.end_position();
    // 1-indexed lines for display
    let line = start.row + 1;
    let end_line = end.row + 1;
    let signature_line = source.lines().nth(start.row)?.trim_start();
    let signature = clean_signature(signature_line);
    if signature.is_empty() {
        return None;
    }
    let indent_chars = source
        .lines()
        .nth(start.row)
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);
    // Defensive: ensure we can re-slice the start byte. If not, something
    // went sideways and we drop the symbol rather than emit garbage.
    let _ = bytes.get(node.start_byte()..node.start_byte().saturating_add(1))?;
    Some(ParsedSymbol {
        line,
        end_line,
        indent: indent_chars,
        kind,
        signature,
    })
}

fn clean_signature(line: &str) -> String {
    let without_body = line.split('{').next().unwrap_or(line).trim();
    let without_colon = without_body.trim_end_matches(':').trim();
    let chars: String = without_colon.chars().take(140).collect();
    if without_colon.chars().count() > 140 {
        format!("{chars}…")
    } else {
        chars
    }
}

/// Walks the tree counting bytes covered by ERROR / MISSING nodes and
/// returns the fraction of total source they represent.
fn error_ratio(root: Node, source_len: usize) -> f64 {
    if source_len == 0 {
        return 0.0;
    }
    let mut error_bytes: usize = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            error_bytes = error_bytes.saturating_add(
                node.end_byte().saturating_sub(node.start_byte()),
            );
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    error_bytes as f64 / source_len as f64
}
