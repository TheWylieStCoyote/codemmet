//! Code-generation edits behind editor code actions.
//!
//! Where [`extract`](crate::extract) gathers collections for *typing* a
//! data-driven abbreviation, this module produces ready-to-apply edits for
//! *existing* code — the discoverable counterpart. Put the cursor in a
//! dispatch over an enum's values and the editor offers to complete it:
//!
//! * a Rust `match` — "Fill match arms from `Msg`" adds `Msg::Quit =>
//!   todo!(),` per missing variant;
//! * a C, C++, Go, JavaScript or TypeScript `switch` — "Fill switch cases
//!   from `Color`" adds `case Color.Red:` (with the language's `break;`);
//! * a Python `match` — "Fill match cases from `Color`" adds `case
//!   Color.RED:` clauses;
//! * an `if` / `else if` chain comparing one value against variants — "Fill
//!   if chain from `Color`" appends an `else if` per missing variant, in the
//!   same form the chain already uses (`==`, `===`, `is`, Rust `if let` or
//!   `matches!`), before a final `else` when there is one.
//!
//! The enums are the file's own ([`extract::enums`]) plus the language's
//! well-known builtins ([`extract::builtin_enums`]: `Option`, `Result`,
//! `bool`, …). A branch already present is never repeated — coverage is
//! judged by the variant's bare name appearing in a case or condition — and
//! once any branch names an enum's variant only that enum is offered; an
//! empty `match`/`switch` offers every enum in sight.
//!
//! The edits are plain text (no tabstops), so any LSP client applies them
//! without cooperating with the codemmet plugin.

use std::fmt::Write as _;

use codemmet_core::context::{FillTarget, Item};
use tree_sitter::Node;

use crate::extract::{self, node_text};

/// Fill targets for the dispatch construct enclosing `offset`: one per enum
/// that still has unhandled variants there. Empty when the cursor is not
/// inside a `match`, `switch` or `if` chain, when every candidate enum is
/// fully covered, or when `language` has no dispatch shape.
pub fn dispatch_fills(
    language: &str,
    root: Node<'_>,
    text: &str,
    offset: usize,
    indent_unit: &str,
) -> Vec<FillTarget> {
    let Some(lang) = Lang::of(language) else {
        return Vec::new();
    };
    let Some(site) = Site::at(lang, root, text, offset) else {
        return Vec::new();
    };
    let heads = site.heads(text);

    let mut enums: Vec<(String, Vec<Item>)> = extract::enums(language, root, text)
        .into_iter()
        .map(|(name, collection)| (name, collection.items))
        .collect();
    for (name, items) in extract::builtin_enums(language) {
        if !enums.iter().any(|(n, _)| n == name) {
            enums.push((name.to_owned(), items));
        }
    }
    let covered = |variant: &Item| heads.iter().any(|h| mentions_word(h, bare(variant)));
    let named = |variants: &[Item]| variants.iter().any(covered);
    // Once a branch names a variant, that enum is what the code is about;
    // until then (an empty body) every enum is a candidate. A chain always
    // has a condition, so it only ever offers the enums its conditions name.
    let any_named = enums.iter().any(|(_, variants)| named(variants));
    let mut out = Vec::new();
    for (name, variants) in &enums {
        if (any_named || matches!(site, Site::Chain { .. })) && !named(variants) {
            continue;
        }
        let missing: Vec<&Item> = variants.iter().filter(|v| !covered(v)).collect();
        if missing.is_empty() {
            continue;
        }
        if let Some((range, new_text)) = site.edit(lang, text, indent_unit, &missing) {
            out.push(FillTarget {
                title: format!("{} from {name}", site.verb(lang)),
                range,
                new_text,
            });
        }
    }
    out
}

/// The dispatch shapes codemmet knows, one per grammar family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Rust,
    /// C and C++.
    C,
    Go,
    /// JavaScript and TypeScript.
    Js,
    Python,
}

impl Lang {
    fn of(language: &str) -> Option<Self> {
        Some(match language {
            "rust" => Self::Rust,
            "c" | "cpp" => Self::C,
            "go" => Self::Go,
            "javascript" | "typescript" => Self::Js,
            "python" => Self::Python,
            _ => return None,
        })
    }

    /// The `match`/`switch` node kind.
    const fn cases_kind(self) -> &'static str {
        match self {
            Self::Rust => "match_expression",
            Self::C | Self::Js => "switch_statement",
            Self::Go => "expression_switch_statement",
            Self::Python => "match_statement",
        }
    }

    /// The `if` node kind.
    const fn if_kind(self) -> &'static str {
        match self {
            Self::Rust => "if_expression",
            Self::C | Self::Go | Self::Js | Self::Python => "if_statement",
        }
    }

    /// The node kinds of one arm/case (the default case included).
    const fn case_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["match_arm"],
            Self::C => &["case_statement"],
            Self::Go => &["expression_case", "default_case"],
            Self::Js => &["switch_case", "switch_default"],
            Self::Python => &["case_clause"],
        }
    }
}

/// Where the cursor is: inside a `match`/`switch` body, or inside an `if`
/// chain (at its root `if`).
#[derive(Debug)]
enum Site<'t> {
    Cases {
        /// The `match`/`switch` node.
        construct: Node<'t>,
        /// The node whose `{`/`}` delimit the cases (the construct itself in Go).
        body: Node<'t>,
        /// Non-default cases, in order.
        cases: Vec<Node<'t>>,
        /// The default / `_` case, if any.
        default: Option<Node<'t>>,
    },
    Chain {
        /// The root `if` of the chain.
        head: Node<'t>,
        /// Each branch's condition, parentheses stripped, in order.
        conditions: Vec<String>,
        /// Where the last `if`/`else if` branch body ends.
        tail_end: usize,
        /// The start of a final `else` keyword, if the chain has one.
        final_else: Option<usize>,
    },
}

impl<'t> Site<'t> {
    /// The innermost dispatch construct enclosing `offset`.
    fn at(lang: Lang, root: Node<'t>, text: &str, offset: usize) -> Option<Self> {
        let mut node = root.named_descendant_for_byte_range(offset, offset)?;
        loop {
            if node.kind() == lang.cases_kind() {
                return Self::cases(lang, node, text);
            }
            if node.kind() == lang.if_kind() {
                return Some(Self::chain(lang, chain_root(lang, node), text));
            }
            node = node.parent()?;
        }
    }

    fn cases(lang: Lang, construct: Node<'t>, text: &str) -> Option<Self> {
        let body = match lang {
            Lang::Go => construct,
            _ => construct.child_by_field_name("body")?,
        };
        let mut cases = Vec::new();
        let mut default = None;
        let mut cursor = body.walk();
        for case in body.named_children(&mut cursor) {
            if !lang.case_kinds().contains(&case.kind()) {
                continue;
            }
            if is_default_case(lang, case, text) {
                default = Some(case);
            } else {
                cases.push(case);
            }
        }
        Some(Self::Cases {
            construct,
            body,
            cases,
            default,
        })
    }

    fn chain(lang: Lang, head: Node<'t>, text: &str) -> Self {
        let mut conditions = Vec::new();
        let mut tail_end = head.end_byte();
        let mut final_else = None;
        if lang == Lang::Python {
            // `elif` clauses are siblings under the root `if`, not nested.
            conditions.extend(condition_text(lang, head, text));
            tail_end = block_end(head, "consequence").unwrap_or(tail_end);
            let mut cursor = head.walk();
            for clause in head.children_by_field_name("alternative", &mut cursor) {
                match clause.kind() {
                    "elif_clause" => {
                        conditions.extend(condition_text(lang, clause, text));
                        tail_end = block_end(clause, "consequence").unwrap_or(tail_end);
                    }
                    "else_clause" => final_else = Some(clause.start_byte()),
                    _ => {}
                }
            }
        } else {
            let mut cur = head;
            loop {
                conditions.extend(condition_text(lang, cur, text));
                tail_end = block_end(cur, "consequence").unwrap_or_else(|| cur.end_byte());
                let Some(alt) = cur.child_by_field_name("alternative") else {
                    break;
                };
                // Rust/C/JS wrap the alternative in an `else_clause`; Go makes
                // the `if` or block the alternative directly.
                let next_if = if alt.kind() == lang.if_kind() {
                    Some(alt)
                } else {
                    named_child_of_kind(alt, lang.if_kind())
                };
                let Some(next) = next_if else {
                    final_else = Some(if alt.kind() == "else_clause" {
                        alt.start_byte()
                    } else {
                        token_child(cur, "else")
                            .map_or_else(|| alt.start_byte(), |t| t.start_byte())
                    });
                    break;
                };
                cur = next;
            }
        }
        Self::Chain {
            head,
            conditions,
            tail_end,
            final_else,
        }
    }

    /// The text already dispatching: case patterns/values, or conditions.
    fn heads(&self, text: &str) -> Vec<String> {
        match self {
            Self::Cases { cases, default, .. } => cases
                .iter()
                .chain(default.iter())
                .map(|case| case_head(*case, text))
                .collect(),
            Self::Chain { conditions, .. } => conditions.clone(),
        }
    }

    const fn verb(&self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Chain { .. }, _) => "Fill if chain",
            (Self::Cases { .. }, Lang::Rust) => "Fill match arms",
            (Self::Cases { .. }, Lang::Python) => "Fill match cases",
            (Self::Cases { .. }, _) => "Fill switch cases",
        }
    }

    /// The edit adding `missing` to this site: `(byte range to replace, text)`.
    fn edit(
        &self,
        lang: Lang,
        text: &str,
        unit: &str,
        missing: &[&Item],
    ) -> Option<(std::ops::Range<usize>, String)> {
        match self {
            Self::Cases {
                construct,
                body,
                cases,
                default,
            } => {
                let line = line_indent(text, construct.start_byte());
                let case_indent = match (cases.first().copied().or(*default), lang) {
                    (Some(first), _) => line_indent(text, first.start_byte()),
                    (None, Lang::Go) => line.clone(),
                    (None, _) => format!("{line}{unit}"),
                };
                let arms: Vec<String> = missing
                    .iter()
                    .map(|v| case_text(lang, &case_indent, unit, &v.value))
                    .collect();
                if let Some(last) = cases.last() {
                    let mut out = String::new();
                    for arm in &arms {
                        let _ = write!(out, "\n{case_indent}{arm}");
                    }
                    Some((last.end_byte()..last.end_byte(), out))
                } else if let Some(default) = default {
                    // Before the default case, which keeps its own line.
                    let mut out = String::new();
                    for arm in &arms {
                        let _ = write!(out, "{arm}\n{case_indent}");
                    }
                    Some((default.start_byte()..default.start_byte(), out))
                } else {
                    // Empty body: fill between the braces. (A Python `match`
                    // cannot be empty, so it always has a case.)
                    let (open, close) = (token_child(*body, "{")?, token_child(*body, "}")?);
                    let mut out = String::new();
                    for arm in &arms {
                        let _ = write!(out, "\n{case_indent}{arm}");
                    }
                    let _ = write!(out, "\n{line}");
                    Some((open.end_byte()..close.start_byte(), out))
                }
            }
            Self::Chain {
                head,
                conditions,
                tail_end,
                final_else,
            } => {
                let (subject, form) = conditions
                    .iter()
                    .find_map(|c| missing_subject(c, missing))?;
                let line = line_indent(text, head.start_byte());
                let branches: Vec<String> = missing
                    .iter()
                    .map(|v| branch_text(lang, &line, unit, &subject, &form, &v.value))
                    .collect();
                let mut out = String::new();
                match (lang, final_else) {
                    (Lang::Python, Some(at)) => {
                        for b in &branches {
                            let _ = write!(out, "{b}\n{line}");
                        }
                        Some((*at..*at, out))
                    }
                    (Lang::Python, None) => {
                        for b in &branches {
                            let _ = write!(out, "\n{line}{b}");
                        }
                        Some((*tail_end..*tail_end, out))
                    }
                    (_, Some(at)) => {
                        for b in &branches {
                            let _ = write!(out, "{b} ");
                        }
                        Some((*at..*at, out))
                    }
                    (_, None) => {
                        for b in &branches {
                            let _ = write!(out, " {b}");
                        }
                        Some((*tail_end..*tail_end, out))
                    }
                }
            }
        }
    }
}

/// How a chain spells its test, learned from an existing condition.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Form {
    /// Rust `if let PATTERN = subject`.
    Let,
    /// Rust `matches!(subject, PATTERN)`.
    Matches,
    /// `subject OP variant` — `==`, `===`, Python `is`.
    Cmp(String),
}

/// The chain's subject and form, from a condition that mentions one of the
/// enum's variants (`missing` is a sample of them; any condition of the
/// chain names some variant of the enum, else the enum was not a candidate).
/// `None` when the condition does not have a recognizable shape.
fn missing_subject(condition: &str, missing: &[&Item]) -> Option<(String, Form)> {
    let c = condition.trim();
    if let Some(rest) = c.strip_prefix("let ") {
        let (_, subject) = rest.split_once(" = ")?;
        return Some((subject.trim().to_owned(), Form::Let));
    }
    if let Some(rest) = c.strip_prefix("matches!(") {
        let inner = rest.strip_suffix(')')?;
        let (subject, _) = inner.split_once(',')?;
        return Some((subject.trim().to_owned(), Form::Matches));
    }
    // Longest operators first, so `===` is not read as `==`.
    for (op, positive) in [
        ("!==", "==="),
        ("===", "==="),
        ("!=", "=="),
        ("==", "=="),
        (" is not ", "is"),
        (" is ", "is"),
    ] {
        if let Some((left, right)) = c.split_once(op) {
            let (left, right) = (left.trim(), right.trim());
            // The subject is whichever side is not a variant. Variants of
            // this enum are not necessarily in `missing`, so test the side
            // that looks like a value against a known variant spelling, and
            // otherwise take the left side as the subject (the common order).
            let looks_like_variant =
                |side: &str| missing.iter().any(|v| mentions_word(side, bare(v)));
            let subject = if looks_like_variant(left) && !looks_like_variant(right) {
                right
            } else {
                left
            };
            return Some((subject.to_owned(), Form::Cmp(positive.to_owned())));
        }
    }
    None
}

/// One generated case for `value`, without its leading newline and indent;
/// `indent` is the case's own indent (used for continuation lines).
fn case_text(lang: Lang, indent: &str, unit: &str, value: &str) -> String {
    match lang {
        Lang::Rust => format!("{value} => todo!(),"),
        Lang::C | Lang::Js => format!("case {value}:\n{indent}{unit}break;"),
        Lang::Go => format!("case {value}:"),
        Lang::Python => format!("case {value}:\n{indent}{unit}pass"),
    }
}

/// One generated `else if` branch (Python: `elif`), without its separator;
/// `line` is the chain's indent.
fn branch_text(
    lang: Lang,
    line: &str,
    unit: &str,
    subject: &str,
    form: &Form,
    value: &str,
) -> String {
    let payload = value.contains("(..)") || value.contains("{ .. }");
    let condition = match (lang, form) {
        (Lang::Rust, Form::Let) => format!("let {value} = {subject}"),
        // `==` cannot test a payload variant; `matches!` can.
        (Lang::Rust, Form::Matches | Form::Cmp(_)) if payload || *form == Form::Matches => {
            format!("matches!({subject}, {value})")
        }
        (_, Form::Cmp(op)) => format!("{subject} {op} {value}"),
        // A non-Rust chain never reads as `let`/`matches!`; spell a comparison.
        (_, Form::Let | Form::Matches) => format!("{subject} == {value}"),
    };
    match lang {
        Lang::Rust => format!("else if {condition} {{\n{line}{unit}todo!()\n{line}}}"),
        Lang::C | Lang::Js => format!("else if ({condition}) {{\n{line}}}"),
        Lang::Go => format!("else if {condition} {{\n{line}}}"),
        Lang::Python => format!("elif {condition}:\n{line}{unit}pass"),
    }
}

/// The root `if` of the chain `node` belongs to: climb while this `if` is the
/// alternative of an enclosing one (`else if`). Python chains are flat, so
/// the innermost `if_statement` is already the root.
fn chain_root(lang: Lang, mut node: Node<'_>) -> Node<'_> {
    loop {
        let Some(parent) = node.parent() else {
            return node;
        };
        let enclosing = match parent.kind() {
            // Rust/C/JS: `if … else_clause(if …)`.
            "else_clause" => parent.parent().filter(|p| p.kind() == lang.if_kind()),
            // Go: the inner `if` is the outer's `alternative` directly.
            kind if kind == lang.if_kind() && lang == Lang::Go => Some(parent).filter(|p| {
                p.child_by_field_name("alternative")
                    .is_some_and(|a| a.id() == node.id())
            }),
            _ => None,
        };
        match enclosing {
            Some(outer) => node = outer,
            None => return node,
        }
    }
}

/// An `if`/`elif` node's condition text with any enclosing parentheses
/// stripped (C's `parenthesized_expression`, C++'s `condition_clause`).
fn condition_text(lang: Lang, node: Node<'_>, text: &str) -> Option<String> {
    let cond = node.child_by_field_name("condition")?;
    let raw = node_text(cond, text).trim();
    let stripped = match lang {
        Lang::C | Lang::Js => raw
            .strip_prefix('(')
            .and_then(|r| r.strip_suffix(')'))
            .unwrap_or(raw),
        _ => raw,
    };
    Some(stripped.trim().to_owned())
}

/// Where a node's `field` block ends.
fn block_end(node: Node<'_>, field: &str) -> Option<usize> {
    node.child_by_field_name(field).map(|b| b.end_byte())
}

/// A case's dispatching text: its pattern(s) or value(s); `default:` and
/// `_` read as themselves (they never match a variant name).
fn case_head(case: Node<'_>, text: &str) -> String {
    if let Some(value) = case
        .child_by_field_name("pattern")
        .or_else(|| case.child_by_field_name("value"))
    {
        return node_text(value, text).to_owned();
    }
    // Python `case_clause`: one or more `case_pattern` children.
    let mut cursor = case.walk();
    let patterns: Vec<&str> = case
        .named_children(&mut cursor)
        .filter(|c| c.kind() == "case_pattern")
        .map(|c| node_text(c, text))
        .collect();
    if patterns.is_empty() {
        node_text(case, text).to_owned()
    } else {
        patterns.join(" | ")
    }
}

fn is_default_case(lang: Lang, case: Node<'_>, text: &str) -> bool {
    match lang {
        Lang::Rust | Lang::Python => case_head(case, text).trim() == "_",
        Lang::C => case.child_by_field_name("value").is_none(),
        Lang::Go => case.kind() == "default_case",
        Lang::Js => case.kind() == "switch_default",
    }
}

/// A variant's bare name: its `name` member, else its value (C, Go).
fn bare(item: &Item) -> &str {
    item.members
        .get("name")
        .map_or(item.value.as_str(), String::as_str)
}

/// Whether `needle` occurs in `hay` as a whole word (not inside a longer
/// identifier). `needle` may carry its own punctuation (`"circle"`).
fn mentions_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    hay.match_indices(needle).any(|(at, _)| {
        let before = hay[..at].chars().next_back().is_none_or(|c| !is_word(c));
        let after = hay[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !is_word(c));
        before && after
    })
}

/// The first named child of `node` with `kind`.
fn named_child_of_kind<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| c.kind() == kind)
}

/// The anonymous token child of `node` with `kind` (`{`, `}`, `else`).
fn token_child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

/// The leading whitespace of the line containing `offset`.
fn line_indent(text: &str, offset: usize) -> String {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map_or(0, |i| i + 1);
    text[start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}
