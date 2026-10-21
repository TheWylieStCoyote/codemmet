//! `codemmet vars`: the collections and variables a source file yields.
//!
//! The discovery side of source collections (`SOURCE_COLLECTIONS_PLAN.md`):
//! print what a template can reference when this file is the `--source` —
//! the `src.*` API-surface collections (functions, types, methods), the
//! data-shape collections (enum variants, struct fields), and the `src.*`
//! variables. It is also the extractor's oracle: what it prints is exactly
//! what an expansion sees.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use codemmet_core::context::{Collection, DocSnapshot};
use codemmet_core::lang::profile;

use super::config_failure;
use super::render::default_provider;

/// Arguments for `codemmet vars`.
#[derive(Debug, Args)]
pub struct VarsArgs {
    /// Language id, file extension, or LSP language id (default: inferred
    /// from the file's extension).
    #[arg(long, value_name = "ID")]
    lang: Option<String>,

    /// Print one JSON object: `{"language", "path", "collections": [{"name",
    /// "items": [{"value", "members"}]}]}`.
    #[arg(long)]
    json: bool,

    /// The source file to read.
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

/// Run `codemmet vars`.
pub fn run(args: &VarsArgs) -> ExitCode {
    let Some(language) = resolve_language(args.lang.as_deref(), &args.file) else {
        return config_failure(&format!(
            "cannot infer the language of `{}`; pass --lang",
            args.file.display()
        ));
    };
    let text = match std::fs::read_to_string(&args.file) {
        Ok(text) => text,
        Err(err) => {
            return config_failure(&format!("cannot read {}: {err}", args.file.display()));
        }
    };
    let doc = DocSnapshot {
        language: language.to_owned(),
        text,
    };
    let collections = default_provider().source_collections(&doc);
    if args.json {
        let items: Vec<serde_json::Value> = collections
            .iter()
            .map(|(name, collection)| collection_json(name, collection))
            .collect();
        let value = serde_json::json!({
            "language": language,
            "path": args.file.display().to_string(),
            "collections": items,
        });
        emit!("{value}");
    } else {
        print_text(&collections);
    }
    ExitCode::SUCCESS
}

/// The canonical profile id for `--lang`, or inferred from the file name.
fn resolve_language(flag: Option<&str>, file: &Path) -> Option<&'static str> {
    let key = match flag {
        Some(flag) => flag.to_owned(),
        None => file.extension()?.to_str()?.to_owned(),
    };
    profile::find(&key).map(|p| p.id)
}

fn collection_json(name: &str, collection: &Collection) -> serde_json::Value {
    let items: Vec<serde_json::Value> = collection
        .items
        .iter()
        .map(|item| {
            let mut value = serde_json::json!({ "value": item.value, "members": item.members });
            // Nested collections (a function's `params`) ride along only when
            // present, so items without substructure stay compact.
            if !item.collections.is_empty() {
                let nested: serde_json::Map<String, serde_json::Value> = item
                    .collections
                    .iter()
                    .map(|(key, sub)| (key.clone(), collection_json(key, sub)["items"].clone()))
                    .collect();
                value["collections"] = serde_json::Value::Object(nested);
            }
            value
        })
        .collect();
    serde_json::json!({ "name": name, "items": items })
}

/// One block per collection: its name, then one line per item with the
/// members as `key=value` pairs (the redundant `name` member omitted), and an
/// indented `.params:` line under items that carry a sub-collection.
fn print_text(collections: &std::collections::BTreeMap<String, Collection>) {
    if collections.is_empty() {
        emit!("(no collections)");
        return;
    }
    let mut first = true;
    for (name, collection) in collections {
        if !first {
            emit!();
        }
        first = false;
        emit!("{name}:");
        let width = collection
            .items
            .iter()
            .map(|item| item.value.len())
            .max()
            .unwrap_or(0);
        for item in &collection.items {
            let members: Vec<String> = item
                .members
                .iter()
                .filter(|(key, value)| !(key.as_str() == "name" && **value == item.value))
                .map(|(key, value)| format!("{key}={value:?}"))
                .collect();
            if members.is_empty() {
                emit!("  {}", item.value);
            } else {
                emit!("  {:<width$}  {}", item.value, members.join(" "));
            }
            // A function's parameters, as the `*@outer.params` sub-collection.
            for (key, sub) in &item.collections {
                let entries: Vec<String> = sub
                    .items
                    .iter()
                    .map(|p| {
                        let extras: Vec<String> = p
                            .members
                            .iter()
                            .map(|(k, v)| format!("{k}={v:?}"))
                            .collect();
                        if extras.is_empty() {
                            p.value.clone()
                        } else {
                            format!("{} ({})", p.value, extras.join(" "))
                        }
                    })
                    .collect();
                emit!("  {:<width$}    .{key}: {}", "", entries.join(", "));
            }
        }
    }
}
