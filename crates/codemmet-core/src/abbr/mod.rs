//! The abbreviation mini-grammar: `fn:main>fori10>pr`.
//!
//! Grammar (see `PLAN.md` for the full spec):
//!
//! ```text
//! abbr     := group
//! group    := level ( '^'+ level )*        ; '^' climbs, clamped at this group
//! level    := chain ( '+' chain )*         ; siblings
//! chain    := element ( '>' level )?       ; '>' attaches following level as children
//! element  := atom ( '*' NUMBER )?
//! atom     := node | '(' group ')'
//! node     := WORD args?
//! args     := ':' arg ( ',' arg )*
//! ```
//!
//! `^` (climb-up) ends the current `>`-nesting level and attaches the
//! following chain N levels up, where N is the number of consecutive carets:
//! `a>b^c` puts `c` back at the top level, `a>b>c^d` makes `d` a sibling of
//! `b`. Carets beyond the top level clamp there (`a^^^b` is `a+b`).
//!
//! Groups interact with `^` per the flat-distribution design (a `(…)` group
//! is inlined into its surrounding level, [`AbbrNode`] has no group concept):
//!
//! - **After** a group, `^` climbs from the level the group occupies:
//!   `x>(a>b)^c` puts the group at `x`'s child level, so one caret makes `c`
//!   a top-level sibling of `x`.
//! - **Inside** a group, `^` clamps at the group's own top level and never
//!   escapes the parentheses: `x>(a>b^^c)` keeps `c` inside the group, as a
//!   sibling of `a` (both children of `x`).
//!
//! The parser is language-agnostic: it does **not** split `fori10` into
//! trigger + glued arg. That resolution needs the active language's trigger
//! set and happens in the renderer via
//! [`SnippetSet::resolve_word`](crate::config::loader::SnippetSet::resolve_word).

mod parser;

pub use parser::parse;

use std::ops::Range;

/// Canonical set of operator characters on the abbreviation surface — the
/// single source of truth for code that scans abbreviation text without
/// parsing it (candidate extraction, operator gating).
///
/// Mirrors the grammar above: `>` child, `+` sibling, `*` repeat, `:` args,
/// `,` arg separator, `(`/`)` groups, `^` climb-up.
pub const OPERATOR_CHARS: &[char] = &['>', '+', '*', ':', ',', '(', ')', '^'];

/// A parsed abbreviation: the top-level sibling chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abbreviation(pub Vec<AbbrNode>);

/// How a node repeats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repeat {
    /// A literal count from `*n` (and the absent case, `Count(1)`).
    Count(u32),
    /// `*@name`: repeat over the collection named `name` (data-driven),
    /// narrowed by `filter` (`*@src.fns?vis=public`).
    Source {
        /// The collection name, possibly dotted (`src.Point.methods`).
        name: String,
        /// The `?predicate` chain written after the name.
        filter: Filter,
    },
    /// A bare `*`: repeat over the subject named by the enclosing construct,
    /// narrowed by `filter`.
    Subject {
        /// The `?predicate` chain written after the `*`.
        filter: Filter,
    },
    /// `*[a,b,c]`: repeat over an inline collection literal. The string is the
    /// raw `[…]` body (`a,b,c`, each value optionally `value:type`), parsed
    /// into items at render time via `Collection::parse_values`.
    Inline(String),
}

impl Repeat {
    /// `*@name` with no filter.
    #[must_use]
    pub fn source(name: &str) -> Self {
        Self::Source {
            name: name.to_owned(),
            filter: Filter::default(),
        }
    }

    /// A bare `*` with no filter.
    #[must_use]
    pub const fn subject() -> Self {
        Self::Subject {
            filter: Filter::NONE,
        }
    }

    /// The filter written on a data-driven repeat (`None` for literal counts
    /// and inline literals, which take none).
    #[must_use]
    pub const fn filter(&self) -> Option<&Filter> {
        match self {
            Self::Source { filter, .. } | Self::Subject { filter } => Some(filter),
            Self::Count(_) | Self::Inline(_) => None,
        }
    }

    /// Parse-time weight for the size budget: the literal count, the item count
    /// for an inline literal, or 1 for a context-driven repeat (whose real size
    /// the render budget bounds).
    #[must_use]
    pub fn budget_weight(&self) -> u32 {
        match self {
            Self::Count(n) => *n,
            Self::Source { .. } | Self::Subject { .. } => 1,
            // A safe upper bound (empties are dropped at render); the value
            // budget still catches anything that expands too large.
            Self::Inline(spec) => u32::try_from(spec.split(',').count()).unwrap_or(u32::MAX),
        }
    }

    /// Whether the count comes from a collection rather than a literal count —
    /// `*@name`, a bare `*`, or an inline `*[…]`. Used to reject data-driven
    /// repeats where the semantics are undefined (groups, composites).
    #[must_use]
    pub const fn is_data_driven(&self) -> bool {
        matches!(
            self,
            Self::Source { .. } | Self::Subject { .. } | Self::Inline(_)
        )
    }
}

/// The `?predicate` chain on a data-driven repeat (see
/// `SOURCE_COLLECTIONS_PLAN.md`, decisions 5 and 6).
///
/// A chain that was **not written** (`predicates` empty, `all` false) lets the
/// snippet's own default `filter` apply; `?all` clears that default, keeping
/// every item (any further predicates in the chain still apply).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// `?all` was written: drop the snippet's default filter.
    pub all: bool,
    /// The predicates, all of which an item must satisfy.
    pub predicates: Vec<Predicate>,
}

impl Filter {
    /// No filter written.
    pub const NONE: Self = Self {
        all: false,
        predicates: Vec::new(),
    };

    /// Whether anything was written (so the snippet default is replaced).
    #[must_use]
    pub const fn is_written(&self) -> bool {
        self.all || !self.predicates.is_empty()
    }

    /// The predicates in effect for a repeat: this chain's when written,
    /// else the snippet's `default`.
    #[must_use]
    pub fn effective<'a>(&'a self, default: &'a [Predicate]) -> &'a [Predicate] {
        if self.is_written() {
            &self.predicates
        } else {
            default
        }
    }

    /// Parse a filter spec as written in a snippet pack (`filter =
    /// "vis=public?!async"`); predicates may be separated by `?` or
    /// whitespace, and an optional leading `?` is accepted.
    ///
    /// # Errors
    /// A message naming the first malformed predicate.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut filter = Self::default();
        for token in spec
            .split(|c: char| c == '?' || c.is_whitespace())
            .filter(|t| !t.is_empty())
        {
            if token == "all" {
                filter.all = true;
            } else {
                filter.predicates.push(Predicate::parse(token)?);
            }
        }
        Ok(filter)
    }
}

/// One `?predicate` of a repeat filter: `key`, `!key`, `key=text`,
/// `key~glob`, each optionally negated with a leading `!`.
///
/// Three keys are **positional** and test the copy's place in its repeat
/// rather than a member: `first`, `last`, and `i` (compared to the 1-based
/// repetition index, so `i=2` or `i~1*`). They hold with or without a
/// data-driven item — see [`Predicate::positional`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    /// Leading `!`: the item must *not* satisfy the test.
    pub negate: bool,
    /// The member name tested, `value` for the item's own text, or one of
    /// the positional keys `first` / `last` / `i`.
    pub key: String,
    /// The comparison; `None` tests presence (non-empty).
    pub op: Option<PredicateOp>,
}

/// How a [`Predicate`] compares a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateOp {
    /// `key=text`: equal to `text`.
    Eq(String),
    /// `key~glob`: matches `glob`, where `*` matches any run of characters.
    Glob(String),
}

impl Predicate {
    /// Parse one predicate token (no `?` separator).
    ///
    /// # Errors
    /// A message for an empty key, a non-identifier key, or an empty value.
    pub fn parse(token: &str) -> Result<Self, String> {
        let (negate, rest) = token
            .strip_prefix('!')
            .map_or((false, token), |rest| (true, rest));
        let key_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let key = &rest[..key_end];
        if key.is_empty() {
            return Err(format!(
                "filter `{token}`: expected a member name (as in `?vis=public`, `?!static`)"
            ));
        }
        let op = match &rest[key_end..] {
            "" => None,
            value if value.starts_with('=') => Some(PredicateOp::Eq(value[1..].to_owned())),
            value if value.starts_with('~') => Some(PredicateOp::Glob(value[1..].to_owned())),
            other => {
                return Err(format!(
                    "filter `{token}`: unexpected `{other}` after `{key}` (expected `=text` or `~glob`)"
                ));
            }
        };
        if let Some(PredicateOp::Eq(v) | PredicateOp::Glob(v)) = &op {
            if v.is_empty() {
                return Err(format!(
                    "filter `{token}`: missing a value after the operator"
                ));
            }
            if v.starts_with(['=', '~']) {
                return Err(format!(
                    "filter `{token}`: a value may not start with `=` or `~` (one operator only)"
                ));
            }
        }
        Ok(Self {
            negate,
            key: key.to_owned(),
            op,
        })
    }

    /// Whether `item` satisfies this predicate.
    #[must_use]
    pub fn matches(&self, item: &crate::context::Item) -> bool {
        let field: Option<&str> = if self.key == "value" {
            Some(item.value.as_str())
        } else {
            item.members.get(&self.key).map(String::as_str)
        };
        let hit = match &self.op {
            None => field.is_some_and(|f| !f.is_empty()),
            Some(PredicateOp::Eq(text)) => field == Some(text.as_str()),
            Some(PredicateOp::Glob(glob)) => field.is_some_and(|f| glob_match(glob, f)),
        };
        hit != self.negate
    }

    /// Whether this predicate is positional (`first`, `last`, `i`): it tests
    /// the copy's place in the repeat, not a member of its item.
    #[must_use]
    pub fn is_positional(&self) -> bool {
        matches!(self.key.as_str(), "first" | "last" | "i")
    }

    /// Evaluate a positional predicate for copy `index` (1-based) of `count`.
    ///
    /// `first` holds for index 1, `last` for index `count`, and `i` compares
    /// the index — `i=2` exactly, `i~1*` as a glob — as text. Negation
    /// applies as usual. Returns `None` for a member predicate, which needs
    /// an item ([`Predicate::matches`]).
    #[must_use]
    pub fn positional(&self, index: u32, count: usize) -> Option<bool> {
        let hit = match self.key.as_str() {
            "first" => index == 1,
            "last" => usize::try_from(index).is_ok_and(|i| i == count),
            "i" => {
                let text = index.to_string();
                match &self.op {
                    None => true,
                    Some(PredicateOp::Eq(expected)) => &text == expected,
                    Some(PredicateOp::Glob(glob)) => glob_match(glob, &text),
                }
            }
            _ => return None,
        };
        Some(hit != self.negate)
    }
}

/// Whether `text` matches `pattern`, where `*` matches any run of characters
/// (including none) and every other character matches itself.
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark): (Option<usize>, usize) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use crate::context::Item;

    fn item(value: &str, members: &[(&str, &str)]) -> Item {
        let mut item = Item::new(value);
        for (k, v) in members {
            item.members.insert((*k).to_owned(), (*v).to_owned());
        }
        item
    }

    #[test]
    fn glob_basics() {
        assert!(glob_match("get_*", "get_name"));
        assert!(glob_match("*_test", "a_test"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("get_*", "set_name"));
        assert!(!glob_match("abc", "abcd"));
    }

    #[test]
    fn predicate_forms() {
        let pub_fn = item("area", &[("vis", "public"), ("async", "async ")]);
        let private = item("_helper", &[("vis", "private")]);
        assert!(
            Predicate::parse("vis=public")
                .expect("predicate should parse")
                .matches(&pub_fn)
        );
        assert!(
            !Predicate::parse("vis=public")
                .expect("predicate should parse")
                .matches(&private)
        );
        assert!(
            Predicate::parse("async")
                .expect("predicate should parse")
                .matches(&pub_fn)
        );
        assert!(
            Predicate::parse("!async")
                .expect("predicate should parse")
                .matches(&private)
        );
        assert!(
            Predicate::parse("value~_*")
                .expect("predicate should parse")
                .matches(&private)
        );
        assert!(
            Predicate::parse("!value~_*")
                .expect("predicate should parse")
                .matches(&pub_fn)
        );
        // An absent member never equals anything and is "not present".
        assert!(
            !Predicate::parse("ret=f64")
                .expect("predicate should parse")
                .matches(&pub_fn)
        );
        assert!(
            Predicate::parse("!ret")
                .expect("predicate should parse")
                .matches(&pub_fn)
        );
    }

    #[test]
    fn predicate_errors() {
        for bad in ["", "!", "=x", "vis=", "vis~", "vis<3", "vis==", "vis=~x"] {
            assert!(Predicate::parse(bad).is_err(), "`{bad}` should be rejected");
        }
    }

    #[test]
    fn filter_spec_and_effective_default() {
        let f = Filter::parse("vis=public ?!async").expect("filter should parse");
        assert_eq!(f.predicates.len(), 2);
        assert!(!f.all);
        let default = vec![Predicate::parse("vis=public").expect("predicate should parse")];
        assert_eq!(Filter::NONE.effective(&default), default.as_slice());
        let all = Filter::parse("all").expect("filter should parse");
        assert!(all.all && all.effective(&default).is_empty());
        let all_async = Filter::parse("?all?async").expect("filter should parse");
        assert_eq!(all_async.effective(&default).len(), 1);
    }
}

/// One node of the abbreviation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbbrNode {
    /// The raw WORD token (trigger plus any glued arg, e.g. `fori10`).
    pub word: String,
    /// Explicit `:`-args, already unquoted and unescaped, in source order.
    pub args: Vec<String>,
    /// How the node repeats: a literal `*n`, or a data-driven `*@name` / `*`
    /// (with its `?predicate` filter).
    pub repeat: Repeat,
    /// Nodes attached with `>`.
    pub children: Vec<Self>,
}

/// A parse failure with a byte-offset span into the abbreviation text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at byte {}..{}", span.start, span.end)]
pub struct ParseError {
    /// Human-readable description of the failure.
    pub message: String,
    /// Byte range in the input the error points at.
    pub span: Range<usize>,
}
