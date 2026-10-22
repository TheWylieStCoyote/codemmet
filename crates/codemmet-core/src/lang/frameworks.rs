//! Builtin **test framework packs**: per-language snippet packs that swap the
//! test triggers (`tfn`, `tmeth`, `tfile`, …) for another framework's shape.
//!
//! A framework pack is an ordinary pack with `[meta] framework = "<name>"`.
//! When a project selects it (`frameworks.<lang> = "<name>"` in
//! `.codemmet/config.toml`, the `--framework` flag, or the LSP `frameworks`
//! init option), the registry merges it right after the language's builtin
//! pack and before any user or project override, so it changes exactly the
//! triggers it defines and users still override everything. Because it only
//! ever selects packs compiled into the binary, a project's choice is safe to
//! honor without the opt-in that project *snippet* packs need.
//!
//! A language without a pack of a given name inherits its ancestor's: with
//! `typescript = "vitest"`, the JavaScript `vitest` pack applies.

use crate::lang::profile;

/// One builtin framework pack.
#[derive(Debug, Clone, Copy)]
pub struct Framework {
    /// The language the pack is written for (`"python"`).
    pub language: &'static str,
    /// The name a project selects it by (`"unittest"`).
    pub name: &'static str,
    /// One line for `codemmet frameworks`.
    pub description: &'static str,
    /// The pack's TOML, compiled into the binary.
    pub source: &'static str,
}

/// Every builtin framework pack.
pub const FRAMEWORKS: &[Framework] = &[
    Framework {
        language: "python",
        name: "unittest",
        description: "unittest.TestCase methods (self.assertEqual); tfile wraps them in a TestCase",
        source: include_str!("../../snippets/frameworks/python/unittest.toml"),
    },
    Framework {
        language: "python",
        name: "asyncio",
        description: "pytest with pytest-asyncio: async functions get @pytest.mark.asyncio tests that await",
        source: include_str!("../../snippets/frameworks/python/asyncio.toml"),
    },
    Framework {
        language: "rust",
        name: "tokio",
        description: "async functions get #[tokio::test] async tests that .await",
        source: include_str!("../../snippets/frameworks/rust/tokio.toml"),
    },
    Framework {
        language: "c",
        name: "unity",
        description: "Unity TEST_ASSERT_EQUAL tests; tfile adds setUp/tearDown and a RUN_TEST main",
        source: include_str!("../../snippets/frameworks/c/unity.toml"),
    },
    Framework {
        language: "cpp",
        name: "catch2",
        description: "Catch2 v3 TEST_CASE / REQUIRE instead of GoogleTest",
        source: include_str!("../../snippets/frameworks/cpp/catch2.toml"),
    },
    Framework {
        language: "cpp",
        name: "doctest",
        description: "doctest TEST_CASE / CHECK instead of GoogleTest; tfile defines the main",
        source: include_str!("../../snippets/frameworks/cpp/doctest.toml"),
    },
    Framework {
        language: "go",
        name: "testify",
        description: "testing.T tests asserting with testify's assert.Equal",
        source: include_str!("../../snippets/frameworks/go/testify.toml"),
    },
    Framework {
        language: "typescript",
        name: "vitest",
        description: "Vitest: Jest-shaped tests, tfile writes <stem>.test.ts importing test/expect from \"vitest\"",
        source: include_str!("../../snippets/frameworks/typescript/vitest.toml"),
    },
    Framework {
        language: "javascript",
        name: "vitest",
        description: "Vitest: Jest-shaped tests, tfile imports test/expect from \"vitest\" (TypeScript inherits)",
        source: include_str!("../../snippets/frameworks/javascript/vitest.toml"),
    },
];

/// The framework pack `name` for `language`.
///
/// Searches the language itself and then the languages it inherits from
/// (TypeScript → JavaScript, C++ → C). `None` when no builtin pack has that
/// name for any of them.
#[must_use]
pub fn find(language: &str, name: &str) -> Option<&'static Framework> {
    let mut current = profile::find(language)?;
    loop {
        if let Some(fw) = FRAMEWORKS
            .iter()
            .find(|f| f.language == current.id && f.name == name)
        {
            return Some(fw);
        }
        current = profile::find(current.inherits?)?;
    }
}

/// The framework packs selectable for `language`: its own, then its
/// ancestors' (a name the language defines itself hides the inherited one).
#[must_use]
pub fn for_language(language: &str) -> Vec<&'static Framework> {
    let mut out: Vec<&'static Framework> = Vec::new();
    let mut current = profile::find(language);
    while let Some(p) = current {
        for fw in FRAMEWORKS.iter().filter(|f| f.language == p.id) {
            if !out.iter().any(|seen| seen.name == fw.name) {
                out.push(fw);
            }
        }
        current = p.inherits.and_then(profile::find);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_walks_the_inheritance_chain() {
        assert_eq!(find("python", "unittest").map(|f| f.name), Some("unittest"));
        assert_eq!(
            find("typescript", "vitest").map(|f| f.language),
            Some("typescript"),
            "a language's own pack wins over the inherited one"
        );
        assert_eq!(
            find("javascript", "vitest").map(|f| f.language),
            Some("javascript")
        );
        assert!(find("cpp", "catch2").is_some());
        assert!(find("c", "catch2").is_none(), "C does not inherit C++");
        assert_eq!(
            find("cpp", "unity").map(|f| f.language),
            Some("c"),
            "C++ inherits C's"
        );
        assert!(find("python", "nope").is_none());
        assert!(find("nolang", "unittest").is_none());
    }

    #[test]
    fn for_language_includes_inherited_packs() {
        let ts: Vec<(&str, &str)> = for_language("typescript")
            .iter()
            .map(|f| (f.language, f.name))
            .collect();
        assert_eq!(
            ts,
            [("typescript", "vitest")],
            "own pack hides the inherited one"
        );
        let names: Vec<&str> = for_language("python").iter().map(|f| f.name).collect();
        assert_eq!(names, ["unittest", "asyncio"]);
        let names: Vec<&str> = for_language("cpp").iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            ["catch2", "doctest", "unity"],
            "own packs first, then C's"
        );
        let names: Vec<&str> = for_language("go").iter().map(|f| f.name).collect();
        assert_eq!(names, ["testify"]);
    }

    /// Every builtin framework pack parses, targets the language it is filed
    /// under, and declares its own name.
    #[test]
    fn builtin_framework_packs_are_consistent() {
        for fw in FRAMEWORKS {
            let file = crate::config::loader::parse_snippet_file(fw.name, fw.source)
                .unwrap_or_else(|err| panic!("{}/{}: {err}", fw.language, fw.name));
            assert_eq!(file.meta.language, fw.language, "{}", fw.name);
            assert_eq!(file.meta.framework.as_deref(), Some(fw.name));
        }
    }
}
