//! Source-file variables: the `${src.<key>}` values derived from a file's
//! path (see `SOURCE_COLLECTIONS_PLAN.md`, decision 3).
//!
//! | key      | value                                              |
//! | -------- | -------------------------------------------------- |
//! | `path`   | the path as given                                  |
//! | `file`   | the file name                                      |
//! | `stem`   | the file name without its extension                |
//! | `dir`    | the parent directory (empty at the root)           |
//! | `module` | the language's import spelling of the file         |
//!
//! `module` per language: Rust `crate::a::b` from `src/a/b.rs` (`lib.rs` /
//! `main.rs` → `crate`, `mod.rs` → its directory); Python `a.b` with a leading
//! `src/` dropped and `__init__.py` → its package; Go the `package` clause
//! (falling back to the directory name); C/C++ the header name
//! (`stem.h`, or the file itself when it already is a header); JavaScript /
//! TypeScript `./stem`. Everything here is path arithmetic — no parsing — so
//! the minimal build gets it too.

use std::collections::BTreeMap;
use std::path::Path;

/// The `${src.*}` variables for `path`, whose contents are `text` (only Go
/// reads it, for the `package` clause). `language` is a canonical profile id.
#[must_use]
pub fn variables(language: &str, path: &Path, text: &str) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    let display = path.display().to_string();
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path
        .parent()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    vars.insert("path".to_owned(), display);
    vars.insert("file".to_owned(), file.clone());
    vars.insert("stem".to_owned(), stem.clone());
    vars.insert("dir".to_owned(), dir);
    vars.insert(
        "module".to_owned(),
        module(language, path, &file, &stem, text),
    );
    vars
}

/// The import spelling of `path` for `language`.
fn module(language: &str, path: &Path, file: &str, stem: &str, text: &str) -> String {
    match language {
        "rust" => rust_module(path, stem),
        "python" => python_module(path, stem),
        "go" => go_package(path, text),
        "c" | "cpp" => {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if matches!(ext.as_str(), "h" | "hpp" | "hh" | "hxx") {
                file.to_owned()
            } else {
                format!("{stem}.h")
            }
        }
        "javascript" | "typescript" => format!("./{stem}"),
        _ => stem.to_owned(),
    }
}

/// The directory components of `path` after its **last** `src` component —
/// the conventional project-relative module path, whether `path` is
/// relative or absolute. Empty when there is no `src` component.
fn components_after_src(path: &Path) -> Vec<String> {
    let parts: Vec<String> = path
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    parts
        .iter()
        .rposition(|p| p == "src")
        .map_or_else(Vec::new, |idx| parts[idx + 1..].to_vec())
}

/// `crate::a::b` for `src/a/b.rs`; `lib.rs`/`main.rs` are the crate root and
/// `mod.rs` is its directory. Without a `src` component only the stem counts.
fn rust_module(path: &Path, stem: &str) -> String {
    let mut parts = components_after_src(path);
    if !matches!(stem, "lib" | "main" | "mod") {
        parts.push(stem.to_owned());
    }
    std::iter::once("crate".to_owned())
        .chain(parts)
        .collect::<Vec<_>>()
        .join("::")
}

/// The package chain of a Python file: every ancestor directory that holds
/// an `__init__.py`, outermost first (the one filesystem check here — a
/// package is defined by that file, not by the path). Falls back to the
/// components after `src` for a file whose packages are not on disk.
fn python_module(path: &Path, stem: &str) -> String {
    let mut chain = Vec::new();
    let mut dir = path.parent();
    while let Some(d) = dir {
        if !d.join("__init__.py").is_file() {
            break;
        }
        match d.file_name() {
            Some(name) => chain.push(name.to_string_lossy().into_owned()),
            None => break,
        }
        dir = d.parent();
    }
    chain.reverse();
    let mut parts = if chain.is_empty() {
        components_after_src(path)
    } else {
        chain
    };
    if stem == "__init__" {
        if parts.is_empty()
            && let Some(dir) = path.parent().and_then(Path::file_name)
        {
            parts.push(dir.to_string_lossy().into_owned());
        }
    } else {
        parts.push(stem.to_owned());
    }
    if parts.is_empty() {
        stem.to_owned()
    } else {
        parts.join(".")
    }
}

/// The `package` clause's name, else the directory name, else the stem.
fn go_package(path: &Path, text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("package "))
        .map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            path.parent()
                .and_then(Path::file_name)
                .map(|d| d.to_string_lossy().into_owned())
                .filter(|d| !d.is_empty())
        })
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_of(language: &str, path: &str, text: &str) -> String {
        variables(language, Path::new(path), text)
            .remove("module")
            .expect("module is always set")
    }

    #[test]
    fn path_parts() {
        let vars = variables("rust", Path::new("src/shapes/point.rs"), "");
        assert_eq!(vars["path"], "src/shapes/point.rs");
        assert_eq!(vars["file"], "point.rs");
        assert_eq!(vars["stem"], "point");
        assert_eq!(vars["dir"], "src/shapes");
        let root = variables("rust", Path::new("lib.rs"), "");
        assert_eq!(root["dir"], "");
    }

    #[test]
    fn rust_modules() {
        assert_eq!(
            module_of("rust", "src/shapes/point.rs", ""),
            "crate::shapes::point"
        );
        assert_eq!(module_of("rust", "src/lib.rs", ""), "crate");
        assert_eq!(module_of("rust", "src/main.rs", ""), "crate");
        assert_eq!(module_of("rust", "src/shapes/mod.rs", ""), "crate::shapes");
        assert_eq!(module_of("rust", "point.rs", ""), "crate::point");
    }

    #[test]
    fn python_modules() {
        assert_eq!(
            module_of("python", "src/pkg/geometry.py", ""),
            "pkg.geometry"
        );
        assert_eq!(module_of("python", "pkg/__init__.py", ""), "pkg");
        assert_eq!(module_of("python", "geometry.py", ""), "geometry");
    }

    #[test]
    fn go_package_clause_then_directory() {
        assert_eq!(module_of("go", "pkg/geo/point.go", "package geo\n"), "geo");
        assert_eq!(module_of("go", "pkg/geo/point.go", "// no clause\n"), "geo");
        assert_eq!(module_of("go", "point.go", ""), "point");
    }

    #[test]
    fn c_headers_and_js_relative_paths() {
        assert_eq!(module_of("c", "src/point.c", ""), "point.h");
        assert_eq!(module_of("c", "include/point.h", ""), "point.h");
        assert_eq!(module_of("cpp", "src/point.cpp", ""), "point.h");
        assert_eq!(module_of("cpp", "src/point.hpp", ""), "point.hpp");
        assert_eq!(module_of("typescript", "src/lib/point.ts", ""), "./point");
        assert_eq!(module_of("javascript", "point.js", ""), "./point");
    }

    #[test]
    fn modules_anchor_on_the_last_src_component() {
        // Absolute paths anchor on the last `src`, never the filesystem root.
        assert_eq!(
            module_of("rust", "/home/me/proj/src/shapes/point.rs", ""),
            "crate::shapes::point"
        );
        assert_eq!(
            module_of("rust", "/tmp/scratch/point.rs", ""),
            "crate::point"
        );
        assert_eq!(
            module_of("python", "/abs/elsewhere/geometry.py", ""),
            "geometry"
        );
    }

    #[test]
    fn python_packages_come_from_init_files_on_disk() {
        let root = std::env::temp_dir().join(format!("codemmet-src-vars-{}", std::process::id()));
        let pkg = root.join("app").join("shapes");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(root.join("app").join("__init__.py"), "").expect("write");
        std::fs::write(pkg.join("__init__.py"), "").expect("write");
        let file = pkg.join("point.py");
        std::fs::write(&file, "").expect("write");
        assert_eq!(
            module_of("python", file.to_str().expect("utf8"), ""),
            "app.shapes.point"
        );
        assert_eq!(
            module_of(
                "python",
                pkg.join("__init__.py").to_str().expect("utf8"),
                ""
            ),
            "app.shapes"
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
