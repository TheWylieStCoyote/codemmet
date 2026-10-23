//! The trainer's fixed **workspace**: named collections every session can
//! repeat over, as if a source file with these definitions were open.
//!
//! Levels before [`Level::Collections`](super::exercise::Level::Collections)
//! use inline literals (`*[a,b,c]`) and need no context. The collections
//! levels teach `*@Name`, filters, zips, and payload members, which need
//! named collections with members — the kind an editor extracts from the
//! buffer. There is no buffer here, so the trainer seeds the same small
//! workspace for every language, spelled the way that language's extractor
//! would: a `Point` and a `Raw` struct, a `Msg` enum (with payloads in
//! Rust), a `Handled` list, a `src.fns` API surface, and a `Shape`
//! interface's declared members. The exercise screen lists them so a
//! learner can see what a `*@Point` or `?!name@Handled` refers to.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use codemmet_core::context::{Collection, Item};

/// Build an item from `(key, value)` member pairs.
fn item(value: &str, members: &[(&str, &str)]) -> Item {
    let mut item = Item::new(value);
    for (key, member) in members {
        item.members.insert((*key).to_owned(), (*member).to_owned());
    }
    item
}

/// A collection of plain values.
fn plain(values: &[&str]) -> Collection {
    Collection {
        items: values.iter().map(|v| Item::new(*v)).collect(),
    }
}

/// A fields collection: `(name, type)` per field.
fn fields(specs: &[(&str, &str)]) -> Collection {
    Collection {
        items: specs
            .iter()
            .map(|(name, ty)| item(name, &[("type", ty)]))
            .collect(),
    }
}

/// An enum collection whose values are `prefix` + bare name, with the bare
/// name as `name` (an empty prefix spells them bare, as C and Go do).
fn variants(prefix: &str, names: &[&str]) -> Collection {
    Collection {
        items: names
            .iter()
            .map(|name| {
                let mut it = Item::new(format!("{prefix}{name}"));
                if !prefix.is_empty() {
                    it.members.insert("name".to_owned(), (*name).to_owned());
                }
                it
            })
            .collect(),
    }
}

/// The Rust `Msg` enum with payload members, as the extractor describes it.
fn rust_msg() -> Collection {
    Collection {
        items: vec![
            item(
                "Msg::Quit",
                &[("name", "Quit"), ("kind", "unit"), ("bind", "Msg::Quit")],
            ),
            item(
                "Msg::Write(..)",
                &[
                    ("name", "Write"),
                    ("kind", "tuple"),
                    ("payload", "String"),
                    ("bind", "Msg::Write(v)"),
                    ("args", "v"),
                    ("arity", "1"),
                ],
            ),
            item(
                "Msg::Move { .. }",
                &[
                    ("name", "Move"),
                    ("kind", "struct"),
                    ("payload", "x: i32, y: i32"),
                    ("bind", "Msg::Move { x, y }"),
                    ("args", "x, y"),
                    ("arity", "2"),
                ],
            ),
        ],
    }
}

/// The Rust `AppError` enum: two single-value wrappers and a unit variant.
fn rust_app_error() -> Collection {
    Collection {
        items: vec![
            item(
                "AppError::Io(..)",
                &[
                    ("name", "Io"),
                    ("kind", "tuple"),
                    ("payload", "std::io::Error"),
                    ("bind", "AppError::Io(v)"),
                    ("args", "v"),
                    ("arity", "1"),
                ],
            ),
            item(
                "AppError::Parse(..)",
                &[
                    ("name", "Parse"),
                    ("kind", "tuple"),
                    ("payload", "std::num::ParseIntError"),
                    ("bind", "AppError::Parse(v)"),
                    ("args", "v"),
                    ("arity", "1"),
                ],
            ),
            item(
                "AppError::Timeout",
                &[
                    ("name", "Timeout"),
                    ("kind", "unit"),
                    ("bind", "AppError::Timeout"),
                ],
            ),
        ],
    }
}

/// `src.fns`: a public `area(w, h)` and a private `helper()`.
fn src_fns(params: &str, ret: &str) -> Collection {
    Collection {
        items: vec![
            item(
                "area",
                &[
                    ("name", "area"),
                    ("params", params),
                    ("args", "w, h"),
                    ("ret", ret),
                    ("vis", "public"),
                ],
            ),
            item("helper", &[("name", "helper"), ("vis", "private")]),
        ],
    }
}

/// `src.Shape.methods`: a required `area` and an optional `describe`
/// (`all_required` makes both required, as every Go interface method is).
fn shape_methods(area: &str, describe: &str, all_required: bool) -> Collection {
    let mut items = vec![item(
        "area",
        &[("name", "area"), ("decl", area), ("required", "required")],
    )];
    let mut describe_item = item("describe", &[("name", "describe"), ("decl", describe)]);
    if all_required {
        describe_item
            .members
            .insert("required".to_owned(), "required".to_owned());
    }
    items.push(describe_item);
    Collection { items }
}

/// The shared pieces every language's workspace has, in that language's
/// spelling: `Point` and `Raw` field lists, the `Msg` variants, the `Handled`
/// list, `src.fns`, and (where the pack has `stub`) `src.Shape.methods`.
struct Spelling<'a> {
    point: &'a [(&'a str, &'a str)],
    raw: &'a [(&'a str, &'a str)],
    msg: Collection,
    handled: &'a str,
    params: &'a str,
    ret: &'a str,
    shape: Option<(&'a str, &'a str, bool)>,
}

impl Spelling<'_> {
    fn into_map(self) -> BTreeMap<String, Collection> {
        let mut map = BTreeMap::new();
        map.insert("Point".to_owned(), fields(self.point));
        map.insert("Raw".to_owned(), fields(self.raw));
        map.insert("Msg".to_owned(), self.msg);
        map.insert("Handled".to_owned(), plain(&[self.handled]));
        map.insert("src.fns".to_owned(), src_fns(self.params, self.ret));
        if let Some((area, describe, all_required)) = self.shape {
            map.insert(
                "src.Shape.methods".to_owned(),
                shape_methods(area, describe, all_required),
            );
        }
        map
    }
}

/// The workspace collections for `language` (empty for an unknown one).
#[must_use]
pub fn collections(language: &str) -> BTreeMap<String, Collection> {
    if language == "javascript" {
        // No types in JavaScript: the field lists are bare names.
        let mut map = Spelling {
            point: &[],
            raw: &[],
            msg: variants("", &["Quit", "Write", "Move"]),
            handled: "Quit",
            params: "w, h",
            ret: "",
            shape: Some(("area()", "describe()", false)),
        }
        .into_map();
        map.insert("Point".to_owned(), plain(&["x", "y", "tags", "label"]));
        map.insert("Raw".to_owned(), plain(&["x", "y", "z"]));
        return map;
    }
    let Some(spelling) = spelling(language) else {
        return BTreeMap::new();
    };
    let mut map = spelling.into_map();
    if language == "rust" {
        map.insert("AppError".to_owned(), rust_app_error());
    }
    map
}

/// How a typed language spells the workspace, or `None` for an unknown one.
fn spelling(language: &str) -> Option<Spelling<'static>> {
    Some(match language {
        "rust" => Spelling {
            point: &[
                ("x", "f64"),
                ("y", "f64"),
                ("tags", "Vec<String>"),
                ("label", "Option<String>"),
            ],
            raw: &[("x", "f64"), ("y", "f64"), ("z", "f64")],
            msg: rust_msg(),
            handled: "Quit",
            params: "w: f64, h: f64",
            ret: "f64",
            shape: Some((
                "fn area(&self) -> f64",
                "fn describe(&self) -> String",
                false,
            )),
        },
        "python" => Spelling {
            point: &[
                ("x", "float"),
                ("y", "float"),
                ("tags", "list[str]"),
                ("label", "str | None"),
            ],
            raw: &[("x", "float"), ("y", "float"), ("z", "float")],
            msg: variants("Msg.", &["QUIT", "WRITE", "MOVE"]),
            handled: "QUIT",
            params: "w, h",
            ret: "float",
            shape: Some((
                "def area(self) -> float",
                "def describe(self) -> str",
                false,
            )),
        },
        "typescript" => Spelling {
            point: &[
                ("x", "number"),
                ("y", "number"),
                ("tags", "string[]"),
                ("label", "string | null"),
            ],
            raw: &[("x", "number"), ("y", "number"), ("z", "number")],
            msg: variants("Msg.", &["Quit", "Write", "Move"]),
            handled: "Quit",
            params: "w: number, h: number",
            ret: "number",
            shape: Some(("area(): number", "describe(): string", false)),
        },
        "go" => Spelling {
            point: &[
                ("X", "float64"),
                ("Y", "float64"),
                ("Tags", "[]string"),
                ("Label", "string"),
            ],
            raw: &[("X", "float64"), ("Y", "float64"), ("Z", "float64")],
            msg: variants("", &["Quit", "Write", "Move"]),
            handled: "Quit",
            params: "w float64, h float64",
            ret: "float64",
            shape: Some(("Area() float64", "Describe() string", true)),
        },
        "c" => Spelling {
            point: &[
                ("x", "double"),
                ("y", "double"),
                ("count", "int"),
                ("label", "const char*"),
            ],
            raw: &[("x", "double"), ("y", "double"), ("z", "double")],
            msg: variants("", &["QUIT", "WRITE", "MOVE"]),
            handled: "QUIT",
            params: "double w, double h",
            ret: "double",
            shape: None,
        },
        "cpp" => Spelling {
            point: &[
                ("x", "double"),
                ("y", "double"),
                ("tags", "std::vector<std::string>"),
                ("label", "std::string"),
            ],
            raw: &[("x", "double"), ("y", "double"), ("z", "double")],
            msg: variants("Msg::", &["Quit", "Write", "Move"]),
            handled: "Quit",
            params: "double w, double h",
            ret: "double",
            shape: Some(("double area() const", "std::string describe() const", false)),
        },
        _ => return None,
    })
}

/// Members worth showing next to an item on the exercise screen.
const SHOWN_MEMBERS: &[&str] = &["type", "payload", "vis", "required"];

/// One display line per collection: its name, then its items with the
/// members a learner needs to see (`x: f64`, `area (public)`, …).
#[must_use]
pub fn summary(collections: &BTreeMap<String, Collection>) -> Vec<String> {
    collections
        .iter()
        .map(|(name, collection)| {
            let items: Vec<String> = collection
                .items
                .iter()
                .map(|it| {
                    let mut text = it.value.clone();
                    if let Some(ty) = it.members.get("type") {
                        text.push_str(": ");
                        text.push_str(ty);
                    }
                    let extras: Vec<&str> = SHOWN_MEMBERS
                        .iter()
                        .filter(|k| **k != "type")
                        .filter_map(|k| it.members.get(*k).map(String::as_str))
                        .collect();
                    if !extras.is_empty() {
                        let _ = write!(text, " ({})", extras.join(", "));
                    }
                    text
                })
                .collect();
            format!("{name}: {}", items.join(", "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curated_language_has_a_workspace_with_the_shared_names() {
        for language in [
            "rust",
            "python",
            "typescript",
            "javascript",
            "go",
            "c",
            "cpp",
        ] {
            let map = collections(language);
            for name in ["Point", "Raw", "Msg", "Handled", "src.fns"] {
                assert!(map.contains_key(name), "{language} lacks {name}");
            }
            // `Handled` names a `Msg` variant by bare name, so `?!name@Handled`
            // (or `?!value@Handled` where variants are bare) has an effect.
            let handled = &map["Handled"].items[0].value;
            assert!(
                map["Msg"]
                    .items
                    .iter()
                    .any(|m| { &m.value == handled || m.members.get("name") == Some(handled) }),
                "{language}: Handled[0] is not a Msg variant"
            );
        }
        assert!(collections("cobol").is_empty());
    }

    #[test]
    fn summary_shows_types_and_key_members() {
        let lines = summary(&collections("rust"));
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("Point: x: f64, y: f64, tags: Vec<String>"))
        );
        assert!(lines.iter().any(|l| l.contains("Msg::Write(..) (String)")));
        assert!(lines.iter().any(|l| l.contains("area (public)")));
        assert!(lines.iter().any(|l| l.contains("area (required)")));
    }
}
