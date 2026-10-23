//! Hand-authored lesson progressions.
//!
//! Guided lessons draw from a curated, deliberately-ordered set of
//! abbreviations per level so the experience is stable and pedagogical, unlike
//! the randomized drills. Each supported language has its own progression,
//! keyed by canonical language id (the trainer normalizes aliases before
//! looking up). Every abbreviation here is rendered through the real engine
//! (and a test asserts each one renders), so curated content can never drift
//! from actual behavior. The collections level repeats over the trainer's
//! [workspace](super::workspace), which every session seeds into its context.

use super::exercise::Level;

/// The curated abbreviations for `language` at `level`, in teaching order.
/// Empty when the language has no curated progression (falls back to generated).
#[must_use]
pub fn curated(language: &str, level: Level) -> &'static [&'static str] {
    match language {
        "rust" => rust(level),
        "python" => python(level),
        "c" => c(level),
        "cpp" => cpp(level),
        "javascript" => javascript(level),
        "typescript" => typescript(level),
        "go" => go(level),
        _ => &[],
    }
}

/// Every language id that ships a curated progression.
#[cfg(test)]
const CURATED_LANGUAGES: &[&str] = &[
    "rust",
    "python",
    "c",
    "cpp",
    "javascript",
    "typescript",
    "go",
];

const fn rust(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "vec", "main", "derive"],
        Level::Arguments => &[
            "fn:greet",
            "let:x",
            "letm:total",
            "fori:i,10",
            "struct:Point",
        ],
        Level::Nesting => &["fn:main>pr", "fori:i,10>pr", "if>ret", "fn:run>let:x"],
        Level::Siblings => &["pr+ret", "let:x+let:y", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "let:x*2", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["dstruct:Point", "denum:Color", "newtype:Meters", "testmod"],
        Level::DataDriven => &[
            "struct:P>field*[x:i32,y:f64]",
            "impl:Point>fn*[new,len,area]",
            "enum:Color>variant*[Red,Green,Blue]",
            "mod*[parser,lexer]>mtest*[empty,unicode]",
        ],
        Level::Members => &[
            "struct:Point>field*[x:f64,count:u32]",
            "struct:Point>field*[x:float,name:str]",
            "sqltable:Point>sqlcol*[x:f64,count:u32,label:String]",
            "impl:Api>fn:${value|snake}*[maxSize,HTTPError]",
        ],
        Level::Collections => &[
            "match:m>arm*@Msg",
            "match:m>armb*@Msg",
            "struct:Copy>field*@Point",
            "match:m>arm*@Msg?!name@Handled",
            "convert:Raw,Point",
            "errfrom:AppError",
            "tfn*@src.fns",
            "stub*@src.Shape.methods",
        ],
    }
}

const fn python(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "main"],
        Level::Arguments => &["def:greet", "class:Widget", "fori:i,10"],
        Level::Nesting => &["def:main>pr", "fori:i,10>pr", "if>ret", "while>pr"],
        Level::Siblings => &["pr+ret", "def:a+def:b"],
        Level::Repeat => &["pr*3", "def:main>pr*3"],
        Level::ClimbUp => &["def:main>fori:i,10>pr^ret", "def:run>if>pr^ret"],
        Level::Grouping => &["(def:main>pr)+def:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["classp:Point", "ctxmgr:Timer"],
        Level::DataDriven => &[
            "class:Shape>def*[area,perim]",
            "class:P>field*[x,y]",
            "match:cmd>arm*[Quit,Move]",
        ],
        Level::Members => &[
            "class:Point>field*[x:float,count:int]",
            "class:Point>field*[x:f64,name:String]",
            "sqltable:Point>sqlcol*[x:float,count:int]",
            "class:Api>def:${value|snake}*[maxSize,HTTPError]",
        ],
        Level::Collections => &[
            "match:cmd>arm*@Msg",
            "class:Copy>field*@Point",
            "ifchain:cmd*@Msg?!name@Handled",
            "tfn*@src.fns",
            "stub*@src.Shape.methods",
        ],
    }
}

const fn c(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "main"],
        Level::Arguments => &["fn:compute", "struct:Point", "fori:i,10"],
        Level::Nesting => &["fn:main>pr", "fori:i,10>pr", "if>ret", "while>pr"],
        Level::Siblings => &["pr+ret", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["hstruct:Node", "henum:Color"],
        Level::DataDriven => &[
            "struct:P>field*[x:int,y:int]",
            "enum:Color>variant*[Red,Green]",
            "switch:cmd>case*[A,B]",
        ],
        Level::Members => &[
            "struct:Point>field*[x:double,count:int]",
            "struct:Point>field*[x:f64,name:String]",
            "fn:${value|snake}*[maxSize,minSize]",
        ],
        Level::Collections => &[
            "switch:cmd>case*@Msg",
            "struct:Copy>field*@Point",
            "ifchain:cmd*@Msg?!value@Handled",
            "tfn*@src.fns",
        ],
    }
}

const fn cpp(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "main"],
        Level::Arguments => &["fn:compute", "class:Widget", "fori:i,10", "ns:app"],
        Level::Nesting => &[
            "fn:main>pr",
            "fori:i,10>pr",
            "if>ret",
            "class:Widget>fn:run",
        ],
        Level::Siblings => &["pr+ret", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["hclass:Widget", "nsclass:Shape", "henum:Mode"],
        Level::DataDriven => &[
            "struct:P>field*[x:int,y:int]",
            "enum:Mode>variant*[On,Off]",
            "switch:cmd>case*[A,B]",
        ],
        Level::Members => &[
            "struct:Point>field*[x:double,count:int]",
            "struct:Point>field*[x:f64,name:String]",
            "class:Api>fn:${value|snake}*[maxSize,HTTPError]",
        ],
        Level::Collections => &[
            "switch:cmd>case*@Msg",
            "struct:Copy>field*@Point",
            "ifchain:cmd*@Msg?!name@Handled",
            "stub*@src.Shape.methods",
        ],
    }
}

const fn javascript(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "arrow"],
        Level::Arguments => &["fn:greet", "class:Widget", "fori:i,10"],
        Level::Nesting => &[
            "fn:main>pr",
            "fori:i,10>pr",
            "if>ret",
            "class:Widget>fn:run",
        ],
        Level::Siblings => &["pr+ret", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["classc:Widget"],
        Level::DataDriven => &["switch:cmd>case*[A,B]", "class:S>fn*[area,perim]"],
        Level::Members => &[
            "class:Api>fn:${value|camel}*[max_size,http_error]",
            "class:Api>fn:${value|snake}*[maxSize,HTTPError]",
        ],
        Level::Collections => &[
            "switch:cmd>case*@Msg",
            "ifchain:cmd*@Msg?!value@Handled",
            "stub*@src.Shape.methods",
            "tfn*@src.fns",
        ],
    }
}

const fn typescript(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "arrow"],
        Level::Arguments => &["fn:greet", "class:Widget", "fori:i,10", "int:Shape"],
        Level::Nesting => &[
            "fn:main>pr",
            "fori:i,10>pr",
            "if>ret",
            "class:Widget>fn:run",
        ],
        Level::Siblings => &["pr+ret", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["classc:Shape"],
        Level::DataDriven => &[
            "int:Shape>field*[x:number,y:number]",
            "enum:Color>variant*[Red,Green]",
            "switch:cmd>case*[A,B]",
        ],
        Level::Members => &[
            "int:Point>field*[x:number,count:number]",
            "int:Point>field*[x:f64,name:String]",
            "sqltable:Point>sqlcol*[x:number,label:string]",
            "class:Api>fn:${value|camel}*[max_size,http_error]",
        ],
        Level::Collections => &[
            "switch:cmd>case*@Msg",
            "int:Copy>field*@Point",
            "ifchain:cmd*@Msg?!name@Handled",
            "stub*@src.Shape.methods",
            "tfn*@src.fns",
        ],
    }
}

const fn go(level: Level) -> &'static [&'static str] {
    match level {
        Level::SingleTrigger => &["pr", "ret", "todo", "main"],
        Level::Arguments => &["fn:run", "struct:Point", "fori:i,10", "iface:Shape"],
        Level::Nesting => &["fn:main>pr", "fori:i,10>pr", "if>ret", "for>pr"],
        Level::Siblings => &["pr+ret", "fn:a+fn:b"],
        Level::Repeat => &["pr*3", "fn:main>pr*3"],
        Level::ClimbUp => &["fn:main>fori:i,10>pr^ret", "fn:run>if>pr^ret"],
        Level::Grouping => &["(fn:main>pr)+fn:helper", "(fori:i,10>pr)+ret"],
        Level::Composite => &["stringer:Point", "errtype:ParseError", "maincmd"],
        Level::DataDriven => &["struct:P>field*[X:int,Y:int]", "switch:cmd>case*[A,B]"],
        Level::Members => &[
            "struct:Point>field*[X:float64,Count:int]",
            "struct:Point>field*[X:f64,Name:String]",
            "sqltable:Point>sqlcol*[X:float64,Count:int64]",
            "struct:Api>fn:${value|pascal}*[max_size,http_error]",
        ],
        Level::Collections => &[
            "switch:cmd>case*@Msg",
            "struct:Copy>field*@Point",
            "ifchain:cmd*@Msg?!value@Handled",
            "stub*@src.Shape.methods",
            "tfn*@src.fns",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codemmet_core::context::{ExpansionContext, IndentUnit};
    use codemmet_core::engine::Engine;
    use codemmet_core::lang::registry::Registry;
    use codemmet_core::snippet::render::TabstopMode;

    #[test]
    fn every_curated_abbreviation_renders() {
        let engine = Engine::with_registry(Registry::with_builtins().expect("builtins"));
        for language in CURATED_LANGUAGES {
            let mut ctx = ExpansionContext::plain(String::new(), IndentUnit::Spaces(4));
            ctx.collections = crate::workspace::collections(language);
            for level in Level::ALL {
                let set = curated(language, level);
                assert!(
                    !set.is_empty(),
                    "{language} has no curated set for {level:?}",
                );
                for abbr in set {
                    let body = engine
                        .expand(abbr, language, &ctx, TabstopMode::Strip)
                        .unwrap_or_else(|err| {
                            panic!("curated `{abbr}` ({language}, {level:?}) failed: {err}")
                        })
                        .body;
                    assert!(
                        !body.trim().is_empty(),
                        "curated `{abbr}` ({language}, {level:?}) rendered empty",
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_language_has_no_curated_set() {
        assert!(curated("cobol", Level::SingleTrigger).is_empty());
        assert!(curated("fortran", Level::Nesting).is_empty());
    }
}
