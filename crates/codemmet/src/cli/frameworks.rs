//! `codemmet frameworks`: the builtin test framework packs, and which one each
//! language uses for this invocation.

use std::process::ExitCode;

use clap::Args;
use codemmet_core::lang::frameworks::{FRAMEWORKS, for_language};
use codemmet_core::lang::profile::PROFILES;

use super::Sources;

/// Arguments for `codemmet frameworks`.
#[derive(Debug, Args)]
pub struct FrameworksArgs {
    /// Only this language (id, file extension, or LSP language id).
    #[arg(long, value_name = "ID")]
    lang: Option<String>,

    /// Print a JSON array of `{"language", "name", "description", "selected"}`.
    #[arg(long)]
    json: bool,
}

/// Run `codemmet frameworks`.
pub fn run(sources: &Sources, args: &FrameworksArgs) -> ExitCode {
    let languages: Vec<&str> = match &args.lang {
        Some(id) => match codemmet_core::lang::profile::find(id) {
            Some(profile) => vec![profile.id],
            None => return super::config_failure(&format!("unknown language `{id}`")),
        },
        None => PROFILES.iter().map(|p| p.id).collect(),
    };
    let mut rows = Vec::new();
    for lang in languages {
        let selected = sources.frameworks.get(lang).map(String::as_str);
        for fw in for_language(lang) {
            rows.push((lang, fw, selected == Some(fw.name)));
        }
    }
    if args.json {
        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|(lang, fw, selected)| {
                serde_json::json!({
                    "language": lang,
                    "name": fw.name,
                    "description": fw.description,
                    "selected": selected,
                })
            })
            .collect();
        emit!("{}", serde_json::Value::Array(items));
        return ExitCode::SUCCESS;
    }
    if rows.is_empty() {
        emit!("(no test framework packs for this language)");
        return ExitCode::SUCCESS;
    }
    let width = FRAMEWORKS.iter().map(|f| f.name.len()).max().unwrap_or(0);
    let mut last = "";
    for (lang, fw, selected) in rows {
        if lang != last {
            if !last.is_empty() {
                emit!();
            }
            emit!("{lang}:");
            last = lang;
        }
        let mark = if selected { "*" } else { " " };
        emit!("  {mark} {:<width$}  {}", fw.name, fw.description);
    }
    ExitCode::SUCCESS
}
