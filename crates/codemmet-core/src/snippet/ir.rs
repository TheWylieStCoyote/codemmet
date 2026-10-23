//! The snippet intermediate representation.
//!
//! Template bodies parse into lines with *relative indent depth*; braces are
//! plain text and the renderer owns indentation, so brace languages and
//! indentation languages (Python) share one pipeline.

use crate::snippet::transform::Transform;

/// A parsed snippet body: what a TOML `body` string compiles to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// The template's lines, in order.
    pub lines: Vec<IrLine>,
}

/// One line of a template with its indent depth relative to the snippet root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLine {
    /// Indent depth relative to the snippet's first line (which is depth 0).
    pub depth: u32,
    /// The line's content, in order. Empty for a blank line.
    pub spans: Vec<Span>,
}

/// A run of content within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    /// Literal text, exactly as it should appear.
    Text(String),
    /// A numbered tabstop from `$n` / `${n:placeholder}` in the body.
    ///
    /// Indices are template-local; the renderer renumbers globally. A
    /// repeated index within one template becomes a mirror at render time.
    Tabstop {
        /// Template-local tabstop index (never 0; `$0` is [`Span::FinalCursor`]).
        index: u32,
        /// Placeholder text, if the body used `${n:placeholder}`.
        placeholder: Option<String>,
    },
    /// A named parameter reference from `${name}` / `${name|transform}` in
    /// the body.
    ///
    /// Renders as literal text when the abbreviation supplied an argument
    /// (rewritten by `transform` when one is given), otherwise as a tabstop
    /// whose placeholder is the param's default — always verbatim: transforms
    /// apply only to caller-supplied text. Repeated references to one name
    /// become mirrors.
    Param {
        /// The parameter name; must exist in the snippet's `params` list.
        name: String,
        /// The transform from `${name|snake}` / `${name|native}`, if any.
        transform: Option<Transform>,
    },
    /// `${i}`: the 1-based repetition index of the enclosing node.
    ///
    /// Renders the copy number when the node is repeated with `*n` (`1`, `2`,
    /// …), and `1` when the node is not repeated.
    RepeatIndex,
    /// `${value}` / `${value.key}` / `${value|transform}` / `${value:default}`
    /// (and the `${outer…}` forms): a data-driven repeat item. `${value}` is
    /// the current level's item; `${outer}` is the nearest *enclosing*
    /// data-driven item (for nested repeats). Renders the item's value (or a
    /// named member), rewritten by `transform` when one is given (`|snake`
    /// re-cases, `|native` looks the text up in a pack map); otherwise a
    /// tabstop carrying `default` — always verbatim: transforms apply only to
    /// data-supplied text. Token order: `source → .member → |transform →
    /// :default`.
    ItemValue {
        /// `true` for `${outer…}` (the enclosing item), `false` for `${value…}`.
        outer: bool,
        /// The member key from `${value.key}`; `None` for the primary value.
        member: Option<String>,
        /// The transform from `${value|snake}` / `${value.type|native}`, if any.
        transform: Option<Transform>,
        /// The `${value:default}` fallback, used when there is no item.
        default: Option<String>,
    },
    /// `${src.key}` / `${src.key|transform}` / `${src.key:default}`: a
    /// source-file variable (`module`, `file`, `stem`, `dir`, `path`),
    /// resolved from [`ExpansionContext::vars`](crate::context::ExpansionContext).
    /// Renders the value (rewritten by `transform`), or — when no source is
    /// known — a tabstop carrying `default`, else the key itself, so the
    /// user always gets a hole to fill rather than silent empty text.
    Variable {
        /// The key after `src.`.
        key: String,
        /// The transform from `${src.key|snake}`, if any.
        transform: Option<Transform>,
        /// The `${src.key:default}` fallback placeholder.
        default: Option<String>,
    },
    /// `${child}`: where `>`-nested children render, one indent level deeper.
    ///
    /// At most one per template. For a leaf node this degrades to the final
    /// cursor position.
    ChildSlot,
    /// `$0`: the final cursor position after all tabstops.
    FinalCursor,
    /// `${join}`, allowed only at the very start of a body: when this
    /// template is one copy of a repeat, its first line is appended to the
    /// previous copy's last line instead of starting a new one — how an
    /// `else if` branch lands on the `}` that closed the branch before it.
    /// The first copy drops the marker (and the whitespace after it).
    Join,
}
