//! Exercise model and auto-generation.
//!
//! Everything here is pure: given an [`Engine`], a language, a [`Level`], and a
//! seeded [`Rng`], `generate` builds a renderable exercise by composing real
//! triggers with the composition operators and rendering the result through the
//! engine. Because the target is produced by the engine, a generated exercise
//! can never disagree with actual expansion behavior.

use std::collections::BTreeMap;

use codemmet_core::config::loader::{CompiledSnippet, SnippetSet};
use codemmet_core::config::schema::ParamKind;
use codemmet_core::context::{Collection, ExpansionContext};
use codemmet_core::engine::Engine;
use codemmet_core::snippet::ir::Span;
use codemmet_core::snippet::render::TabstopMode;

use super::rng::Rng;

/// How many times `generate` retries composing a renderable abbreviation
/// before falling back to a bare trigger.
const GEN_ATTEMPTS: usize = 48;
/// How many times decoy generation retries before giving up.
const DECOY_ATTEMPTS: usize = 48;
/// Target number of decoys for a multiple-choice forward exercise.
const DECOY_TARGET: usize = 3;

/// Identifier argument pool for generated calls.
const IDENTS: &[&str] = &["i", "x", "item", "node", "value"];
/// Integer argument pool.
const INTS: &[&str] = &["3", "5", "10"];
/// String argument pool.
const STRS: &[&str] = &["msg", "label", "name"];

/// A staged difficulty level: composition operators first, then vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A single trigger with no arguments (`pr`, `ret`).
    SingleTrigger,
    /// A trigger with arguments (`fn:main`, `fori:i,10`).
    Arguments,
    /// Child nesting with `>` (`fn:main>pr`).
    Nesting,
    /// Siblings with `+` (`if+el`).
    Siblings,
    /// Repetition with `*` (`arm*3`).
    Repeat,
    /// Climb-up with `^` (`fn:main>pr^ret`).
    ClimbUp,
    /// Grouping with `()` (`(fn:main>pr)+ret`).
    Grouping,
    /// Composite triggers that expand to a whole scaffold (`dstruct:Point`).
    Composite,
    /// Data-driven repeats from an inline collection literal
    /// (`struct:P>field*[x:i32,y:f64]`, `impl:Point>fn*[new,len]`).
    DataDriven,
    /// Item members and transforms: `${value.type}` through a body,
    /// `|snake` re-casing, `|native` / `|sql` type maps
    /// (`struct:P>field*[x:float,name:str]`, `fn:${value|snake}*[maxSize]`).
    Members,
    /// Named collections from the workspace: `*@Name`, `?filters`, zips by
    /// name, and payload members (`match:m>arm*@Msg?!name@Handled`,
    /// `convert:Raw,Point`, `stub*@src.Shape.methods`).
    Collections,
}

impl Level {
    /// Every level, in teaching order.
    pub const ALL: [Self; 11] = [
        Self::SingleTrigger,
        Self::Arguments,
        Self::Nesting,
        Self::Siblings,
        Self::Repeat,
        Self::ClimbUp,
        Self::Grouping,
        Self::Composite,
        Self::DataDriven,
        Self::Members,
        Self::Collections,
    ];

    /// The 1-based level number.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::SingleTrigger => 1,
            Self::Arguments => 2,
            Self::Nesting => 3,
            Self::Siblings => 4,
            Self::Repeat => 5,
            Self::ClimbUp => 6,
            Self::Grouping => 7,
            Self::Composite => 8,
            Self::DataDriven => 9,
            Self::Members => 10,
            Self::Collections => 11,
        }
    }

    /// The level for a 1-based number, if in range.
    #[must_use]
    pub const fn from_number(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::SingleTrigger),
            2 => Some(Self::Arguments),
            3 => Some(Self::Nesting),
            4 => Some(Self::Siblings),
            5 => Some(Self::Repeat),
            6 => Some(Self::ClimbUp),
            7 => Some(Self::Grouping),
            8 => Some(Self::Composite),
            9 => Some(Self::DataDriven),
            10 => Some(Self::Members),
            11 => Some(Self::Collections),
            _ => None,
        }
    }

    /// A short human title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::SingleTrigger => "single triggers",
            Self::Arguments => "arguments",
            Self::Nesting => "nesting (>)",
            Self::Siblings => "siblings (+)",
            Self::Repeat => "repeat (*)",
            Self::ClimbUp => "climb-up (^)",
            Self::Grouping => "grouping ( )",
            Self::Composite => "composites",
            Self::DataDriven => "data-driven (*[…])",
            Self::Members => "members & transforms (${value.type}, |snake)",
            Self::Collections => "collections (*@Name, ?filter)",
        }
    }

    /// A teaching blurb shown on the lesson's intro screen.
    #[must_use]
    pub const fn blurb(self) -> &'static str {
        match self {
            Self::SingleTrigger => {
                "A trigger is a short keyword that expands to a construct.\n\
                 Type the trigger on its own, e.g. `pr` or `ret`."
            }
            Self::Arguments => {
                "Triggers take arguments. Glue them on (`fori10`, `fn:main`) or pass\n\
                 them explicitly after a colon, comma-separated (`fori:i,10`)."
            }
            Self::Nesting => {
                "`>` nests the right side inside the left as its child:\n\
                 `fn:main>pr` puts a println inside a main function."
            }
            Self::Siblings => {
                "`+` places two constructs side by side, as siblings:\n\
                 `if+el` yields an if followed by an else."
            }
            Self::Repeat => {
                "`*n` repeats a construct n times, each with its own tabstops:\n\
                 `arm*3` produces three match arms."
            }
            Self::ClimbUp => {
                "`^` climbs back up out of the current child, so the next construct\n\
                 is a sibling of the parent: `fn:main>pr^ret`."
            }
            Self::Grouping => {
                "`( )` groups a whole subtree so operators apply to it as a unit:\n\
                 `(fn:main>pr)+ret` — the group, then a sibling."
            }
            Self::Composite => {
                "A composite trigger expands to a whole scaffold built from other\n\
                 triggers: `dstruct:Point` yields a derived struct, `classp:Point`\n\
                 a class with the common dunders. Use one like any other trigger."
            }
            Self::DataDriven => {
                "`*[a,b,c]` repeats a node once per item in an inline collection,\n\
                 filling each copy from that item: `field*[x:i32,y:f64]` writes both\n\
                 fields, `fn*[new,len]` names each method. `${outer}` reaches the\n\
                 enclosing repeat's item when they nest."
            }
            Self::Members => {
                "An item can carry members: `*[x:f64,count:u32]` gives each item a\n\
                 `type`, which a body reads as `${value.type}` (`field`, `sqlcol`).\n\
                 A `|transform` rewrites the item where it is used: `|snake`, `|pascal`\n\
                 re-case it (`fn:${value|snake}*[maxSize]`), while a map like `|native`\n\
                 or `|sql` looks it up — `field` turns a Python `float` into `f64`,\n\
                 `sqlcol` turns `u32` into `INTEGER`."
            }
            Self::Collections => {
                "`*@Name` repeats over a named collection — here the workspace listed\n\
                 on screen, in an editor the enums and structs of your file. `?key=v`,\n\
                 `?!key`, `?first` narrow it; `?!name@Handled` keeps the items another\n\
                 collection lacks; `?value=@outer` pairs nested repeats by name.\n\
                 Variants carry their payload (`armb` binds it), `src.fns` the API,\n\
                 and `src.Shape.methods` an interface's members (`stub` implements them)."
            }
        }
    }
}

/// The exercise direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Given target code, produce an abbreviation.
    Reverse,
    /// Given an abbreviation, predict the expansion.
    Forward,
}

/// A concept an exercise touches: used for hints and as a spaced-repetition
/// item (hence the ordering and serde derives).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Concept {
    /// A trigger word.
    Trigger(String),
    /// A composition operator character.
    Operator(char),
}

impl Concept {
    /// A stable key for use as a map key in persisted review state.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Trigger(t) => format!("t:{t}"),
            Self::Operator(op) => format!("o:{op}"),
        }
    }

    /// The level whose exercises best drill this concept.
    #[must_use]
    pub const fn level(&self) -> Level {
        match self {
            Self::Trigger(_) => Level::Arguments,
            Self::Operator('>') => Level::Nesting,
            Self::Operator('+') => Level::Siblings,
            Self::Operator('*') => Level::Repeat,
            Self::Operator('^') => Level::ClimbUp,
            // `[` is the inline collection literal `*[…]` of a data-driven repeat.
            Self::Operator('[') => Level::DataDriven,
            // `.` is a member reference, `|` a transform.
            Self::Operator('.' | '|') => Level::Members,
            // `@` names a collection, `?` filters it.
            Self::Operator('@' | '?') => Level::Collections,
            Self::Operator(_) => Level::Grouping,
        }
    }
}

/// A single generated exercise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exercise {
    /// The language id this exercise is for.
    pub language: String,
    /// The difficulty level.
    pub level: Level,
    /// Reverse (produce the abbreviation) or forward (predict the code).
    pub direction: Direction,
    /// A canonical abbreviation that produces `target`.
    pub abbreviation: String,
    /// The strip-mode expansion of `abbreviation`.
    pub target: String,
    /// Multiple-choice options for a forward exercise; empty means free-type.
    pub choices: Vec<String>,
    /// Index of the correct option within `choices` (meaningless when empty).
    pub answer_index: usize,
    /// Triggers and operators involved, for hints.
    pub concepts: Vec<Concept>,
}

impl Exercise {
    /// Whether the forward exercise is answered by choosing from `choices`.
    #[must_use]
    pub fn is_multiple_choice(&self) -> bool {
        self.direction == Direction::Forward && !self.choices.is_empty()
    }
}

/// Triggers of a language bucketed by shape, borrowed from a [`SnippetSet`].
struct Vocab<'a> {
    /// Triggers with no parameters.
    no_param: Vec<&'a CompiledSnippet>,
    /// Triggers with at least one parameter.
    with_param: Vec<&'a CompiledSnippet>,
    /// Triggers whose template has a `${child}` slot (block constructs).
    blocks: Vec<&'a CompiledSnippet>,
    /// Triggers with no `${child}` slot (statement-like leaves).
    leaves: Vec<&'a CompiledSnippet>,
    /// Composite triggers (aliases that expand to a scaffold). Drilled only at
    /// [`Level::Composite`], so they stay out of the operator levels where a
    /// whole scaffold would swamp the concept being taught.
    composites: Vec<&'a CompiledSnippet>,
    /// Leaves whose body reads the repeat item (`${value…}`) *without* a
    /// member — arms, cases, variants: they consume an enum-like collection.
    consumers: Vec<&'a CompiledSnippet>,
    /// Leaves whose body reads `${value.type}` — fields, columns: they
    /// consume a fields collection, and show members off.
    typed: Vec<&'a CompiledSnippet>,
}

impl<'a> Vocab<'a> {
    fn from_set(set: &'a SnippetSet) -> Self {
        let mut v = Self {
            no_param: Vec::new(),
            with_param: Vec::new(),
            blocks: Vec::new(),
            leaves: Vec::new(),
            composites: Vec::new(),
            consumers: Vec::new(),
            typed: Vec::new(),
        };
        for s in set.iter() {
            if s.composite.is_some() {
                v.composites.push(s);
                continue;
            }
            if s.params.is_empty() {
                v.no_param.push(s);
            } else {
                v.with_param.push(s);
            }
            if has_child_slot(s) {
                v.blocks.push(s);
            } else {
                v.leaves.push(s);
                match value_members(s) {
                    Some(members) if members.iter().any(|m| m == "type") => v.typed.push(s),
                    // Leaves reading only the item (or `name`/`bind`) take an
                    // enum-like collection; ones with a default filter
                    // (`tfn`) are tied to `src.*` and stay out.
                    Some(_) if s.filter.is_empty() => v.consumers.push(s),
                    _ => {}
                }
            }
        }
        v
    }
}

/// The members a snippet's default body reads from the repeat item
/// (`${value.key}` → `key`; a bare `${value}` contributes `""`), or `None`
/// when the body never reads the item.
fn value_members(snip: &CompiledSnippet) -> Option<Vec<String>> {
    let members: Vec<String> = snip
        .template
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .filter_map(|span| match span {
            Span::ItemValue {
                outer: false,
                member,
                ..
            } => Some(member.clone().unwrap_or_default()),
            _ => None,
        })
        .collect();
    (!members.is_empty()).then_some(members)
}

/// The workspace collections split by shape: `(fields, enums)` — fields
/// collections carry a `type` member, enum-like ones do not; `src.*` is
/// neither.
fn collection_pools(collections: &BTreeMap<String, Collection>) -> (Vec<&str>, Vec<&str>) {
    let mut fields = Vec::new();
    let mut enums = Vec::new();
    for (name, collection) in collections {
        if name.starts_with("src.") || collection.items.is_empty() {
            continue;
        }
        if collection.items[0].members.contains_key("type") {
            fields.push(name.as_str());
        } else {
            enums.push(name.as_str());
        }
    }
    (fields, enums)
}

/// Typed inline literals for the members level: Rust spellings, which the
/// other packs' `|native` maps respell (and Rust passes through).
const TYPED_LITERALS: &[&str] = &[
    "[x:f64,y:f64]",
    "[x:f64,count:u32]",
    "[name:String,count:u32]",
    "[x:f64,tags:Vec<String>]",
];
/// Mixed-case names for the `|snake` / `|pascal` transform exercises.
const CASED_LITERALS: &[&str] = &[
    "[maxSize,HTTPError]",
    "[maxSize,minSize]",
    "[readFile,putFile]",
];
/// Positional filters that hold for any collection.
const FILTERS: &[&str] = &["", "", "?first", "?last", "?!first"];

/// Whether a snippet's template contains a `${child}` slot.
fn has_child_slot(snip: &CompiledSnippet) -> bool {
    snip.template
        .lines
        .iter()
        .any(|line| line.spans.iter().any(|s| matches!(s, Span::ChildSlot)))
}

/// A pool value for a parameter of the given kind.
fn arg_for(kind: ParamKind, rng: &mut Rng) -> String {
    let pool = match kind {
        ParamKind::Ident => IDENTS,
        ParamKind::Int => INTS,
        ParamKind::Str => STRS,
    };
    rng.choice(pool).copied().unwrap_or("x").to_string()
}

/// Render one trigger call, filling any parameters with explicit args, and
/// record the trigger as a concept.
fn call(snip: &CompiledSnippet, rng: &mut Rng, concepts: &mut Vec<Concept>) -> String {
    concepts.push(Concept::Trigger(snip.trigger.clone()));
    if snip.params.is_empty() {
        snip.trigger.clone()
    } else {
        let args: Vec<String> = snip.params.iter().map(|p| arg_for(p.kind, rng)).collect();
        format!("{}:{}", snip.trigger, args.join(","))
    }
}

/// Compose a candidate abbreviation for `level`, or `None` when the vocabulary
/// lacks a trigger of the shape a level needs.
fn compose(
    vocab: &Vocab,
    level: Level,
    collections: &BTreeMap<String, Collection>,
    rng: &mut Rng,
) -> Option<(String, Vec<Concept>)> {
    let mut concepts = Vec::new();
    let abbr = match level {
        Level::SingleTrigger => {
            let s = rng.choice(&vocab.no_param).copied()?;
            call(s, rng, &mut concepts)
        }
        Level::Arguments => {
            let s = rng.choice(&vocab.with_param).copied()?;
            call(s, rng, &mut concepts)
        }
        Level::Nesting => {
            let a = rng.choice(&vocab.blocks).copied()?;
            let b = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            let sb = call(b, rng, &mut concepts);
            concepts.push(Concept::Operator('>'));
            format!("{sa}>{sb}")
        }
        Level::Siblings => {
            let a = rng.choice(&vocab.leaves).copied()?;
            let b = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            let sb = call(b, rng, &mut concepts);
            concepts.push(Concept::Operator('+'));
            format!("{sa}+{sb}")
        }
        Level::Repeat => {
            let a = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            let n = rng.choice(&[2_usize, 3]).copied().unwrap_or(2);
            concepts.push(Concept::Operator('*'));
            format!("{sa}*{n}")
        }
        Level::ClimbUp => {
            let a = rng.choice(&vocab.blocks).copied()?;
            let b = rng.choice(&vocab.leaves).copied()?;
            let c = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            let sb = call(b, rng, &mut concepts);
            let sc = call(c, rng, &mut concepts);
            concepts.push(Concept::Operator('>'));
            concepts.push(Concept::Operator('^'));
            format!("{sa}>{sb}^{sc}")
        }
        Level::Grouping => {
            let a = rng.choice(&vocab.blocks).copied()?;
            let b = rng.choice(&vocab.leaves).copied()?;
            let c = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            let sb = call(b, rng, &mut concepts);
            let sc = call(c, rng, &mut concepts);
            concepts.push(Concept::Operator('('));
            concepts.push(Concept::Operator('+'));
            format!("({sa}>{sb})+{sc}")
        }
        Level::Composite => {
            let s = rng.choice(&vocab.composites).copied()?;
            call(s, rng, &mut concepts)
        }
        Level::DataDriven => {
            let a = rng.choice(&vocab.blocks).copied()?;
            let b = rng.choice(&vocab.leaves).copied()?;
            let sa = call(a, rng, &mut concepts);
            // Repeat the child from an inline collection literal: each copy
            // auto-binds its item (or just repeats, for a param-less leaf).
            concepts.push(Concept::Trigger(b.trigger.clone()));
            concepts.push(Concept::Operator('['));
            format!("{sa}>{}*[a,b,c]", b.trigger)
        }
        Level::Members => compose_members(vocab, rng, &mut concepts)?,
        Level::Collections => compose_collections(vocab, collections, rng, &mut concepts)?,
    };
    Some((abbr, concepts))
}

/// A members-level abbreviation: a typed inline literal through a
/// `${value.type}` body, or a re-cased item in argument position.
fn compose_members(vocab: &Vocab, rng: &mut Rng, concepts: &mut Vec<Concept>) -> Option<String> {
    let a = rng.choice(&vocab.blocks).copied()?;
    let sa = call(a, rng, concepts);
    concepts.push(Concept::Operator('['));
    let typed = rng.choice(&vocab.typed).copied();
    let cased = rng.choice(&vocab.with_param).copied();
    let use_typed = typed.is_some() && (cased.is_none() || rng.coin());
    if use_typed {
        let b = typed?;
        concepts.push(Concept::Trigger(b.trigger.clone()));
        concepts.push(Concept::Operator('.'));
        let items = rng.choice(TYPED_LITERALS).copied().unwrap_or("[x:f64]");
        Some(format!("{sa}>{}*{items}", b.trigger))
    } else {
        let b = cased?;
        concepts.push(Concept::Trigger(b.trigger.clone()));
        concepts.push(Concept::Operator('|'));
        let items = rng.choice(CASED_LITERALS).copied().unwrap_or("[maxSize]");
        let transform = rng.choice(&["snake", "pascal", "upper"]).copied()?;
        Some(format!("{sa}>{}:${{value|{transform}}}*{items}", b.trigger))
    }
}

/// A collections-level abbreviation: a typed leaf over a fields collection,
/// or an item consumer over an enum-like one, with a positional filter some
/// of the time.
fn compose_collections(
    vocab: &Vocab,
    collections: &BTreeMap<String, Collection>,
    rng: &mut Rng,
    concepts: &mut Vec<Concept>,
) -> Option<String> {
    let (fields, enums) = collection_pools(collections);
    let a = rng.choice(&vocab.blocks).copied()?;
    let sa = call(a, rng, concepts);
    let (b, name) = if rng.coin() && !fields.is_empty() {
        (
            rng.choice(&vocab.typed).copied()?,
            rng.choice(&fields).copied()?,
        )
    } else {
        (
            rng.choice(&vocab.consumers).copied()?,
            rng.choice(&enums).copied()?,
        )
    };
    concepts.push(Concept::Trigger(b.trigger.clone()));
    concepts.push(Concept::Operator('@'));
    let filter = rng.choice(FILTERS).copied().unwrap_or("");
    if !filter.is_empty() {
        concepts.push(Concept::Operator('?'));
    }
    Some(format!("{sa}>{}*@{name}{filter}", b.trigger))
}

/// The rendering context threaded through generation: an engine, a language,
/// and an expansion context.
struct Gen<'a> {
    engine: &'a Engine,
    language: &'a str,
    ctx: &'a ExpansionContext,
}

impl Gen<'_> {
    /// Render `abbr` to strip-mode text, or `None` when it fails or is empty.
    fn render(&self, abbr: &str) -> Option<String> {
        let body = self
            .engine
            .expand(abbr, self.language, self.ctx, TabstopMode::Strip)
            .ok()?
            .body;
        if body.trim().is_empty() {
            None
        } else {
            Some(body)
        }
    }

    /// Try to add `body` as a decoy if it is a distinct multi-line expansion
    /// other than `target`. Returns whether the pool is now full.
    fn take_decoy(decoys: &mut Vec<String>, body: String, target: &str) -> bool {
        if body.contains('\n') && body != target && !decoys.contains(&body) {
            decoys.push(body);
        }
        decoys.len() >= DECOY_TARGET
    }

    /// Distinct multi-line decoys for `target`, preferring near-misses: small
    /// perturbations of `abbr` (swapped operator, swapped sibling trigger),
    /// then random same-level exercises to top up.
    fn decoys(
        &self,
        vocab: &Vocab,
        level: Level,
        abbr: &str,
        target: &str,
        rng: &mut Rng,
    ) -> Vec<String> {
        let mut decoys: Vec<String> = Vec::new();
        for candidate in perturbations(vocab, abbr, rng) {
            if let Some(body) = self.render(&candidate)
                && Self::take_decoy(&mut decoys, body, target)
            {
                return decoys;
            }
        }
        for _ in 0..DECOY_ATTEMPTS {
            let Some((candidate, _)) = compose(vocab, level, &self.ctx.collections, rng) else {
                continue;
            };
            if let Some(body) = self.render(&candidate)
                && Self::take_decoy(&mut decoys, body, target)
            {
                break;
            }
        }
        decoys
    }

    /// Assemble a finished exercise from a rendered `draft`. `want` forces the
    /// direction when set, otherwise it is chosen at random. Multi-line forward
    /// exercises are multiple choice; a single-line target is free-typed; a
    /// multi-line target with no distinct decoys falls back to reverse rather
    /// than asking for many typed lines.
    fn finish(
        &self,
        vocab: &Vocab,
        draft: Draft,
        want: Option<Direction>,
        rng: &mut Rng,
    ) -> Exercise {
        let Draft {
            level,
            abbr,
            target,
            concepts,
        } = draft;
        let forward = want.map_or_else(|| rng.coin(), |d| d == Direction::Forward);
        let (direction, choices, answer_index) = if forward {
            if target.contains('\n') {
                let decoys = self.decoys(vocab, level, &abbr, &target, rng);
                if decoys.is_empty() {
                    (Direction::Reverse, Vec::new(), 0)
                } else {
                    let (choices, answer_index) = build_choices(&target, decoys, rng);
                    (Direction::Forward, choices, answer_index)
                }
            } else {
                (Direction::Forward, Vec::new(), 0)
            }
        } else {
            (Direction::Reverse, Vec::new(), 0)
        };
        Exercise {
            language: self.language.to_string(),
            level,
            direction,
            abbreviation: abbr,
            target,
            choices,
            answer_index,
            concepts,
        }
    }
}

/// A rendered exercise awaiting a direction and (maybe) multiple-choice options.
struct Draft {
    level: Level,
    abbr: String,
    target: String,
    concepts: Vec<Concept>,
}

/// Shuffle `target` in among `decoys`, returning the options and the target's
/// index.
fn build_choices(target: &str, mut all: Vec<String>, rng: &mut Rng) -> (Vec<String>, usize) {
    all.push(target.to_string());
    // Fisher-Yates.
    for i in (1..all.len()).rev() {
        all.swap(i, rng.below(i + 1));
    }
    let idx = all.iter().position(|c| c == target).unwrap_or(0);
    (all, idx)
}

/// Generate a renderable exercise for `language` at `level`.
///
/// Returns `None` only when the language is unknown or has no renderable
/// triggers at all.
#[must_use]
pub fn generate(
    engine: &Engine,
    language: &str,
    level: Level,
    ctx: &ExpansionContext,
    rng: &mut Rng,
) -> Option<Exercise> {
    let set = &engine.registry().language(language)?.set;
    let vocab = Vocab::from_set(set);
    let generator = Gen {
        engine,
        language,
        ctx,
    };

    for _ in 0..GEN_ATTEMPTS {
        if let Some((abbr, concepts)) = compose(&vocab, level, &ctx.collections, rng)
            && let Some(target) = generator.render(&abbr)
        {
            let draft = Draft {
                level,
                abbr,
                target,
                concepts,
            };
            return Some(generator.finish(&vocab, draft, None, rng));
        }
    }

    // Fallback: the simplest renderable single trigger, so a session never
    // stalls even for a sparse pack.
    for s in &vocab.no_param {
        if let Some(target) = generator.render(&s.trigger) {
            let draft = Draft {
                level: Level::SingleTrigger,
                abbr: s.trigger.clone(),
                target,
                concepts: vec![Concept::Trigger(s.trigger.clone())],
            };
            return Some(generator.finish(&vocab, draft, None, rng));
        }
    }
    None
}

/// Candidate near-miss abbreviations derived from `abbr`: swap one operator, or
/// swap the leading trigger for a same-class sibling.
fn perturbations(vocab: &Vocab, abbr: &str, rng: &mut Rng) -> Vec<String> {
    let mut out = Vec::new();
    for (from, to) in [('>', '+'), ('+', '>'), ('^', '>'), ('>', '^')] {
        if abbr.contains(from) {
            out.push(abbr.replacen(from, &to.to_string(), 1));
        }
    }
    if let Some(swapped) = swap_leading_trigger(vocab, abbr, rng) {
        out.push(swapped);
    }
    out
}

/// Replace the first element's trigger with a random same-class sibling,
/// dropping that element's arguments. `None` if there is no sibling to use.
fn swap_leading_trigger(vocab: &Vocab, abbr: &str, rng: &mut Rng) -> Option<String> {
    let boundary = abbr
        .find(['>', '+', '^', '*', '(', ')'])
        .unwrap_or(abbr.len());
    let (head, rest) = abbr.split_at(boundary);
    let word_end = head.find([':', ',']).unwrap_or(head.len());
    let word = &head[..word_end];
    let is_block = vocab.blocks.iter().any(|s| s.trigger == word);
    let pool = if is_block {
        &vocab.blocks
    } else {
        &vocab.leaves
    };
    let sibling = rng.choice(pool)?;
    if sibling.trigger == word {
        return None;
    }
    Some(format!("{}{rest}", sibling.trigger))
}

/// Build an exercise from a specific, hand-authored abbreviation.
///
/// Used by curated lessons. `want` forces the direction; `None` lets it vary by
/// seed. Returns `None` if the abbreviation does not render for `language`.
#[must_use]
pub fn from_abbreviation(
    engine: &Engine,
    language: &str,
    level: Level,
    abbr: &str,
    want: Option<Direction>,
    ctx: &ExpansionContext,
    rng: &mut Rng,
) -> Option<Exercise> {
    let set = &engine.registry().language(language)?.set;
    let vocab = Vocab::from_set(set);
    let generator = Gen {
        engine,
        language,
        ctx,
    };
    let target = generator.render(abbr)?;
    let concepts = concepts_in(abbr, set);
    let draft = Draft {
        level,
        abbr: abbr.to_string(),
        target,
        concepts,
    };
    Some(generator.finish(&vocab, draft, want, rng))
}

/// The concepts an abbreviation touches: the leading trigger of each element
/// (arguments excluded) plus the composition operators present.
fn concepts_in(abbr: &str, set: &SnippetSet) -> Vec<Concept> {
    let mut concepts = Vec::new();
    for element in abbr.split(|c: char| "><+*^()".contains(c)) {
        let word = element.trim().split([':', ',', ' ']).next().unwrap_or("");
        if word.is_empty() {
            continue;
        }
        if let Some((snippet, _)) = set.resolve_word(word) {
            let concept = Concept::Trigger(snippet.trigger.clone());
            if !concepts.contains(&concept) {
                concepts.push(concept);
            }
        }
    }
    for op in ['>', '+', '*', '^', '('] {
        if abbr.contains(op) {
            concepts.push(Concept::Operator(op));
        }
    }
    // The data-driven forms: an inline literal, a named collection, a
    // filter, a member reference, a transform.
    for (needle, op) in [
        ("*[", '['),
        ("*@", '@'),
        ("?", '?'),
        ("${value.", '.'),
        ("|", '|'),
    ] {
        if abbr.contains(needle) {
            concepts.push(Concept::Operator(op));
        }
    }
    concepts
}

/// Generate an exercise that drills a specific concept (for spaced repetition).
///
/// A trigger item yields an exercise built around that exact trigger; an
/// operator item yields a normal exercise at the operator's level. Returns
/// `None` when the concept can no longer be rendered (e.g. a trigger the pack
/// dropped).
#[must_use]
pub fn generate_for_concept(
    engine: &Engine,
    language: &str,
    concept: &Concept,
    ctx: &ExpansionContext,
    rng: &mut Rng,
) -> Option<Exercise> {
    let Concept::Trigger(name) = concept else {
        return generate(engine, language, concept.level(), ctx, rng);
    };
    let set = &engine.registry().language(language)?.set;
    let snippet = set.get(name)?;
    let vocab = Vocab::from_set(set);
    let generator = Gen {
        engine,
        language,
        ctx,
    };
    let mut concepts = Vec::new();
    let abbr = call(snippet, rng, &mut concepts);
    let target = generator.render(&abbr)?;
    let level = if snippet.params.is_empty() {
        Level::SingleTrigger
    } else {
        Level::Arguments
    };
    let draft = Draft {
        level,
        abbr,
        target,
        concepts,
    };
    Some(generator.finish(&vocab, draft, None, rng))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Outcome, grade_reverse};
    use codemmet_core::context::IndentUnit;
    use codemmet_core::lang::registry::Registry;

    fn engine() -> Engine {
        Engine::with_registry(Registry::with_builtins().expect("builtins should load"))
    }

    fn ctx() -> ExpansionContext {
        let mut ctx = ExpansionContext::plain(String::new(), IndentUnit::Spaces(4));
        ctx.collections = crate::workspace::collections("rust");
        ctx
    }

    #[test]
    fn generates_a_renderable_self_consistent_exercise_at_every_level() {
        let engine = engine();
        let ctx = ctx();
        for level in Level::ALL {
            // A few seeds so we exercise different vocabulary picks.
            for seed in 0..8_u64 {
                let mut rng = Rng::new(seed.wrapping_mul(0x9E37_79B9).wrapping_add(1));
                let ex = generate(&engine, "rust", level, &ctx, &mut rng)
                    .expect("rust has renderable triggers");
                assert!(!ex.target.trim().is_empty(), "empty target at {level:?}");
                // The canonical abbreviation must grade correct against its own
                // target — generation and grading agree by construction.
                let outcome = grade_reverse(&engine, "rust", &ctx, &ex.abbreviation, &ex.target);
                assert_eq!(
                    outcome,
                    Outcome::Correct,
                    "canonical answer {:?} did not reproduce target at {level:?}",
                    ex.abbreviation,
                );
            }
        }
    }

    #[test]
    fn generation_is_deterministic_for_a_seed() {
        let engine = engine();
        let ctx = ctx();
        let mut a = Rng::new(12345);
        let mut b = Rng::new(12345);
        let ea =
            generate(&engine, "rust", Level::Nesting, &ctx, &mut a).expect("exercise generated");
        let eb =
            generate(&engine, "rust", Level::Nesting, &ctx, &mut b).expect("exercise generated");
        assert_eq!(ea, eb);
    }

    #[test]
    fn multiple_choice_answer_index_points_at_the_target() {
        // Search seeds for a multi-line forward exercise and check the answer.
        let engine = engine();
        let ctx = ctx();
        let mut found = false;
        for seed in 0..200_u64 {
            let mut rng = Rng::new(seed.wrapping_add(1));
            let ex = generate(&engine, "rust", Level::Nesting, &ctx, &mut rng)
                .expect("exercise generated");
            if ex.is_multiple_choice() {
                assert_eq!(ex.choices[ex.answer_index], ex.target);
                assert!(ex.choices.len() >= 2);
                found = true;
                break;
            }
        }
        assert!(found, "no multiple-choice exercise generated in 200 seeds");
    }

    #[test]
    fn levels_round_trip_through_numbers() {
        for level in Level::ALL {
            assert_eq!(Level::from_number(level.number()), Some(level));
        }
        assert_eq!(Level::from_number(0), None);
        assert_eq!(Level::from_number(12), None);
    }
}
