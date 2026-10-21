//! Source-level definitions as collections: the functions, types, and methods
//! a file declares (see `SOURCE_COLLECTIONS_PLAN.md`).
//!
//! Where [`extract`](crate::extract) reads *data shapes* (enum variants,
//! struct fields, list literals), this module reads the *API surface* of a
//! file so a template can repeat over it — one test per function, one block
//! per type. Four collections, all under the reserved `src.` prefix so they
//! never collide with a user's own type names:
//!
//! | collection            | items                                   |
//! | --------------------- | --------------------------------------- |
//! | `src.fns`             | free functions (top level or in modules) |
//! | `src.types`           | classes, structs, enums, traits, …      |
//! | `src.methods`         | every method, with an `owner` member    |
//! | `src.<Type>.methods`  | the methods of one type                 |
//!
//! Every item's value is the bare name. The members are a fixed vocabulary
//! (`params`, `args`, `ret`, `vis`, `async`, `static`, `receiver`, `owner`,
//! `trait`, `kind`, `test`, `ctor`, `module`), documented in the plan and in
//! `docs/context-codegen.md`. Collections are **complete** — private and
//! test functions included; narrowing is the repeat filter's job.
//!
//! Extraction is shape-directed: it reads what the syntax literally says and
//! never resolves types or imports. Overloads (C++) collapse onto one item;
//! an out-of-class definition merges into its in-class declaration.

use std::collections::BTreeMap;

use codemmet_core::context::{Collection, Item};
use tree_sitter::Node;

use crate::extract::{clean_type, descendants_of, field_text, node_text};

/// The free-function collection.
pub const FNS: &str = "src.fns";
/// The type collection.
pub const TYPES: &str = "src.types";
/// The flat method collection (every type's methods, `owner` says whose).
pub const METHODS: &str = "src.methods";

/// The per-type method collection name for `owner`.
#[must_use]
pub fn methods_key(owner: &str) -> String {
    format!("src.{owner}.methods")
}

/// Add every `src.*` collection `root` yields to `map`.
pub fn definitions(
    language: &str,
    root: Node<'_>,
    text: &str,
    map: &mut BTreeMap<String, Collection>,
) {
    let mut defs = Defs::default();
    match language {
        "rust" => rust::walk(root, text, None, &mut defs),
        "python" => python::walk(root, text, &mut defs),
        "c" | "cpp" => c::walk(root, text, None, &mut defs),
        "javascript" | "typescript" => js::walk(root, text, None, false, &mut defs),
        "go" => go::walk(root, text, &mut defs),
        _ => {}
    }
    defs.into_map(map);
}

// ---------------------------------------------------------------------------
// Accumulator
// ---------------------------------------------------------------------------

/// Normalized visibility, rendered as the `vis` member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vis {
    Public,
    Private,
    Protected,
}

impl Vis {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
        }
    }
}

/// One function-like definition before it becomes an [`Item`].
// The flags mirror the member vocabulary one-to-one; a bitset would obscure
// which member each one renders.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
struct Func {
    name: String,
    /// Each declared parameter as written (receiver removed).
    params: Vec<String>,
    /// The parameter names, for a call expression.
    args: Vec<String>,
    /// One item per parameter (value = name, members `type`/`default`), the
    /// `params` sub-collection a nested `*@outer.params` repeats over.
    param_items: Vec<Item>,
    ret: Option<String>,
    vis: Option<Vis>,
    is_async: bool,
    is_static: bool,
    receiver: Option<String>,
    owner: Option<String>,
    trait_name: Option<String>,
    is_test: bool,
    is_ctor: bool,
    module: Option<String>,
}

impl Func {
    fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }

    fn into_item(self) -> Item {
        let mut item = Item::new(self.name.clone());
        let m = &mut item.members;
        m.insert("name".to_owned(), self.name);
        if !self.params.is_empty() {
            m.insert("params".to_owned(), self.params.join(", "));
        }
        if !self.args.is_empty() {
            m.insert("args".to_owned(), self.args.join(", "));
        }
        if let Some(ret) = self.ret.filter(|r| !r.is_empty()) {
            m.insert("ret".to_owned(), ret);
        }
        if let Some(vis) = self.vis {
            m.insert("vis".to_owned(), vis.as_str().to_owned());
        }
        if self.is_async {
            m.insert("async".to_owned(), "async ".to_owned());
        }
        // Only a method can be static; a free function has no receiver to
        // lack.
        if self.is_static && self.owner.is_some() {
            m.insert("static".to_owned(), "static".to_owned());
        }
        if let Some(receiver) = self.receiver {
            m.insert("receiver".to_owned(), receiver);
        }
        if let Some(owner) = self.owner {
            m.insert("owner".to_owned(), owner);
        }
        if let Some(trait_name) = self.trait_name {
            m.insert("trait".to_owned(), trait_name);
        }
        if self.is_test {
            m.insert("test".to_owned(), "test".to_owned());
        }
        if self.is_ctor {
            m.insert("ctor".to_owned(), "ctor".to_owned());
        }
        if let Some(module) = self.module {
            m.insert("module".to_owned(), module);
        }
        if !self.param_items.is_empty() {
            item.collections.insert(
                "params".to_owned(),
                Collection {
                    items: self.param_items,
                },
            );
        }
        item
    }

    /// Record one parameter: its name for `args`, its full text for
    /// `params`, and an item (with `type` / `default` members when known) for
    /// the `params` sub-collection.
    fn param(&mut self, raw: &str, name: &str, ty: Option<&str>, default: Option<&str>) {
        if !raw.is_empty() {
            self.params.push(raw.to_owned());
        }
        self.args.push(name.to_owned());
        let mut item = Item::new(name);
        if let Some(ty) = ty.map(str::trim).filter(|t| !t.is_empty()) {
            item.members.insert("type".to_owned(), ty.to_owned());
        }
        if let Some(default) = default.map(str::trim).filter(|d| !d.is_empty()) {
            item.members
                .insert("default".to_owned(), default.to_owned());
        }
        self.param_items.push(item);
    }
}

/// A type-like definition.
fn type_item(name: &str, kind: &str, vis: Option<Vis>, module: Option<&str>) -> Item {
    let mut item = Item::new(name);
    item.members.insert("name".to_owned(), name.to_owned());
    item.members.insert("kind".to_owned(), kind.to_owned());
    if let Some(vis) = vis {
        item.members
            .insert("vis".to_owned(), vis.as_str().to_owned());
    }
    if let Some(module) = module {
        item.members.insert("module".to_owned(), module.to_owned());
    }
    item
}

/// Everything collected from one file, in source order.
#[derive(Debug, Default)]
struct Defs {
    fns: Vec<Item>,
    types: Vec<Item>,
    methods: Vec<Item>,
}

impl Defs {
    fn push_fn(&mut self, func: Func) {
        upsert(&mut self.fns, func.into_item(), |a, b| a.value == b.value);
    }

    fn push_type(&mut self, item: Item) {
        upsert(&mut self.types, item, |a, b| a.value == b.value);
    }

    fn push_method(&mut self, func: Func) {
        upsert(&mut self.methods, func.into_item(), |a, b| {
            a.value == b.value && a.members.get("owner") == b.members.get("owner")
        });
    }

    fn into_map(self, map: &mut BTreeMap<String, Collection>) {
        let mut by_owner: BTreeMap<String, Vec<Item>> = BTreeMap::new();
        for item in &self.methods {
            if let Some(owner) = item.members.get("owner") {
                by_owner
                    .entry(owner.clone())
                    .or_default()
                    .push(item.clone());
            }
        }
        for (owner, items) in by_owner {
            map.insert(methods_key(&owner), Collection { items });
        }
        for (key, items) in [
            (FNS, self.fns),
            (TYPES, self.types),
            (METHODS, self.methods),
        ] {
            if !items.is_empty() {
                map.insert(key.to_owned(), Collection { items });
            }
        }
    }
}

/// Insert `item`, or merge it into the entry `same` already matches: a later
/// definition fills in members its earlier declaration lacked, but the first
/// declaration keeps its position and its own members (a C++ in-class
/// declaration knows the access section; the out-of-class definition does
/// not).
fn upsert(list: &mut Vec<Item>, item: Item, same: impl Fn(&Item, &Item) -> bool) {
    if let Some(existing) = list.iter_mut().find(|e| same(e, &item)) {
        for (key, value) in item.members {
            existing.members.entry(key).or_insert(value);
        }
    } else {
        list.push(item);
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Whether `node` has an anonymous (keyword) child spelled `token`.
fn has_token(node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| !c.is_named() && c.kind() == token)
}

/// Whether `node` has a named child of kind `kind`.
fn has_child(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(|c| c.kind() == kind)
}

/// The named children of `node`, in order.
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Named children of a field, or empty when the field is absent.
fn field_nodes<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    node.child_by_field_name(field)
        .map_or_else(Vec::new, children)
}

/// Join a module path with a new segment using `sep`.
fn join_module(module: Option<&str>, name: &str, sep: &str) -> String {
    module.map_or_else(|| name.to_owned(), |m| format!("{m}{sep}{name}"))
}

/// The innermost identifier-like node inside a (possibly nested) declarator.
fn innermost_ident<'t>(node: Node<'t>, kinds: &[&str]) -> Option<Node<'t>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }
    descendants_of(node, kinds).into_iter().next()
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

mod rust {
    use super::{
        Defs, Func, Vis, children, field_nodes, field_text, has_child, join_module, node_text,
        type_item,
    };
    use tree_sitter::Node;

    /// Walk a `source_file` or module `declaration_list`.
    pub(super) fn walk(container: Node<'_>, text: &str, module: Option<&str>, defs: &mut Defs) {
        let mut attrs: Vec<Node<'_>> = Vec::new();
        for child in children(container) {
            match child.kind() {
                "attribute_item" => {
                    attrs.push(child);
                    continue;
                }
                "function_item" => {
                    let mut func = function(child, text, &attrs);
                    func.module = module.map(str::to_owned);
                    defs.push_fn(func);
                }
                "mod_item" => {
                    if let (Some(name), Some(body)) = (
                        field_text(child, "name", text),
                        child.child_by_field_name("body"),
                    ) {
                        let path = join_module(module, name, "::");
                        walk(body, text, Some(&path), defs);
                    }
                }
                "struct_item" | "enum_item" | "trait_item" | "union_item" | "type_item" => {
                    if let Some(name) = field_text(child, "name", text) {
                        let kind = child.kind().trim_end_matches("_item");
                        defs.push_type(type_item(name, kind, Some(vis_of(child)), module));
                    }
                }
                "impl_item" => impl_block(child, text, defs),
                _ => {}
            }
            attrs.clear();
        }
    }

    fn vis_of(node: Node<'_>) -> Vis {
        if has_child(node, "visibility_modifier") {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    /// The bare type name an `impl` targets: generics and paths stripped.
    fn self_type<'a>(node: Node<'_>, text: &'a str) -> Option<&'a str> {
        let mut ty = node.child_by_field_name("type")?;
        loop {
            match ty.kind() {
                "generic_type" | "reference_type" | "dynamic_type" => {
                    ty = ty.child_by_field_name("type")?;
                }
                "scoped_type_identifier" => ty = ty.child_by_field_name("name")?,
                _ => return Some(node_text(ty, text)),
            }
        }
    }

    fn impl_block(node: Node<'_>, text: &str, defs: &mut Defs) {
        let Some(owner) = self_type(node, text) else {
            return;
        };
        let trait_name = node.child_by_field_name("trait").map(|t| {
            // `fmt::Display` → `Display`
            let raw = node_text(t, text);
            raw.rsplit("::").next().unwrap_or(raw).to_owned()
        });
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let mut attrs: Vec<Node<'_>> = Vec::new();
        for child in children(body) {
            match child.kind() {
                "attribute_item" => {
                    attrs.push(child);
                    continue;
                }
                "function_item" => {
                    let mut func = function(child, text, &attrs);
                    func.owner = Some(owner.to_owned());
                    // A trait impl's methods are as visible as the trait.
                    if trait_name.is_some() {
                        func.vis = Some(Vis::Public);
                    }
                    func.trait_name.clone_from(&trait_name);
                    func.is_ctor = func.name == "new";
                    defs.push_method(func);
                }
                _ => {}
            }
            attrs.clear();
        }
    }

    fn function(node: Node<'_>, text: &str, attrs: &[Node<'_>]) -> Func {
        let mut func = Func::named(field_text(node, "name", text).unwrap_or_default());
        func.vis = Some(vis_of(node));
        func.is_async = children(node)
            .iter()
            .any(|c| c.kind() == "function_modifiers" && node_text(*c, text).contains("async"));
        let mut saw_receiver = false;
        for param in field_nodes(node, "parameters") {
            match param.kind() {
                "self_parameter" => {
                    saw_receiver = true;
                    func.receiver = Some(node_text(param, text).to_owned());
                }
                "parameter" => match field_text(param, "pattern", text) {
                    Some(pattern) => func.param(
                        node_text(param, text),
                        pattern,
                        field_text(param, "type", text),
                        None,
                    ),
                    None => func.params.push(node_text(param, text).to_owned()),
                },
                "variadic_parameter" => func.params.push(node_text(param, text).to_owned()),
                _ => {}
            }
        }
        func.is_static = !saw_receiver;
        func.ret = field_text(node, "return_type", text).map(str::to_owned);
        func.is_test = attrs.iter().any(|a| is_test_attribute(*a, text));
        func
    }

    /// `#[test]`, `#[tokio::test]`, `#[test_case(…)]`-style attributes: the
    /// path's last segment starts with `test`.
    fn is_test_attribute(attr: Node<'_>, text: &str) -> bool {
        let Some(inner) = attr.named_child(0) else {
            return false;
        };
        let raw = node_text(inner, text);
        let path = raw.split(['(', ' ', '=']).next().unwrap_or(raw);
        path.rsplit("::")
            .next()
            .is_some_and(|last| last.starts_with("test"))
    }
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

mod python {
    use super::{
        Defs, Func, Vis, children, field_nodes, field_text, has_token, node_text, type_item,
    };
    use tree_sitter::Node;

    /// Walk a `module` body.
    pub(super) fn walk(container: Node<'_>, text: &str, defs: &mut Defs) {
        for child in children(container) {
            let (node, decorators) = unwrap_decorated(child, text);
            match node.kind() {
                "function_definition" => {
                    defs.push_fn(function(node, text, &decorators, None));
                }
                "class_definition" => class(node, text, defs),
                _ => {}
            }
        }
    }

    /// A `decorated_definition` → its definition plus decorator names
    /// (`staticmethod`, `pytest.fixture`, …); any other node unchanged.
    fn unwrap_decorated<'t>(node: Node<'t>, text: &str) -> (Node<'t>, Vec<String>) {
        if node.kind() != "decorated_definition" {
            return (node, Vec::new());
        }
        let decorators = children(node)
            .into_iter()
            .filter(|c| c.kind() == "decorator")
            .map(|d| {
                let raw = node_text(d, text).trim_start_matches('@');
                raw.split('(').next().unwrap_or(raw).trim().to_owned()
            })
            .collect();
        let def = node.child_by_field_name("definition").unwrap_or(node);
        (def, decorators)
    }

    fn class(node: Node<'_>, text: &str, defs: &mut Defs) {
        let Some(name) = field_text(node, "name", text) else {
            return;
        };
        defs.push_type(type_item(name, "class", Some(vis_of(name)), None));
        for child in field_nodes(node, "body") {
            let (member, decorators) = unwrap_decorated(child, text);
            match member.kind() {
                "function_definition" => {
                    let mut func = function(member, text, &decorators, Some(name));
                    func.is_ctor = func.name == "__init__";
                    defs.push_method(func);
                }
                // A nested class is a type of its own.
                "class_definition" => class(member, text, defs),
                _ => {}
            }
        }
    }

    /// A leading underscore is private by convention; dunders (`__init__`)
    /// are protocol methods, hence public.
    fn vis_of(name: &str) -> Vis {
        let dunder = name.len() > 4 && name.starts_with("__") && name.ends_with("__");
        if name.starts_with('_') && !dunder {
            Vis::Private
        } else {
            Vis::Public
        }
    }

    fn function(node: Node<'_>, text: &str, decorators: &[String], owner: Option<&str>) -> Func {
        let mut func = Func::named(field_text(node, "name", text).unwrap_or_default());
        func.vis = Some(vis_of(&func.name));
        func.is_async = has_token(node, "async");
        func.owner = owner.map(str::to_owned);
        func.is_static = decorators.iter().any(|d| d == "staticmethod");
        let mut first = true;
        for param in field_nodes(node, "parameters") {
            let raw = node_text(param, text);
            let (name, ty, default): (String, Option<&str>, Option<&str>) = match param.kind() {
                "identifier" | "list_splat_pattern" | "dictionary_splat_pattern" => {
                    (raw.to_owned(), None, None)
                }
                "typed_parameter" => (
                    param
                        .named_child(0)
                        .map_or_else(String::new, |n| node_text(n, text).to_owned()),
                    field_text(param, "type", text),
                    None,
                ),
                "default_parameter" | "typed_default_parameter" => (
                    field_text(param, "name", text)
                        .unwrap_or_default()
                        .to_owned(),
                    field_text(param, "type", text),
                    field_text(param, "value", text),
                ),
                // `*` and `/` separators carry no argument.
                _ => continue,
            };
            // A method's leading `self`/`cls` is the receiver, not a parameter.
            if first
                && owner.is_some()
                && !func.is_static
                && matches!(name.as_str(), "self" | "cls")
            {
                func.receiver = Some(name);
                first = false;
                continue;
            }
            first = false;
            func.param(raw, &name, ty, default);
        }
        func.ret = field_text(node, "return_type", text).map(str::to_owned);
        func.is_test = func.name.starts_with("test");
        func
    }
}

// ---------------------------------------------------------------------------
// C / C++
// ---------------------------------------------------------------------------

mod c {
    use super::{
        Defs, Func, Vis, children, field_nodes, field_text, innermost_ident, join_module,
        node_text, type_item,
    };
    use tree_sitter::Node;

    /// Walk a translation unit, namespace body, or preprocessor block.
    pub(super) fn walk(container: Node<'_>, text: &str, module: Option<&str>, defs: &mut Defs) {
        for child in children(container) {
            visit(child, text, module, defs);
        }
    }

    fn visit(node: Node<'_>, text: &str, module: Option<&str>, defs: &mut Defs) {
        match node.kind() {
            // A definition, or a prototype: `int area(int w, int h);`
            "function_definition" | "declaration" => {
                if let Some(func) = function(node, text, Vis::Public) {
                    place(func, module, defs);
                }
            }
            "template_declaration" => {
                for child in children(node) {
                    visit(child, text, module, defs);
                }
            }
            "namespace_definition" => {
                let name = field_text(node, "name", text);
                let path = name.map(|n| join_module(module, n, "::"));
                if let Some(body) = node.child_by_field_name("body") {
                    walk(body, text, path.as_deref().or(module), defs);
                }
            }
            "preproc_ifdef" | "preproc_if" | "preproc_else" | "preproc_elif" => {
                walk(node, text, module, defs);
            }
            "linkage_specification" => {
                if let Some(body) = node.child_by_field_name("body") {
                    walk(body, text, module, defs);
                }
            }
            "struct_specifier" | "class_specifier" | "union_specifier" | "enum_specifier" => {
                // Forward declarations have no body.
                if let (Some(name), Some(body)) = (
                    field_text(node, "name", text),
                    node.child_by_field_name("body"),
                ) {
                    let kind = node.kind().trim_end_matches("_specifier");
                    defs.push_type(type_item(name, kind, None, module));
                    if kind != "enum" {
                        class_body(body, text, name, kind == "class", defs);
                    }
                }
            }
            "type_definition" => {
                if let Some(decl) = node.child_by_field_name("declarator") {
                    defs.push_type(type_item(node_text(decl, text), "type", None, module));
                }
            }
            _ => {}
        }
    }

    /// A free function joins `src.fns`; a `Type::name` out-of-class
    /// definition joins that type's methods.
    fn place(mut func: Func, module: Option<&str>, defs: &mut Defs) {
        if func.owner.is_some() {
            // Access is known only at the in-class declaration; let the
            // declaration's `vis` win on merge.
            func.vis = None;
            defs.push_method(func);
        } else {
            func.module = module.map(str::to_owned);
            defs.push_fn(func);
        }
    }

    /// Members of a `struct`/`class` body, tracking access sections.
    fn class_body(body: Node<'_>, text: &str, owner: &str, is_class: bool, defs: &mut Defs) {
        let mut access = if is_class { Vis::Private } else { Vis::Public };
        for child in children(body) {
            match child.kind() {
                "access_specifier" => {
                    access = match node_text(child, text) {
                        "public" => Vis::Public,
                        "protected" => Vis::Protected,
                        _ => Vis::Private,
                    };
                }
                "field_declaration" | "function_definition" | "declaration" => {
                    if let Some(mut func) = function(child, text, access) {
                        func.owner = Some(owner.to_owned());
                        func.is_ctor = func.name == owner;
                        defs.push_method(func);
                    }
                }
                "template_declaration" => {
                    for inner in children(child) {
                        if let Some(mut func) = function(inner, text, access) {
                            func.owner = Some(owner.to_owned());
                            defs.push_method(func);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// A function from a definition, prototype, or member declaration — or
    /// `None` when the node's declarator is not a function (a plain field).
    fn function(node: Node<'_>, text: &str, access: Vis) -> Option<Func> {
        let mut declarator = node.child_by_field_name("declarator")?;
        let mut pointers = 0;
        loop {
            match declarator.kind() {
                "pointer_declarator" => {
                    pointers += 1;
                    declarator = declarator.child_by_field_name("declarator")?;
                }
                "reference_declarator" => {
                    declarator = declarator.named_child(0)?;
                }
                "function_declarator" => break,
                _ => return None,
            }
        }
        let name_node = declarator.child_by_field_name("declarator")?;
        let (owner, name) = match name_node.kind() {
            "identifier" | "field_identifier" => (None, node_text(name_node, text)),
            "qualified_identifier" => (
                field_text(name_node, "scope", text),
                field_text(name_node, "name", text)?,
            ),
            // Destructors and operators are not test targets.
            _ => return None,
        };
        let mut func = Func::named(name);
        func.owner = owner.map(str::to_owned);
        let is_static = children(node)
            .iter()
            .any(|c| c.kind() == "storage_class_specifier" && node_text(*c, text) == "static");
        // At namespace level `static` means file-private; in a class body
        // it means a static member.
        if node
            .parent()
            .is_some_and(|p| p.kind() == "field_declaration_list")
        {
            func.vis = Some(access);
            func.is_static = is_static;
        } else {
            func.vis = Some(if is_static { Vis::Private } else { Vis::Public });
        }
        for param in field_nodes(declarator, "parameters") {
            match param.kind() {
                "parameter_declaration" | "optional_parameter_declaration" => {
                    let ident = param
                        .child_by_field_name("declarator")
                        .and_then(|d| innermost_ident(d, &["identifier"]));
                    match ident {
                        Some(ident) => func.param(
                            node_text(param, text),
                            node_text(ident, text),
                            field_text(param, "type", text),
                            field_text(param, "default_value", text),
                        ),
                        // `(void)` and unnamed parameters contribute no argument.
                        None if field_text(param, "type", text) == Some("void") => {}
                        None => func.params.push(node_text(param, text).to_owned()),
                    }
                }
                "variadic_parameter" => func.params.push("...".to_owned()),
                _ => {}
            }
        }
        if let Some(ty) = field_text(node, "type", text)
            && (ty != "void" || pointers > 0)
        {
            func.ret = Some(format!("{ty}{}", "*".repeat(pointers)));
        }
        Some(func)
    }
}

// ---------------------------------------------------------------------------
// JavaScript / TypeScript
// ---------------------------------------------------------------------------

mod js {
    use super::{
        Defs, Func, Vis, children, clean_type, field_nodes, field_text, has_token, join_module,
        node_text, type_item,
    };
    use tree_sitter::Node;

    /// Walk a `program` or namespace `statement_block`.
    pub(super) fn walk(
        container: Node<'_>,
        text: &str,
        module: Option<&str>,
        exported: bool,
        defs: &mut Defs,
    ) {
        for child in children(container) {
            visit(child, text, module, exported, defs);
        }
    }

    fn visit(node: Node<'_>, text: &str, module: Option<&str>, exported: bool, defs: &mut Defs) {
        let vis = if exported { Vis::Public } else { Vis::Private };
        match node.kind() {
            "export_statement" => {
                if let Some(decl) = node.child_by_field_name("declaration") {
                    visit(decl, text, module, true, defs);
                }
            }
            "function_declaration" | "generator_function_declaration" => {
                let mut func = function(node, text, None);
                func.vis = Some(vis);
                func.module = module.map(str::to_owned);
                defs.push_fn(func);
            }
            // `const f = (a) => …` / `const f = function (a) {…}`
            "lexical_declaration" | "variable_declaration" => {
                for decl in children(node) {
                    let (Some(name), Some(value)) = (
                        field_text(decl, "name", text),
                        decl.child_by_field_name("value"),
                    ) else {
                        continue;
                    };
                    if matches!(value.kind(), "arrow_function" | "function_expression") {
                        let mut func = function(value, text, None);
                        name.clone_into(&mut func.name);
                        func.vis = Some(vis);
                        func.module = module.map(str::to_owned);
                        defs.push_fn(func);
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(name) = field_text(node, "name", text) {
                    defs.push_type(type_item(name, "class", Some(vis), module));
                    for member in field_nodes(node, "body") {
                        if member.kind() == "method_definition" {
                            let mut func = function(member, text, Some(name));
                            func.is_ctor = func.name == "constructor";
                            defs.push_method(func);
                        }
                    }
                }
            }
            "interface_declaration" | "enum_declaration" | "type_alias_declaration" => {
                if let Some(name) = field_text(node, "name", text) {
                    let kind = node.kind().trim_end_matches("_declaration");
                    let kind = if kind == "type_alias" { "type" } else { kind };
                    defs.push_type(type_item(name, kind, Some(vis), module));
                }
            }
            // TS `namespace X { … }`
            "internal_module" | "module" => {
                let name = field_text(node, "name", text);
                let path = name.map(|n| join_module(module, n, "."));
                if let Some(body) = node.child_by_field_name("body") {
                    walk(body, text, path.as_deref().or(module), exported, defs);
                }
            }
            _ => {}
        }
    }

    fn function(node: Node<'_>, text: &str, owner: Option<&str>) -> Func {
        let mut func = Func::named(field_text(node, "name", text).unwrap_or_default());
        func.owner = owner.map(str::to_owned);
        func.is_async = has_token(node, "async");
        if owner.is_some() {
            func.is_static = has_token(node, "static");
            let private_name = node
                .child_by_field_name("name")
                .is_some_and(|n| n.kind() == "private_property_identifier");
            let modifier = children(node)
                .into_iter()
                .find(|c| c.kind() == "accessibility_modifier")
                .map(|c| node_text(c, text).to_owned());
            func.vis = Some(match (private_name, modifier.as_deref()) {
                (true, _) | (_, Some("private")) => Vis::Private,
                (_, Some("protected")) => Vis::Protected,
                _ => Vis::Public,
            });
        }
        for param in field_nodes(node, "parameters") {
            let raw = node_text(param, text);
            let ty = field_text(param, "type", text).map(clean_type);
            let (name, default) = match param.kind() {
                "identifier" | "rest_pattern" => (raw.to_owned(), None),
                "required_parameter" | "optional_parameter" => (
                    field_text(param, "pattern", text).unwrap_or(raw).to_owned(),
                    field_text(param, "value", text),
                ),
                "assignment_pattern" => (
                    field_text(param, "left", text).unwrap_or(raw).to_owned(),
                    field_text(param, "right", text),
                ),
                _ => continue,
            };
            func.param(raw, &name, ty.as_deref(), default);
        }
        func.ret = field_text(node, "return_type", text).map(clean_type);
        func
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

mod go {
    use super::{
        Defs, Func, Vis, children, clean_type, field_nodes, field_text, node_text, type_item,
    };
    use tree_sitter::Node;

    /// Walk a `source_file`.
    pub(super) fn walk(container: Node<'_>, text: &str, defs: &mut Defs) {
        for child in children(container) {
            match child.kind() {
                "function_declaration" => {
                    let mut func = function(child, text);
                    func.is_static = false;
                    func.is_test = func.name.starts_with("Test") && !func.params.is_empty();
                    defs.push_fn(func);
                }
                "method_declaration" => {
                    let mut func = function(child, text);
                    let receiver = field_nodes(child, "receiver").into_iter().next();
                    if let Some(receiver) = receiver {
                        func.receiver = Some(node_text(receiver, text).to_owned());
                        if let Some(ty) = field_text(receiver, "type", text) {
                            let base = ty.trim_start_matches('*');
                            let base = base.split('[').next().unwrap_or(base);
                            func.owner = Some(base.to_owned());
                        }
                    }
                    defs.push_method(func);
                }
                "type_declaration" => {
                    for spec in children(child) {
                        let Some(name) = field_text(spec, "name", text) else {
                            continue;
                        };
                        let kind = match spec.child_by_field_name("type").map(|t| t.kind()) {
                            Some("struct_type") => "struct",
                            Some("interface_type") => "interface",
                            _ => "type",
                        };
                        defs.push_type(type_item(name, kind, Some(vis_of(name)), None));
                    }
                }
                _ => {}
            }
        }
    }

    fn vis_of(name: &str) -> Vis {
        if name.chars().next().is_some_and(char::is_uppercase) {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    fn function(node: Node<'_>, text: &str) -> Func {
        let mut func = Func::named(field_text(node, "name", text).unwrap_or_default());
        func.vis = Some(vis_of(&func.name));
        for param in field_nodes(node, "parameters") {
            // `w, h float64` names two arguments; a bare type names none. The
            // declaration text goes into `params` once, on the first name.
            let ty = field_text(param, "type", text).map(clean_type);
            let mut cursor = param.walk();
            let names: Vec<String> = param
                .children_by_field_name("name", &mut cursor)
                .map(|n| node_text(n, text).to_owned())
                .collect();
            if names.is_empty() {
                func.params.push(node_text(param, text).to_owned());
            }
            for (i, name) in names.iter().enumerate() {
                let raw = if i == 0 { node_text(param, text) } else { "" };
                func.param(raw, name, ty.as_deref(), None);
            }
        }
        func.ret = field_text(node, "result", text).map(clean_type);
        func
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use codemmet_core::context::{Collection, ContextProvider as _, DocSnapshot, Item};

    use crate::TreeSitterProvider;

    fn collections(language: &str, text: &str) -> BTreeMap<String, Collection> {
        let doc = DocSnapshot {
            language: language.into(),
            text: text.into(),
        };
        TreeSitterProvider::new().source_collections(&doc)
    }

    fn names(map: &BTreeMap<String, Collection>, key: &str) -> Vec<String> {
        map.get(key)
            .map(|c| c.items.iter().map(|i| i.value.clone()).collect())
            .unwrap_or_default()
    }

    fn item<'m>(map: &'m BTreeMap<String, Collection>, key: &str, value: &str) -> &'m Item {
        map.get(key)
            .and_then(|c| c.items.iter().find(|i| i.value == value))
            .unwrap_or_else(|| panic!("{key} has no item {value}"))
    }

    fn member<'m>(
        map: &'m BTreeMap<String, Collection>,
        key: &str,
        value: &str,
        m: &str,
    ) -> &'m str {
        item(map, key, value)
            .members
            .get(m)
            .map_or("<absent>", String::as_str)
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn rust_fns_types_and_methods() {
        let map = collections(
            "rust",
            r"
pub fn area(w: f64, h: f64) -> f64 { w * h }
fn helper(x: i32) {}
pub(crate) async fn fetch(url: &str) -> Result<String, E> { todo!() }
pub struct Point<T> { x: T }
pub enum Msg { Quit }
pub trait Shape { fn area(&self) -> f64; }
impl<T> Point<T> {
    pub fn new(x: T) -> Self { Self { x } }
    pub fn len(&self) -> f64 { 0.0 }
    fn bump(&mut self, by: f64) {}
}
impl fmt::Display for Point<f64> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) }
}
mod inner { pub fn nested_free() {} }
fn outer() { fn local() {} }
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
    #[tokio::test]
    async fn async_works() {}
}
",
        );
        assert_eq!(
            names(&map, "src.fns"),
            [
                "area",
                "helper",
                "fetch",
                "nested_free",
                "outer",
                "it_works",
                "async_works"
            ]
        );
        assert_eq!(member(&map, "src.fns", "area", "params"), "w: f64, h: f64");
        assert_eq!(member(&map, "src.fns", "area", "args"), "w, h");
        assert_eq!(member(&map, "src.fns", "area", "ret"), "f64");
        assert_eq!(member(&map, "src.fns", "area", "vis"), "public");
        assert_eq!(member(&map, "src.fns", "area", "static"), "<absent>");
        assert_eq!(member(&map, "src.fns", "helper", "vis"), "private");
        assert_eq!(member(&map, "src.fns", "fetch", "vis"), "public");
        assert_eq!(member(&map, "src.fns", "fetch", "async"), "async ");
        assert_eq!(member(&map, "src.fns", "nested_free", "module"), "inner");
        assert_eq!(member(&map, "src.fns", "it_works", "test"), "test");
        assert_eq!(member(&map, "src.fns", "async_works", "test"), "test");
        assert_eq!(member(&map, "src.fns", "it_works", "module"), "tests");
        assert_eq!(member(&map, "src.fns", "outer", "test"), "<absent>");

        assert_eq!(names(&map, "src.types"), ["Point", "Msg", "Shape"]);
        assert_eq!(member(&map, "src.types", "Shape", "kind"), "trait");

        assert_eq!(
            names(&map, "src.Point.methods"),
            ["new", "len", "bump", "fmt"]
        );
        assert_eq!(member(&map, "src.Point.methods", "new", "static"), "static");
        assert_eq!(member(&map, "src.Point.methods", "new", "ctor"), "ctor");
        assert_eq!(
            member(&map, "src.Point.methods", "len", "receiver"),
            "&self"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "len", "static"),
            "<absent>"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "bump", "receiver"),
            "&mut self"
        );
        assert_eq!(member(&map, "src.Point.methods", "bump", "vis"), "private");
        assert_eq!(member(&map, "src.Point.methods", "fmt", "trait"), "Display");
        assert_eq!(member(&map, "src.Point.methods", "fmt", "vis"), "public");
        assert_eq!(member(&map, "src.methods", "fmt", "owner"), "Point");
        // The data-shape collections still come along.
        assert_eq!(names(&map, "Msg"), ["Quit"]);
    }

    #[cfg(feature = "lang-python")]
    #[test]
    fn python_classes_decorators_and_receivers() {
        let map = collections(
            "python",
            r"
def area(w, h=1, *args, **kw) -> float:
    return w * h
def _helper(x): pass
async def fetch(url: str) -> str: ...
class Point(Base):
    count = 0
    def __init__(self, x, y): self.x = x
    def length(self) -> float: return 0.0
    def _secret(self): pass
    @staticmethod
    def origin(): return Point(0, 0)
    @classmethod
    def from_tuple(cls, t): return cls(*t)
    class Inner:
        def m(self): pass
def test_area(): assert area(1, 2) == 2
",
        );
        assert_eq!(
            names(&map, "src.fns"),
            ["area", "_helper", "fetch", "test_area"]
        );
        assert_eq!(
            member(&map, "src.fns", "area", "params"),
            "w, h=1, *args, **kw"
        );
        assert_eq!(member(&map, "src.fns", "area", "args"), "w, h, *args, **kw");
        assert_eq!(member(&map, "src.fns", "area", "ret"), "float");
        assert_eq!(member(&map, "src.fns", "_helper", "vis"), "private");
        assert_eq!(member(&map, "src.fns", "fetch", "async"), "async ");
        assert_eq!(member(&map, "src.fns", "test_area", "test"), "test");

        assert_eq!(names(&map, "src.types"), ["Point", "Inner"]);
        assert_eq!(
            names(&map, "src.Point.methods"),
            ["__init__", "length", "_secret", "origin", "from_tuple"]
        );
        assert_eq!(
            member(&map, "src.Point.methods", "__init__", "ctor"),
            "ctor"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "__init__", "vis"),
            "public"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "__init__", "args"),
            "x, y"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "length", "receiver"),
            "self"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "origin", "static"),
            "static"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "from_tuple", "receiver"),
            "cls"
        );
        assert_eq!(member(&map, "src.Point.methods", "from_tuple", "args"), "t");
        assert_eq!(names(&map, "src.Inner.methods"), ["m"]);
    }

    #[cfg(feature = "lang-c")]
    #[test]
    fn c_functions_prototypes_and_pointers() {
        let map = collections(
            "c",
            r"
#ifndef GEO_H
#define GEO_H
int area(int w, int h);
static void helper(void) {}
double *scale(double *p, size_t n) { return p; }
int area(int w, int h) { return w * h; }
void noargs() {}
struct Point { int x; int y; };
#endif
",
        );
        assert_eq!(
            names(&map, "src.fns"),
            ["area", "helper", "scale", "noargs"]
        );
        assert_eq!(member(&map, "src.fns", "area", "params"), "int w, int h");
        assert_eq!(member(&map, "src.fns", "area", "args"), "w, h");
        assert_eq!(member(&map, "src.fns", "area", "ret"), "int");
        assert_eq!(member(&map, "src.fns", "helper", "vis"), "private");
        assert_eq!(member(&map, "src.fns", "helper", "params"), "<absent>");
        assert_eq!(member(&map, "src.fns", "scale", "ret"), "double*");
        assert_eq!(member(&map, "src.fns", "scale", "args"), "p, n");
        assert_eq!(member(&map, "src.fns", "noargs", "ret"), "<absent>");
        assert_eq!(names(&map, "src.types"), ["Point"]);
    }

    #[cfg(feature = "lang-cpp")]
    #[test]
    fn cpp_classes_access_sections_and_out_of_class_definitions() {
        let map = collections(
            "cpp",
            r"
namespace geo {
int area(int w, int h) { return w * h; }
class Point {
public:
    Point(int x, int y);
    double length() const;
    static Point origin();
protected:
    void prot();
private:
    int x_;
    void secret();
};
struct Vec2 { float x; float dot(const Vec2& o) const { return 0; } };
double Point::length() const { return 0; }
}
template <typename T> T ident(T v) { return v; }
",
        );
        assert_eq!(names(&map, "src.fns"), ["area", "ident"]);
        assert_eq!(member(&map, "src.fns", "area", "module"), "geo");
        assert_eq!(member(&map, "src.fns", "ident", "params"), "T v");
        assert_eq!(names(&map, "src.types"), ["Point", "Vec2"]);
        assert_eq!(member(&map, "src.types", "Point", "kind"), "class");
        assert_eq!(
            names(&map, "src.Point.methods"),
            ["Point", "length", "origin", "prot", "secret"]
        );
        assert_eq!(member(&map, "src.Point.methods", "Point", "ctor"), "ctor");
        assert_eq!(member(&map, "src.Point.methods", "Point", "args"), "x, y");
        // Declared in-class (public), defined out-of-class: one item.
        assert_eq!(member(&map, "src.Point.methods", "length", "vis"), "public");
        assert_eq!(member(&map, "src.Point.methods", "length", "ret"), "double");
        assert_eq!(
            member(&map, "src.Point.methods", "origin", "static"),
            "static"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "prot", "vis"),
            "protected"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "secret", "vis"),
            "private"
        );
        assert_eq!(names(&map, "src.Vec2.methods"), ["dot"]);
        assert_eq!(member(&map, "src.Vec2.methods", "dot", "vis"), "public");
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn typescript_exports_modifiers_and_arrows() {
        let map = collections(
            "typescript",
            r#"
export function area(w: number, h: number): number { return w * h; }
function helper(x: string) {}
export async function fetchIt(url: string): Promise<string> { return ""; }
export const arrow = (a: number, b = 2) => a;
export class Point {
  constructor(x: number, y: number) {}
  length(): number { return 0; }
  private secret(): void {}
  protected prot() {}
  #hidden() {}
  static origin(): Point { return new Point(0, 0); }
}
export interface Shape { area(): number; }
export type Dir = "n" | "s";
export default function main() {}
"#,
        );
        assert_eq!(
            names(&map, "src.fns"),
            ["area", "helper", "fetchIt", "arrow", "main"]
        );
        assert_eq!(member(&map, "src.fns", "area", "ret"), "number");
        assert_eq!(member(&map, "src.fns", "area", "vis"), "public");
        assert_eq!(member(&map, "src.fns", "helper", "vis"), "private");
        assert_eq!(member(&map, "src.fns", "fetchIt", "async"), "async ");
        assert_eq!(member(&map, "src.fns", "arrow", "args"), "a, b");
        assert_eq!(member(&map, "src.fns", "main", "vis"), "public");
        assert_eq!(names(&map, "src.types"), ["Point", "Shape", "Dir"]);
        assert_eq!(member(&map, "src.types", "Shape", "kind"), "interface");
        assert_eq!(member(&map, "src.types", "Dir", "kind"), "type");
        assert_eq!(
            names(&map, "src.Point.methods"),
            [
                "constructor",
                "length",
                "secret",
                "prot",
                "#hidden",
                "origin"
            ]
        );
        assert_eq!(
            member(&map, "src.Point.methods", "constructor", "ctor"),
            "ctor"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "secret", "vis"),
            "private"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "prot", "vis"),
            "protected"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "#hidden", "vis"),
            "private"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "origin", "static"),
            "static"
        );
    }

    #[cfg(feature = "lang-javascript")]
    #[test]
    fn javascript_without_exports_is_private() {
        let map = collections(
            "javascript",
            r"
export function area(w, h) { return w * h; }
function helper(x) {}
class Point {
  length() { return 0; }
  static origin() { return new Point(0, 0); }
}
",
        );
        assert_eq!(names(&map, "src.fns"), ["area", "helper"]);
        assert_eq!(member(&map, "src.fns", "area", "vis"), "public");
        assert_eq!(member(&map, "src.fns", "area", "args"), "w, h");
        assert_eq!(member(&map, "src.fns", "helper", "vis"), "private");
        assert_eq!(member(&map, "src.types", "Point", "vis"), "private");
        assert_eq!(names(&map, "src.Point.methods"), ["length", "origin"]);
    }

    #[cfg(feature = "lang-go")]
    #[test]
    fn go_receivers_and_capitalization() {
        let map = collections(
            "go",
            r"
package geo
func Area(w, h float64) float64 { return w * h }
func helper(x int) {}
func Pair() (int, error) { return 0, nil }
type Point struct { X, Y float64 }
func (p Point) Length() float64 { return 0 }
func (p *Point) Bump(by float64) {}
func TestArea(t *testing.T) {}
type Shape interface { Area() float64 }
",
        );
        assert_eq!(
            names(&map, "src.fns"),
            ["Area", "helper", "Pair", "TestArea"]
        );
        assert_eq!(member(&map, "src.fns", "Area", "params"), "w, h float64");
        assert_eq!(member(&map, "src.fns", "Area", "args"), "w, h");
        assert_eq!(member(&map, "src.fns", "Area", "vis"), "public");
        assert_eq!(member(&map, "src.fns", "helper", "vis"), "private");
        assert_eq!(member(&map, "src.fns", "Pair", "ret"), "(int, error)");
        assert_eq!(member(&map, "src.fns", "TestArea", "test"), "test");
        assert_eq!(names(&map, "src.Point.methods"), ["Length", "Bump"]);
        assert_eq!(
            member(&map, "src.Point.methods", "Length", "receiver"),
            "p Point"
        );
        assert_eq!(
            member(&map, "src.Point.methods", "Bump", "receiver"),
            "p *Point"
        );
        assert_eq!(member(&map, "src.Point.methods", "Bump", "owner"), "Point");
        assert_eq!(names(&map, "src.types"), ["Point", "Shape"]);
        assert_eq!(member(&map, "src.types", "Shape", "kind"), "interface");
    }

    /// Every function item carries a `params` sub-collection: one item per
    /// parameter, with `type` (and `default`) members where declared.
    #[cfg(all(feature = "lang-rust", feature = "lang-python", feature = "lang-go"))]
    #[test]
    fn params_sub_collection() {
        let params =
            |map: &BTreeMap<String, Collection>, key: &str, value: &str| -> Vec<(String, String)> {
                item(map, key, value)
                    .collections
                    .get("params")
                    .map(|c| {
                        c.items
                            .iter()
                            .map(|p| {
                                (
                                    p.value.clone(),
                                    p.members.get("type").cloned().unwrap_or_default(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
        let rust = collections(
            "rust",
            "pub fn area(w: f64, h: &str) -> f64 { 0.0 }\nfn none() {}\n",
        );
        assert_eq!(
            params(&rust, "src.fns", "area"),
            [
                ("w".to_owned(), "f64".to_owned()),
                ("h".to_owned(), "&str".to_owned())
            ]
        );
        assert!(item(&rust, "src.fns", "none").collections.is_empty());

        let py = collections("python", "def f(a, b: int = 2, *rest):\n    pass\n");
        assert_eq!(
            params(&py, "src.fns", "f"),
            [
                ("a".to_owned(), String::new()),
                ("b".to_owned(), "int".to_owned()),
                ("*rest".to_owned(), String::new())
            ]
        );
        assert_eq!(
            item(&py, "src.fns", "f").collections["params"].items[1].members["default"],
            "2"
        );

        let go = collections("go", "package x\nfunc Area(w, h float64, s string) {}\n");
        assert_eq!(
            params(&go, "src.fns", "Area"),
            [
                ("w".to_owned(), "float64".to_owned()),
                ("h".to_owned(), "float64".to_owned()),
                ("s".to_owned(), "string".to_owned())
            ]
        );
        assert_eq!(
            member(&go, "src.fns", "Area", "params"),
            "w, h float64, s string"
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn unsupported_language_yields_nothing() {
        assert!(collections("nope", "fn x() {}").is_empty());
    }
}
