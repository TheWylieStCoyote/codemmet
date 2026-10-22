//! The project configuration file, `.codemmet/config.toml`.
//!
//! Today it holds one table: which [test framework
//! pack](crate::lang::frameworks) each language uses.
//!
//! ```toml
//! # .codemmet/config.toml
//! [frameworks]
//! python = "unittest"
//! rust   = "tokio"
//! ```
//!
//! Like project snippet packs, config files are collected from the filesystem
//! root down to the file being edited and merged **root-most first, nearest
//! wins** per language, so a monorepo can set a default at its root and a
//! subproject can refine it. Unlike snippet packs, a config file only
//! *selects among packs compiled into the binary*, so it is safe to honor in
//! an editor without the project-pack opt-in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::loader::ConfigError;

/// The selected framework per language id (`"python"` → `"unittest"`).
pub type Frameworks = BTreeMap<String, String>;

/// The `config.toml` file shape.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// `[frameworks]`: language id → framework pack name.
    #[serde(default)]
    pub frameworks: Frameworks,
}

impl ProjectConfig {
    /// Parse one `config.toml`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Toml {
            origin: path.display().to_string(),
            source: Box::new(source),
        })
    }
}

/// Every `.codemmet/config.toml` at or above `start`, **root-most first**.
#[must_use]
pub fn find_project_configs(start: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = start
        .ancestors()
        .filter_map(|dir| {
            let candidate = dir.join(".codemmet").join("config.toml");
            candidate.is_file().then_some(candidate)
        })
        .collect();
    files.reverse();
    files
}

/// The frameworks in effect for a file at `start`: `defaults` (an editor's or
/// invocation's), overridden per language by each project config from the
/// root down, nearest winning.
pub fn resolve_frameworks(start: &Path, defaults: &Frameworks) -> Result<Frameworks, ConfigError> {
    let mut frameworks = defaults.clone();
    for path in find_project_configs(start) {
        frameworks.extend(ProjectConfig::load(&path)?.frameworks);
    }
    Ok(frameworks)
}

/// Parse a `LANG=NAME` selection (the `--framework` flag).
pub fn parse_selection(spec: &str) -> Result<(String, String), String> {
    let (lang, name) = spec
        .split_once('=')
        .ok_or_else(|| format!("`{spec}`: expected LANG=NAME, e.g. python=unittest"))?;
    let (lang, name) = (lang.trim(), name.trim());
    if lang.is_empty() || name.is_empty() {
        return Err(format!(
            "`{spec}`: expected LANG=NAME, e.g. python=unittest"
        ));
    }
    let profile = crate::lang::profile::find(lang)
        .ok_or_else(|| format!("`{spec}`: unknown language `{lang}`"))?;
    Ok((profile.id.to_owned(), name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("codemmet-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn configs_merge_root_most_first_nearest_wins() {
        let root = scratch("merge");
        let sub = root.join("svc").join("api");
        std::fs::create_dir_all(sub.join(".codemmet")).expect("mkdir");
        std::fs::create_dir_all(root.join(".codemmet")).expect("mkdir");
        std::fs::write(
            root.join(".codemmet/config.toml"),
            "[frameworks]\npython = \"unittest\"\nrust = \"tokio\"\n",
        )
        .expect("write");
        std::fs::write(
            sub.join(".codemmet/config.toml"),
            "[frameworks]\npython = \"asyncio\"\n",
        )
        .expect("write");
        let defaults = Frameworks::from([("cpp".to_owned(), "catch2".to_owned())]);
        let got = resolve_frameworks(&sub, &defaults).expect("resolve");
        assert_eq!(got["python"], "asyncio", "nearest wins");
        assert_eq!(got["rust"], "tokio", "root applies below");
        assert_eq!(got["cpp"], "catch2", "defaults survive");
        // Above the subproject only the root config applies.
        let got = resolve_frameworks(&root.join("svc"), &Frameworks::new()).expect("resolve");
        assert_eq!(got["python"], "unittest");
        assert_eq!(
            find_project_configs(&sub).len(),
            2,
            "both configs found, root first"
        );
    }

    #[test]
    fn unknown_keys_are_errors() {
        let root = scratch("unknown");
        std::fs::create_dir_all(root.join(".codemmet")).expect("mkdir");
        std::fs::write(root.join(".codemmet/config.toml"), "[nope]\nx = 1\n").expect("write");
        assert!(resolve_frameworks(&root, &Frameworks::new()).is_err());
    }

    #[test]
    fn selection_parses_lang_equals_name() {
        assert_eq!(
            parse_selection("py=unittest").expect("ok"),
            ("python".to_owned(), "unittest".to_owned())
        );
        assert!(parse_selection("python").is_err());
        assert!(parse_selection("klingon=x").is_err());
        assert!(parse_selection("python=").is_err());
    }
}
