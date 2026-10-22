//! Buffer-sourced [`Collection`]s for data-driven repeats.
//!
//! The expansion engine resolves a data-driven repeat (`arm*@Msg`, or a bare
//! `arm*` under `match:Msg`) purely by *name*: it looks the name up in
//! [`ExpansionContext::collections`](codemmet_core::context::ExpansionContext).
//! So this module's whole job is to gather every named collection it can see in
//! the buffer and key it by that name:
//!
//! * **enum variants** (CX2) — keyed by the enum/type name (`Msg` → its
//!   variants). Visible file-wide, since a definition can sit below its use.
//!   A variant's value is spelled the way a *use site* needs it — qualified
//!   where the language requires it (`Msg::Quit`, `Color.RED`, `Color.Red`,
//!   a C++ `enum class`'s `Color::Red`), bare where it does not (C, Go) —
//!   and the `name` member always carries the bare name. A few well-known
//!   types the language defines elsewhere (`Option`, `Result`, `bool`, …)
//!   are supplied as [builtin collections](builtin_enums) when the file does
//!   not define a type of the same name.
//! * **struct/record fields** (CX3) — keyed by the type name (`Point` → its
//!   fields), each field carrying a `type` member for `${value.type}`. Also
//!   file-wide.
//!
//! Beyond `type`, extraction fills the blessed member vocabulary where the
//! grammar carries the data (IM1, see `ZIPPED_COLLECTIONS_PLAN.md`): `vis`
//! (Rust field visibility), `optional` (TS `?`), `default` (Python field
//! initializers), `value` (C/TS enum assigned values, Python enum members),
//! and `name` (a Rust variant's bare name, since its item value is the
//! match-ready pattern). Members that render as optional prefixes/suffixes
//! are **display-ready** — they carry their own spacing and punctuation
//! (`vis` = `"pub "`, `optional` = `"?"`) — so bodies can join them without
//! conditionals and an absent member simply vanishes.
//! * **in-scope list bindings** (CX1) — keyed by the variable name
//!   (`items` → its elements). Only bindings lexically in scope at the cursor
//!   and declared textually before it are offered; on a name clash the nearest
//!   enclosing binding wins.
//!
//! Extraction is deliberately shape-directed rather than type-aware: it reads
//! what the syntax tree literally says (an enum's written variants, a list
//! literal's written elements). Inferring a variable's *type* to find its
//! definition is out of scope — the user names the source instead
//! (`arm*@Msg`). See `CONTEXT_CODEGEN_PLAN.md`.

use std::collections::BTreeMap;

use codemmet_core::context::{Collection, Item};
use tree_sitter::Node;

/// Gather every named collection visible at `offset` in `root`: the file-wide
/// definitions plus the list bindings in scope at the cursor.
pub fn collections(
    language: &str,
    root: Node<'_>,
    text: &str,
    offset: usize,
) -> BTreeMap<String, Collection> {
    let mut map = file_collections(language, root, text);
    add_list_bindings(language, root, text, offset, &mut map);
    map
}

/// The file-wide collections `root` defines, independent of any cursor: enum
/// variants, struct fields, and the `src.*` API-surface collections (see
/// [`crate::source`]). This is what an explicit `--source` file contributes.
pub fn file_collections(
    language: &str,
    root: Node<'_>,
    text: &str,
) -> BTreeMap<String, Collection> {
    let mut map = enums(language, root, text);
    match language {
        "rust" => rust_structs(root, text, &mut map),
        "c" | "cpp" => c_structs(root, text, &mut map),
        "typescript" => ts_interfaces(root, text, &mut map),
        "go" => go_structs(root, text, &mut map),
        "python" => python_dataclasses(root, text, &mut map),
        // JavaScript has no enum/struct definitions; only list bindings apply.
        _ => {}
    }
    for (name, items) in builtin_enums(language) {
        map.entry(name.to_owned())
            .or_insert_with(|| Collection { items });
    }
    crate::source::definitions(language, root, text, &mut map);
    map
}

/// The enum-like definitions of `root` only — enum variants, iota groups,
/// literal unions, `Enum` subclasses — keyed by type name. This is what the
/// "fill from an enum" code actions choose from (see [`crate::codegen`]);
/// [`file_collections`] adds the structs and builtins on top.
pub fn enums(language: &str, root: Node<'_>, text: &str) -> BTreeMap<String, Collection> {
    let mut map = BTreeMap::new();
    match language {
        "rust" => rust_enums(root, text, &mut map),
        "c" | "cpp" => c_enums(root, text, &mut map),
        "typescript" => ts_enums(root, text, &mut map),
        "go" => go_iota_groups(root, text, &mut map),
        "python" => python_enums(root, text, &mut map),
        _ => {}
    }
    map
}

/// Well-known enum-like types a language defines outside any file, as
/// `(name, variants)`: the things one matches on constantly without ever
/// seeing their definition. Each variant is spelled as a use site needs it,
/// with the bare name as its `name` member. Rust: `Option` (`Some(..)`,
/// `None`), `Result` (`Ok(..)`, `Err(..)`), `Ordering` (`Ordering::Less`, …)
/// and `bool`; Python: `bool` (`True`, `False`); C, C++, Go, JavaScript and
/// TypeScript: `bool` (`true`, `false`), which TypeScript also calls
/// `boolean`. A file's own definition of the same name wins.
#[must_use]
pub fn builtin_enums(language: &str) -> Vec<(&'static str, Vec<Item>)> {
    let variants = |specs: &[(&str, &str)]| -> Vec<Item> {
        specs
            .iter()
            .map(|(value, name)| {
                let mut item = Item::new(*value);
                item.members.insert("name".to_owned(), (*name).to_owned());
                item
            })
            .collect()
    };
    match language {
        "rust" => vec![
            (
                "Option",
                variants(&[("Some(..)", "Some"), ("None", "None")]),
            ),
            ("Result", variants(&[("Ok(..)", "Ok"), ("Err(..)", "Err")])),
            (
                "Ordering",
                variants(&[
                    ("Ordering::Less", "Less"),
                    ("Ordering::Equal", "Equal"),
                    ("Ordering::Greater", "Greater"),
                ]),
            ),
            ("bool", variants(&[("true", "true"), ("false", "false")])),
        ],
        "python" => vec![("bool", variants(&[("True", "True"), ("False", "False")]))],
        "typescript" => vec![
            ("bool", variants(&[("true", "true"), ("false", "false")])),
            ("boolean", variants(&[("true", "true"), ("false", "false")])),
        ],
        "c" | "cpp" | "go" | "javascript" => {
            vec![("bool", variants(&[("true", "true"), ("false", "false")]))]
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Source text a node spans.
pub fn node_text<'a>(node: Node<'_>, text: &'a str) -> &'a str {
    &text[node.byte_range()]
}

/// Text of a node's named field, if present.
pub fn field_text<'a>(node: Node<'_>, field: &str, text: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field).map(|c| node_text(c, text))
}

/// Every named descendant of `root` (inclusive) whose kind is in `kinds`, in
/// document (pre-)order.
pub fn descendants_of<'t>(root: Node<'t>, kinds: &[&str]) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    collect_descendants(root, kinds, &mut out);
    out
}

fn collect_descendants<'t>(node: Node<'t>, kinds: &[&str], out: &mut Vec<Node<'t>>) {
    if node.is_named() && kinds.contains(&node.kind()) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_descendants(child, kinds, out);
    }
}

/// Named children of a node's field (empty when the field is absent).
fn field_children<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    let Some(container) = node.child_by_field_name(field) else {
        return Vec::new();
    };
    let mut cursor = container.walk();
    container.named_children(&mut cursor).collect()
}

/// A field's declared type, cleaned for display: a leading `:` (TypeScript's
/// `type_annotation` includes the colon) and surrounding whitespace stripped.
pub fn clean_type(raw: &str) -> String {
    raw.trim().trim_start_matches(':').trim().to_owned()
}

/// Insert a collection under `name` unless a definition already claimed it or
/// the item list is empty. File-wide definitions overwrite; scoped list
/// bindings use [`BTreeMap::entry`] so the nearest one wins.
fn insert_def(map: &mut BTreeMap<String, Collection>, name: &str, items: Vec<Item>) {
    if !items.is_empty() {
        map.insert(name.to_owned(), Collection { items });
    }
}

// ---------------------------------------------------------------------------
// Enum variants (CX2)
// ---------------------------------------------------------------------------

/// Enum-like definitions whose members are the direct named children of a
/// container field. A member's bare name is its `name` field when it has one
/// (C `enumerator`, TS `enum_assignment`), else the member's own text (a
/// bare TS enum member, a union `literal_type`). The item's value is that
/// name joined to the enum's name with the separator `qualify` returns for
/// the definition (`Some(".")` → `Color.Red`; `None` → bare), and the bare
/// name is kept as the `name` member. An assigned value, when the grammar
/// exposes one (`GREEN = 2` — a C enumerator's or TS enum assignment's
/// `value` field), is carried as the item's `value` member for
/// `${value.value}`.
fn extract_enum(
    root: Node<'_>,
    text: &str,
    def_kind: &str,
    container_field: &str,
    member_kinds: &[&str],
    qualify: &dyn Fn(Node<'_>) -> Option<&'static str>,
    map: &mut BTreeMap<String, Collection>,
) {
    for def in descendants_of(root, &[def_kind]) {
        let Some(name) = field_text(def, "name", text) else {
            continue;
        };
        let separator = qualify(def);
        let items = field_children(def, container_field)
            .into_iter()
            .filter(|m| member_kinds.contains(&m.kind()))
            .map(|m| {
                let bare = field_text(m, "name", text).unwrap_or_else(|| node_text(m, text));
                let mut item = Item::new(
                    separator.map_or_else(|| bare.to_owned(), |sep| format!("{name}{sep}{bare}")),
                );
                if separator.is_some() {
                    item.members.insert("name".to_owned(), bare.to_owned());
                }
                if let Some(value) = field_text(m, "value", text) {
                    item.members
                        .insert("value".to_owned(), value.trim().to_owned());
                }
                item
            })
            .collect();
        insert_def(map, name, items);
    }
}

// ---------------------------------------------------------------------------
// Per-language definition passes (enums + structs)
// ---------------------------------------------------------------------------

fn rust_enums(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    // Rust variants can carry a payload, so the item value is the match-ready
    // *qualified* pattern — `Msg::Quit`, `Msg::Write(..)`, `Msg::Move { .. }`
    // — so `match:Msg>arm*` produces arms that actually compile (a bare
    // `Quit` would be a catch-all binding). The bare name is still reachable
    // as the `name` member (`${value.name}`).
    for def in descendants_of(root, &["enum_item"]) {
        let Some(name) = field_text(def, "name", text) else {
            continue;
        };
        let items = field_children(def, "body")
            .into_iter()
            .filter(|v| v.kind() == "enum_variant")
            .filter_map(|v| {
                let bare = field_text(v, "name", text)?;
                let pattern = rust_variant_pattern(v, text)?;
                let mut item = Item::new(format!("{name}::{pattern}"));
                item.members.insert("name".to_owned(), bare.to_owned());
                Some(item)
            })
            .collect();
        insert_def(map, name, items);
    }
}

fn rust_structs(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    for def in descendants_of(root, &["struct_item"]) {
        let Some(name) = field_text(def, "name", text) else {
            continue;
        };
        let items = field_children(def, "body")
            .into_iter()
            .filter(|f| f.kind() == "field_declaration")
            .filter_map(|f| named_field_item(f, text))
            .collect();
        insert_def(map, name, items);
    }
}

fn ts_enums(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    // `enum Color { Red }` is used as `Color.Red`.
    extract_enum(
        root,
        text,
        "enum_declaration",
        "body",
        &["property_identifier", "enum_assignment"],
        &|_| Some("."),
        map,
    );
    // `type Dir = "n" | "s"` — a union of literals reads as an enum; its
    // members are the literals themselves.
    extract_enum(
        root,
        text,
        "type_alias_declaration",
        "value",
        &["literal_type"],
        &|_| None,
        map,
    );
}

fn ts_interfaces(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    for def in descendants_of(root, &["interface_declaration"]) {
        let Some(name) = field_text(def, "name", text) else {
            continue;
        };
        let items = field_children(def, "body")
            .into_iter()
            .filter(|f| f.kind() == "property_signature")
            .filter_map(|f| named_field_item(f, text))
            .collect();
        insert_def(map, name, items);
    }
}

fn c_enums(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    // A plain `enum Color { RED }` puts `RED` in the enclosing scope; a C++
    // `enum class Color { Red }` scopes it as `Color::Red`.
    extract_enum(
        root,
        text,
        "enum_specifier",
        "body",
        &["enumerator"],
        &|def| {
            let mut cursor = def.walk();
            def.children(&mut cursor)
                .any(|c| matches!(c.kind(), "class" | "struct"))
                .then_some("::")
        },
        map,
    );
}

fn c_structs(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    for def in descendants_of(root, &["struct_specifier", "union_specifier"]) {
        let Some(name) = field_text(def, "name", text) else {
            continue;
        };
        let items = field_children(def, "body")
            .into_iter()
            .filter(|f| f.kind() == "field_declaration")
            .filter_map(|f| c_field_item(f, text))
            .collect();
        insert_def(map, name, items);
    }
}

fn go_iota_groups(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    // `const ( Red Color = iota; Green; Blue )` — an iota const group is Go's
    // enum. It is keyed by the declared type of its **first** spec (an
    // untyped group has no name to key by and is skipped); the items are
    // every spec's names, in order — bare, as Go spells them.
    for decl in descendants_of(root, &["const_declaration"]) {
        let specs = descendants_of(decl, &["const_spec"]);
        let Some(ty) = specs.first().and_then(|s| field_text(*s, "type", text)) else {
            continue;
        };
        let uses_iota = specs.iter().any(|s| {
            s.child_by_field_name("value").is_some_and(|v| {
                node_text(v, text)
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|token| token == "iota")
            })
        });
        if !uses_iota {
            continue;
        }
        let mut items = Vec::new();
        for spec in &specs {
            let mut cursor = spec.walk();
            for name in spec.children_by_field_name("name", &mut cursor) {
                items.push(Item::new(node_text(name, text)));
            }
        }
        insert_def(map, &clean_type(ty), items);
    }
}

fn go_structs(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    // Go structs live under `type_spec name: … type: (struct_type …)`.
    for spec in descendants_of(root, &["type_spec"]) {
        let Some(name) = field_text(spec, "name", text) else {
            continue;
        };
        let Some(ty) = spec.child_by_field_name("type") else {
            continue;
        };
        if ty.kind() != "struct_type" {
            continue;
        }
        let mut items = Vec::new();
        for field in descendants_of(ty, &["field_declaration"]) {
            let type_text = field_text(field, "type", text).map(clean_type);
            // A Go field declaration can name several fields at once
            // (`X, Y int`); each `name` child becomes its own item.
            let mut cursor = field.walk();
            for child in field.children_by_field_name("name", &mut cursor) {
                let mut item = Item::new(node_text(child, text));
                if let Some(t) = &type_text {
                    item.members.insert("type".to_owned(), t.clone());
                }
                items.push(item);
            }
        }
        insert_def(map, name, items);
    }
}

fn python_enums(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    python_classes(root, text, true, map);
}

fn python_dataclasses(root: Node<'_>, text: &str, map: &mut BTreeMap<String, Collection>) {
    python_classes(root, text, false, map);
}

/// Python classes of one kind: `Enum` subclasses (`enums`), whose members
/// are their assigned names spelled `Color.RED` (bare name in `name`), or
/// every other class (dataclasses, plain classes), whose fields are their
/// annotated attributes.
fn python_classes(root: Node<'_>, text: &str, enums: bool, map: &mut BTreeMap<String, Collection>) {
    for class in descendants_of(root, &["class_definition"]) {
        let Some(name) = field_text(class, "name", text) else {
            continue;
        };
        let is_enum = field_children(class, "superclasses")
            .iter()
            .any(|s| node_text(*s, text).ends_with("Enum"));
        if is_enum != enums {
            continue;
        }
        let mut items = Vec::new();
        for stmt in field_children(class, "body") {
            if stmt.kind() != "expression_statement" {
                continue;
            }
            let Some(assign) = stmt.named_child(0).filter(|a| a.kind() == "assignment") else {
                continue;
            };
            let Some(target) = assign.child_by_field_name("left") else {
                continue;
            };
            if target.kind() != "identifier" {
                continue;
            }
            // An enum's members are its assigned names; a dataclass's fields
            // are its *annotated* attributes (`x: int`). A plain assignment in
            // a dataclass (a default without annotation) is skipped.
            let annotated = assign.child_by_field_name("type");
            if !is_enum && annotated.is_none() {
                continue;
            }
            let bare = node_text(target, text);
            let mut item = if is_enum {
                let mut item = Item::new(format!("{name}.{bare}"));
                item.members.insert("name".to_owned(), bare.to_owned());
                item
            } else {
                Item::new(bare)
            };
            if let Some(ty) = annotated {
                item.members
                    .insert("type".to_owned(), clean_type(node_text(ty, text)));
            }
            // The assignment's right side: an enum member's assigned value
            // (`RED = 1` → `value`), or an annotated field's initializer
            // (`x: int = 0` → `default`).
            if let Some(rhs) = assign.child_by_field_name("right") {
                let key = if is_enum { "value" } else { "default" };
                item.members
                    .insert(key.to_owned(), node_text(rhs, text).trim().to_owned());
            }
            items.push(item);
        }
        insert_def(map, name, items);
    }
}

/// A Rust `enum_variant`'s match pattern: the bare name for a unit variant, a
/// wildcard binding for a payload variant (`Write(..)` for a tuple variant,
/// `Move { .. }` for a struct variant). This is minimal by design — it binds
/// nothing, so the arm compiles without naming each field.
fn rust_variant_pattern(variant: Node<'_>, text: &str) -> Option<String> {
    let name = field_text(variant, "name", text)?;
    Some(
        match variant.child_by_field_name("body").map(|b| b.kind()) {
            Some("ordered_field_declaration_list") => format!("{name}(..)"),
            Some("field_declaration_list") => format!("{name} {{ .. }}"),
            _ => name.to_owned(),
        },
    )
}

/// A struct field whose name is in a `name` field and type in a `type` field
/// (Rust, Go, TypeScript). The `type` member is omitted when absent. Two
/// further display-ready members are captured where the grammar carries them
/// (each is a prefix/suffix fragment that renders cleanly next to the name,
/// and simply vanishes when absent):
///
/// * `vis` — a Rust `visibility_modifier` (`pub`, `pub(crate)`), **with a
///   trailing space**, so `${value.vis}${value}` reads `pub x` / `x`;
/// * `optional` — a TS `property_signature`'s `?` marker, so
///   `${value}${value.optional}` reads `x?` / `x`.
fn named_field_item(field: Node<'_>, text: &str) -> Option<Item> {
    let name = field_text(field, "name", text)?;
    let mut item = Item::new(name);
    if let Some(ty) = field_text(field, "type", text) {
        item.members.insert("type".to_owned(), clean_type(ty));
    }
    let mut cursor = field.walk();
    for child in field.children(&mut cursor) {
        match child.kind() {
            "visibility_modifier" => {
                item.members
                    .insert("vis".to_owned(), format!("{} ", node_text(child, text)));
            }
            "?" => {
                item.members.insert("optional".to_owned(), "?".to_owned());
            }
            _ => {}
        }
    }
    Some(item)
}

/// A C/C++ struct field: the name lives in a `declarator` (a bare
/// `field_identifier`, or nested inside a pointer/array declarator), the type
/// in a `type` field.
fn c_field_item(field: Node<'_>, text: &str) -> Option<Item> {
    let declarator = field.child_by_field_name("declarator")?;
    let ident = if declarator.kind() == "field_identifier" {
        declarator
    } else {
        descendants_of(declarator, &["field_identifier"])
            .into_iter()
            .next()?
    };
    let mut item = Item::new(node_text(ident, text));
    if let Some(ty) = field_text(field, "type", text) {
        item.members.insert("type".to_owned(), clean_type(ty));
    }
    Some(item)
}

// ---------------------------------------------------------------------------
// In-scope list bindings (CX1)
// ---------------------------------------------------------------------------

/// Add list/array bindings that are lexically in scope at `offset`.
///
/// Walks enclosing scopes from the cursor outward; within each, any statement
/// starting before the cursor that binds a list literal contributes a
/// collection. The nearest binding wins a name clash (scopes are visited
/// innermost first and [`BTreeMap::entry`] keeps the first insert).
fn add_list_bindings(
    language: &str,
    root: Node<'_>,
    text: &str,
    offset: usize,
    map: &mut BTreeMap<String, Collection>,
) {
    let mut scope = root.named_descendant_for_byte_range(offset, offset);
    while let Some(node) = scope {
        for stmt in scope_statements(node) {
            if stmt.start_byte() >= offset {
                continue;
            }
            for (name, items) in list_bindings(language, stmt, text) {
                if !items.is_empty() && !map.contains_key(&name) {
                    map.insert(name, Collection { items });
                }
            }
        }
        scope = node.parent();
    }
}

/// The statement nodes directly inside a scope, flattening grouping wrappers
/// that are not themselves scopes (Go nests statements under an extra
/// `statement_list` inside each `block`).
fn scope_statements(node: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "statement_list" {
            let mut inner = child.walk();
            out.extend(child.named_children(&mut inner));
        } else {
            out.push(child);
        }
    }
    out
}

/// The list bindings a single statement introduces, as `(name, elements)`.
fn list_bindings(language: &str, stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    match language {
        "rust" => rust_list(stmt, text),
        "python" => python_list(stmt, text),
        "javascript" | "typescript" => js_list(stmt, text),
        "go" => go_list(stmt, text),
        "c" | "cpp" => c_list(stmt, text),
        _ => Vec::new(),
    }
}

/// Elements of a literal-container node, each as an [`Item`].
fn elements(container: Node<'_>, text: &str) -> Vec<Item> {
    let mut cursor = container.walk();
    container
        .named_children(&mut cursor)
        .map(|e| Item::new(node_text(e, text)))
        .collect()
}

fn rust_list(stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    if stmt.kind() != "let_declaration" {
        return Vec::new();
    }
    let Some(pattern) = stmt.child_by_field_name("pattern") else {
        return Vec::new();
    };
    if pattern.kind() != "identifier" {
        return Vec::new();
    }
    let name = node_text(pattern, text).to_owned();
    let Some(value) = stmt.child_by_field_name("value") else {
        return Vec::new();
    };
    match value.kind() {
        "array_expression" => vec![(name, elements(value, text))],
        // `vec![a, b, c]` — the elements are the named children of the macro's
        // token tree (commas are anonymous tokens).
        "macro_invocation" if field_text(value, "macro", text) == Some("vec") => value
            .named_children(&mut value.walk())
            .find(|c| c.kind() == "token_tree")
            .map_or_else(Vec::new, |tt| vec![(name.clone(), elements(tt, text))]),
        _ => Vec::new(),
    }
}

fn python_list(stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    if stmt.kind() != "expression_statement" {
        return Vec::new();
    }
    let Some(assign) = stmt.named_child(0).filter(|a| a.kind() == "assignment") else {
        return Vec::new();
    };
    let (Some(left), Some(right)) = (
        assign.child_by_field_name("left"),
        assign.child_by_field_name("right"),
    ) else {
        return Vec::new();
    };
    if left.kind() != "identifier" || right.kind() != "list" {
        return Vec::new();
    }
    vec![(node_text(left, text).to_owned(), elements(right, text))]
}

fn js_list(stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    if !matches!(stmt.kind(), "lexical_declaration" | "variable_declaration") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for decl in descendants_of(stmt, &["variable_declarator"]) {
        let (Some(name), Some(value)) = (
            decl.child_by_field_name("name"),
            decl.child_by_field_name("value"),
        ) else {
            continue;
        };
        if name.kind() == "identifier" && value.kind() == "array" {
            out.push((node_text(name, text).to_owned(), elements(value, text)));
        }
    }
    out
}

fn go_list(stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    // Both `var items = []T{…}` and `items := []T{…}`.
    let specs = match stmt.kind() {
        "var_declaration" => descendants_of(stmt, &["var_spec"]),
        "short_var_declaration" => vec![stmt],
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for spec in specs {
        // `var_spec` names via a `name` field; `:=` names via a `left`
        // expression list of identifiers.
        let names: Vec<Node<'_>> = if spec.kind() == "short_var_declaration" {
            spec.child_by_field_name("left")
                .map(|l| l.named_children(&mut l.walk()).collect())
                .unwrap_or_default()
        } else {
            spec.children_by_field_name("name", &mut spec.walk())
                .collect()
        };
        let value_field = if spec.kind() == "short_var_declaration" {
            "right"
        } else {
            "value"
        };
        let Some(value) = spec.child_by_field_name(value_field) else {
            continue;
        };
        // The value is an expression list; take its first composite literal.
        let Some(composite) = descendants_of(value, &["composite_literal"])
            .into_iter()
            .next()
        else {
            continue;
        };
        let Some(body) = composite.child_by_field_name("body") else {
            continue;
        };
        if let Some(name) = names.first() {
            out.push((node_text(*name, text).to_owned(), elements(body, text)));
        }
    }
    out
}

fn c_list(stmt: Node<'_>, text: &str) -> Vec<(String, Vec<Item>)> {
    if stmt.kind() != "declaration" {
        return Vec::new();
    }
    let mut out = Vec::new();
    for init in descendants_of(stmt, &["init_declarator"]) {
        let (Some(declarator), Some(value)) = (
            init.child_by_field_name("declarator"),
            init.child_by_field_name("value"),
        ) else {
            continue;
        };
        if value.kind() != "initializer_list" {
            continue;
        }
        // The bound name is the identifier inside the (possibly array)
        // declarator.
        let ident = if declarator.kind() == "identifier" {
            Some(declarator)
        } else {
            descendants_of(declarator, &["identifier"])
                .into_iter()
                .next()
        };
        if let Some(ident) = ident {
            out.push((node_text(ident, text).to_owned(), elements(value, text)));
        }
    }
    out
}
