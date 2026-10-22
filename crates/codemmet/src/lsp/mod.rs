//! The LSP server: a synchronous, single-threaded loop over stdio.
//!
//! Architecture per `PLAN.md`: no async runtime — expansion is pure CPU
//! work, so one `for msg in &connection.receiver` loop suffices. Documents
//! use incremental sync; positions cross the wire in UTF-8 when the client
//! offers it (see [`line_index`]), UTF-16 otherwise. Syntax awareness comes
//! from a [`ContextProvider`]: tree-sitter-backed under the default
//! `tree-sitter` feature, the scope-blind null provider otherwise.

mod commands;
mod completion;
mod extract;
mod line_index;
mod requires;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use codemmet_core::Engine;
use codemmet_core::config::loader::{ConfigError, FileTarget};
use codemmet_core::config::paths::PathMapping;
use codemmet_core::context::{
    Collection, ContextProvider, DocSnapshot, ExpansionContext, Item, SourceContext, TESTED,
    mark_tested,
};
use codemmet_core::lang::profile;
use codemmet_core::lang::registry::{Registry, resolve_override_dirs};
use codemmet_core::lang::source::test_region;
use codemmet_core::snippet::render::TabstopMode;
#[cfg(feature = "tree-sitter")]
use codemmet_tsctx::TreeSitterProvider;
use lsp_server::{Connection, ErrorCode, Message, Request, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CompletionOptions, CompletionParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, ExecuteCommandOptions,
    ExecuteCommandParams, InitializeParams, PositionEncodingKind, Range, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri, WorkspaceEdit,
};
use serde::Deserialize;

use commands::CommandError;
use line_index::{LineIndex, PositionEncoding};

/// Arguments for `codemmet lsp`.
#[derive(Debug, clap::Args)]
pub struct LspArgs {
    /// Accepted for client compatibility; stdio is the only transport.
    #[arg(long)]
    stdio: bool,
}

/// When completion items are offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionMode {
    /// On every keystroke with a candidate (the default).
    Always,
    /// Only once the candidate contains a composition operator.
    OperatorsOnly,
    /// Never.
    Off,
}

/// `initializationOptions` the client may pass.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InitOptions {
    /// Snippet override directory (defaults to `<config>/codemmet/snippets`
    /// or the CLI's `--config`).
    snippets_dir: Option<PathBuf>,
    /// Personal path-prefix → snippet-dir mapping file (defaults to
    /// `<config>/codemmet/paths.toml` or the CLI's `--paths`).
    paths_file: Option<PathBuf>,
    /// `"always"` | `"operators-only"` | `"off"`.
    completion: Option<String>,
    /// Opt in to project-local packs (`.codemmet/snippets`, layered up the
    /// tree from the file being edited). Off by default: the editor may open
    /// untrusted repositories, and a project pack can silently redefine what
    /// abbreviations expand to.
    project_snippets: Option<bool>,
}

/// One open document, kept current via incremental sync.
#[derive(Debug)]
struct Document {
    /// Full document text.
    text: String,
    /// The client's `languageId` for the document.
    language_id: String,
    /// Version of the last applied change.
    version: i32,
}

/// Resolves (and caches) an [`Engine`] per document, so path-scoped snippet
/// tiers apply to the file being edited.
///
/// For each document the override stack is `user global < user path-mapped <
/// layered project packs` (see
/// [`resolve_override_dirs`](codemmet_core::lang::registry::resolve_override_dirs)).
/// Engines are cached by their exact resolved directory list — the caching
/// consequence of per-document resolution — so repeated requests against the
/// same subtree are free. [`reload`](SnippetResolver::reload) clears the cache;
/// this is a **read-only** operation (it re-reads packs from disk on the next
/// request, it never writes), which is how the editor picks up a snippet that
/// `codemmet capture` just wrote.
struct SnippetResolver {
    /// The user global override directory (init `snippets_dir` or `--config`).
    user_dir: Option<PathBuf>,
    /// Personal path-prefix → dir mapping.
    mapping: PathMapping,
    /// Whether to layer in `.codemmet/snippets` project packs (opt-in).
    include_project: bool,
    /// Engines keyed by their resolved override-dir list.
    cache: HashMap<Vec<PathBuf>, Rc<Engine>>,
    /// Builtins-only fallback, used when an override tier fails to load.
    builtins: Rc<Engine>,
}

impl SnippetResolver {
    /// Build a resolver, precompiling the builtins-only fallback engine.
    fn new(
        user_dir: Option<PathBuf>,
        mapping: PathMapping,
        include_project: bool,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            user_dir,
            mapping,
            include_project,
            cache: HashMap::new(),
            builtins: Rc::new(Engine::with_registry(Registry::with_builtins()?)),
        })
    }

    /// The engine for a document at `doc_path` (its resolved snippet tiers),
    /// building and caching it on first use. A broken override tier falls back
    /// to builtins so the server stays usable.
    fn engine_for(&mut self, doc_path: Option<&Path>) -> Rc<Engine> {
        let dirs = match doc_path {
            Some(path) => resolve_override_dirs(
                path,
                self.user_dir.as_deref(),
                &self.mapping,
                self.include_project,
            ),
            // A non-file document (untitled buffer) gets the user global tier
            // only — there is no path to map or walk.
            None => self.user_dir.iter().cloned().collect(),
        };
        if let Some(engine) = self.cache.get(&dirs) {
            return Rc::clone(engine);
        }
        let refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
        match Registry::with_dirs_layered(&refs) {
            Ok(registry) => {
                let engine = Rc::new(Engine::with_registry(registry));
                self.cache.insert(dirs, Rc::clone(&engine));
                engine
            }
            Err(err) => {
                eprintln!("codemmet: ignoring snippet overrides: {err}");
                Rc::clone(&self.builtins)
            }
        }
    }

    /// Drop every cached engine so the next request re-reads packs from disk.
    fn reload(&mut self) {
        self.cache.clear();
    }
}

/// All server state.
struct Server {
    /// Open documents by URI.
    docs: HashMap<Uri, Document>,
    /// Completion gating from `initializationOptions`.
    completion_mode: CompletionMode,
    /// The negotiated position encoding.
    encoding: PositionEncoding,
    /// Syntax awareness for expansion points (`context_at` needs `&mut`
    /// for its tree cache, so the server owns the provider mutably).
    provider: Box<dyn ContextProvider>,
    /// Per-document engine resolution + cache.
    resolver: SnippetResolver,
    /// Explicit source files per document (`codemmet.setSource`): the file
    /// whose definitions feed `src.*` collections and `${src.*}` variables
    /// when expanding in that document (a test file).
    sources: HashMap<Uri, SourceSlot>,
}

/// A document's explicit source file and the context last read from it.
struct SourceSlot {
    path: PathBuf,
    /// The context extracted from the file as it was at `(modified, len)`;
    /// reused while the file's metadata is unchanged, so a request costs a
    /// `stat`, not a read and a parse.
    cached: Option<(Option<std::time::SystemTime>, u64, Rc<SourceContext>)>,
}

impl Server {
    /// The explicit source context for the document at `uri`, if one was
    /// set and still reads. Edits to the source file show up on the next
    /// request (its mtime/size changed); an unchanged file is served from
    /// the slot's cache.
    fn source_context(&mut self, uri: &Uri) -> Option<Rc<SourceContext>> {
        let slot = self.sources.get(uri)?;
        let path = slot.path.clone();
        let meta = std::fs::metadata(&path).ok()?;
        let stamp = (meta.modified().ok(), meta.len());
        if let Some((modified, len, cached)) = &slot.cached
            && (*modified, *len) == stamp
        {
            return Some(Rc::clone(cached));
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let language = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(profile::find)
            .or_else(|| {
                self.docs
                    .get(uri)
                    .and_then(|doc| profile::find(&doc.language_id))
            })
            .map(|p| p.id)?;
        let doc = DocSnapshot {
            language: language.to_owned(),
            text,
        };
        let context = Rc::new(SourceContext {
            path: path.display().to_string(),
            collections: self.provider.source_collections(&doc),
            vars: codemmet_core::lang::source::variables(language, &path, &doc.text),
        });
        if let Some(slot) = self.sources.get_mut(uri) {
            slot.cached = Some((stamp.0, stamp.1, Rc::clone(&context)));
        }
        Some(context)
    }

    /// Finish `ctx` for the document at `uri`: apply its source (if any) and
    /// mark the `src.*` items its tests already cover ([`mark_tested`]), so
    /// `?!tested` repeats and the `tested` member work. With an explicit
    /// source in another file, the whole document is the tests; a document
    /// that is its own source — explicitly, or with no source set — counts
    /// only its test region ([`test_region`]).
    fn apply_source(&mut self, uri: &Uri, doc: &DocSnapshot, ctx: &mut ExpansionContext) {
        let source = self.source_context(uri);
        let same_file = source.as_ref().is_none_or(|source| {
            file_uri_to_path(uri.as_str()).as_deref() == Some(Path::new(&source.path))
        });
        if let Some(source) = &source {
            source.apply(ctx);
        }
        mark_tested(&mut ctx.collections, tests_in(doc, same_file));
    }

    /// The engine resolved for the document at `uri` (path-scoped tiers).
    fn engine_for(&mut self, uri: &Uri) -> Rc<Engine> {
        let path = file_uri_to_path(uri.as_str());
        self.resolver.engine_for(path.as_deref())
    }
}

/// Convert a `file:` URI string to a filesystem path.
///
/// Handles the common `file:///path` (empty authority) and `file://host/path`
/// forms and percent-decodes the path. Returns `None` for non-`file` URIs.
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Everything from the first '/' is the path; anything before it is an
    // authority (host) we don't need for a local path.
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(start) => rest.get(start..)?,
        None => return None,
    };
    Some(PathBuf::from(percent_decode(path)))
}

/// Percent-decode a URI path component (`%20` → space, etc.).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        if byte == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex_nibble),
                bytes.get(i + 2).copied().and_then(hex_nibble),
            )
        {
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(byte);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A hex digit's value, or `None` if `byte` is not `[0-9a-fA-F]`.
const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// The best context provider this build carries: tree-sitter under the
/// default `tree-sitter` feature, the scope-blind null provider otherwise.
fn default_provider() -> Box<dyn ContextProvider> {
    #[cfg(feature = "tree-sitter")]
    {
        Box::new(TreeSitterProvider::new())
    }
    #[cfg(not(feature = "tree-sitter"))]
    {
        Box::new(codemmet_core::context::NullContextProvider)
    }
}

/// Run `codemmet lsp` over stdio until the client disconnects or exits.
///
/// `default_user_dir` / `default_paths_file` are the CLI-resolved defaults; the
/// client's `initializationOptions` (`snippets_dir`, `paths_file`) override
/// them.
pub fn run(
    default_user_dir: Option<&Path>,
    default_paths_file: Option<&Path>,
    _args: &LspArgs,
) -> ExitCode {
    match serve(default_user_dir, default_paths_file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// The full server lifecycle over stdio: handshake, main loop, teardown.
fn serve(default_user_dir: Option<&Path>, default_paths_file: Option<&Path>) -> anyhow::Result<()> {
    let (connection, io_threads) = Connection::stdio();
    serve_connection(connection, default_user_dir, default_paths_file)?;
    io_threads.join()?;
    Ok(())
}

/// The server lifecycle over any [`Connection`] (stdio in production,
/// [`Connection::memory`] in tests): handshake, main loop, teardown.
fn serve_connection(
    connection: Connection,
    default_user_dir: Option<&Path>,
    default_paths_file: Option<&Path>,
) -> anyhow::Result<()> {
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(initialize_params)?;
    let options: InitOptions = params
        .initialization_options
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let encoding = negotiate_encoding(&params.capabilities);

    connection.initialize_finish(
        initialize_id,
        serde_json::json!({
            "capabilities": capabilities(encoding),
            "serverInfo": { "name": "codemmet", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;

    let user_dir = options.snippets_dir.as_deref().or(default_user_dir);
    let paths_file = options.paths_file.as_deref().or(default_paths_file);
    // The personal path mapping is user config, so it is always on; a broken
    // one is reported but non-fatal.
    let mapping = paths_file.map_or_else(PathMapping::empty, |path| {
        PathMapping::load(path).unwrap_or_else(|err| {
            eprintln!("codemmet: ignoring path mapping: {err}");
            PathMapping::empty()
        })
    });
    // Project packs are opt-in per document: only when `project_snippets` is
    // set does the resolver layer in `.codemmet/snippets` from the file's tree.
    // (The editor may open untrusted repositories.)
    let include_project = options.project_snippets.unwrap_or(false);
    let resolver = SnippetResolver::new(user_dir.map(Path::to_path_buf), mapping, include_project)?;

    let mut server = Server {
        docs: HashMap::new(),
        completion_mode: options.completion_mode(),
        encoding,
        provider: default_provider(),
        sources: HashMap::new(),
        resolver,
    };

    for msg in &connection.receiver {
        match msg {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                let response = server.handle_request(request);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                server.handle_notification(&notification.method, notification.params);
            }
            Message::Response(_) => {}
        }
    }

    drop(connection);
    Ok(())
}

impl InitOptions {
    /// The completion gate the options select; unknown values mean "always".
    fn completion_mode(&self) -> CompletionMode {
        match self.completion.as_deref() {
            Some("off") => CompletionMode::Off,
            Some("operators-only") => CompletionMode::OperatorsOnly,
            _ => CompletionMode::Always,
        }
    }
}

/// UTF-8 when the client advertises support for it, UTF-16 otherwise.
fn negotiate_encoding(capabilities: &lsp_types::ClientCapabilities) -> PositionEncoding {
    let offers_utf8 = capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
        .is_some_and(|encodings| encodings.contains(&PositionEncodingKind::UTF8));
    if offers_utf8 {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    }
}

/// The server capabilities advertised at initialize time.
fn capabilities(encoding: PositionEncoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(match encoding {
            PositionEncoding::Utf8 => PositionEncodingKind::UTF8,
            PositionEncoding::Utf16 => PositionEncodingKind::UTF16,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(
                [">", "+", ":", ","]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            ),
            ..CompletionOptions::default()
        }),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                "codemmet.expand".to_owned(),
                "codemmet.wrap".to_owned(),
                "codemmet.preview".to_owned(),
                "codemmet.list".to_owned(),
                "codemmet.context".to_owned(),
                "codemmet.reloadSnippets".to_owned(),
                "codemmet.setSource".to_owned(),
            ],
            ..ExecuteCommandOptions::default()
        }),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}

impl Server {
    /// Dispatch one client request to its handler.
    fn handle_request(&mut self, request: Request) -> Response {
        match request.method.as_str() {
            "textDocument/completion" => self.handle_completion(request),
            "textDocument/codeAction" => self.handle_code_action(request),
            "workspace/executeCommand" => self.handle_execute_command(request),
            _ => Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unhandled method `{}`", request.method),
            ),
        }
    }

    /// `textDocument/completion`: items for the abbreviation at the cursor.
    fn handle_completion(&mut self, request: Request) -> Response {
        let params: CompletionParams = match serde_json::from_value(request.params) {
            Ok(params) => params,
            Err(err) => return invalid_params(request.id, &err),
        };
        let position = params.text_document_position;
        let uri = &position.text_document.uri;
        let path = file_uri_to_path(uri.as_str());
        let source = self.source_context(uri);
        // No source, or one that is this very file: its own test region counts.
        let same_file = source
            .as_ref()
            .is_none_or(|source| path.as_deref() == Some(Path::new(&source.path)));
        let Some(doc) = self.docs.get(uri) else {
            // An unopened document is not an error; there is just nothing
            // to complete.
            return Response::new_ok(request.id, serde_json::Value::Null);
        };
        let index = LineIndex::new(&doc.text);
        let offset = index.offset(&doc.text, position.position, self.encoding);
        // Resolve the engine for this file's snippet tiers (disjoint field
        // borrow, so `doc` and `provider` can be borrowed alongside it).
        let engine = self.resolver.engine_for(path.as_deref());
        let items = completion::compute(
            &engine,
            self.provider.as_mut(),
            &index,
            &doc.text,
            &doc.language_id,
            offset,
            self.completion_mode,
            self.encoding,
            source.as_deref(),
            same_file,
        );
        match serde_json::to_value(items) {
            Ok(value) => Response::new_ok(request.id, value),
            Err(err) => internal_error(request.id, &err),
        }
    }

    /// `textDocument/codeAction`: code-generation edits at the cursor — today,
    /// filling a `match` from an in-scope enum. Each is a plain-text
    /// `WorkspaceEdit`, so any client applies it without the codemmet plugin.
    fn handle_code_action(&mut self, request: Request) -> Response {
        let params: CodeActionParams = match serde_json::from_value(request.params) {
            Ok(params) => params,
            Err(err) => return invalid_params(request.id, &err),
        };
        let uri = params.text_document.uri;
        let Ok(doc) = self.snapshot(&uri) else {
            return Response::new_ok(request.id, serde_json::Value::Array(Vec::new()));
        };
        let index = LineIndex::new(&doc.text);
        // Offer actions for the cursor (the selection start).
        let offset = index.offset(&doc.text, params.range.start, self.encoding);
        let mut actions: Vec<CodeActionOrCommand> = self
            .provider
            .fill_targets(&doc, offset)
            .into_iter()
            .map(|target| {
                let range = Range {
                    start: index.position(&doc.text, target.range.start, self.encoding),
                    end: index.position(&doc.text, target.range.end, self.encoding),
                };
                text_edit_action(&uri, target.title, range, target.new_text)
            })
            .collect();
        actions.extend(self.missing_test_actions(&uri, &doc, &index));
        match serde_json::to_value(actions) {
            Ok(value) => Response::new_ok(request.id, value),
            Err(err) => internal_error(request.id, &err),
        }
    }

    /// "Add missing tests" actions.
    ///
    /// With an explicit source (`codemmet.setSource`) the document is a test
    /// file: one action per builtin test trigger — `tfn` over the source's
    /// `src.fns`, `tmeth` over its `src.methods` — for the items that pass the
    /// trigger's own default filter and whose name the document never
    /// mentions. The edit appends the rendered tests at the end of the
    /// document as plain text, so any LSP client can apply it.
    ///
    /// Without one, the document is its own source when its language keeps
    /// tests in the source file (the builtin `tfile` targets `source`, as in
    /// Rust) or it already holds tests (Python `def test_…` / `class Test…`).
    /// Then its own definitions feed the collections, only the test region
    /// ([`test_region`]) counts as mentioning a name, and a single action adds
    /// the tests inside the existing test module when its closing brace ends
    /// the file, or after the code (in a fresh `tmod` module when the language
    /// has one).
    fn missing_test_actions(
        &mut self,
        uri: &Uri,
        doc: &DocSnapshot,
        index: &LineIndex,
    ) -> Vec<CodeActionOrCommand> {
        let engine = self.engine_for(uri);
        let Some(lang) = engine.registry().language(&doc.language) else {
            return Vec::new();
        };
        let doc_path = file_uri_to_path(uri.as_str());
        let explicit = self.source_context(uri);
        let own_collections;
        let (collections, vars, path, same_file) = if let Some(source) = &explicit {
            let path = PathBuf::from(&source.path);
            let same = doc_path.as_deref() == Some(path.as_path());
            (&source.collections, source.vars.clone(), path, same)
        } else {
            // Its own test file when the language keeps tests beside the
            // code (Rust) or the file already holds tests (Python's
            // `def test_…` / `class Test…`).
            if !tests_live_in_source(lang) && test_region(lang.profile.id, &doc.text).is_none() {
                return Vec::new();
            }
            let Some(path) = doc_path else {
                return Vec::new();
            };
            own_collections = self.provider.source_collections(doc);
            let vars = codemmet_core::lang::source::variables(lang.profile.id, &path, &doc.text);
            (&own_collections, vars, path, true)
        };
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |f| f.to_string_lossy().into_owned(),
        );
        // In a source file, a name mentioned by the code itself is not tested;
        // only the test region counts.
        let region = same_file
            .then(|| test_region(lang.profile.id, &doc.text))
            .flatten();
        let mut collections = collections.clone();
        mark_tested(&mut collections, tests_in(doc, same_file));
        let parts = missing_parts(lang, &collections);
        if parts.is_empty() {
            return Vec::new();
        }

        let mut ctx = ExpansionContext::plain(String::new(), lang.profile.default_indent.clone());
        ctx.vars = vars;
        // `?all`: the default filter was applied above; each collection is
        // exactly the set to render.
        let expand = |abbr: &str, ctx: &ExpansionContext| {
            engine
                .expand(abbr, &doc.language, ctx, TabstopMode::Strip)
                .ok()
                .map(|e| e.body.trim_end_matches('\n').to_owned())
        };
        let end = index.position(&doc.text, doc.text.len(), self.encoding);
        let mut actions = Vec::new();
        if same_file {
            let count: usize = parts.iter().map(|(_, _, items)| items.len()).sum();
            let abbr = parts
                .into_iter()
                .map(|(trigger, _, items)| {
                    let name = format!("missing_{trigger}");
                    let abbr = format!("{trigger}*@{name}?all");
                    ctx.collections.insert(name, Collection { items });
                    abbr
                })
                .collect::<Vec<_>>()
                .join("+");
            let plural = if count == 1 { "" } else { "s" };
            let title = format!("Add {count} missing test{plural} to {file}");
            let closing = region.and_then(|_| closing_brace_line(&doc.text));
            let indent = lang.profile.default_indent.as_str();
            let wrap = lang.set.get("tmod").is_some();
            let edit = same_file_edit(&doc.text, closing, &indent, &abbr, wrap, |abbr| {
                expand(abbr, &ctx)
            });
            if let Some((at, new_text)) = edit {
                let at = index.position(&doc.text, at, self.encoding);
                actions.push(text_edit_action(
                    uri,
                    title,
                    Range { start: at, end: at },
                    new_text,
                ));
            }
            return actions;
        }
        for (trigger, noun, missing) in parts {
            let count = missing.len();
            ctx.collections
                .insert("missing".to_owned(), Collection { items: missing });
            let Some(body) = expand(&format!("{trigger}*@missing?all"), &ctx) else {
                continue;
            };
            let lead = if doc.text.is_empty() || doc.text.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let plural = if count == 1 { "" } else { "s" };
            let title = format!("Add {count} missing {noun} test{plural} from {file}");
            actions.push(text_edit_action(
                uri,
                title,
                Range { start: end, end },
                format!("{lead}{body}\n"),
            ));
        }
        actions
    }

    /// `workspace/executeCommand`: `codemmet.expand` / `codemmet.wrap`.
    fn handle_execute_command(&mut self, request: Request) -> Response {
        let params: ExecuteCommandParams = match serde_json::from_value(request.params) {
            Ok(params) => params,
            Err(err) => return invalid_params(request.id, &err),
        };
        let result = match params.command.as_str() {
            "codemmet.expand" => {
                command_args(params.arguments).and_then(|args| commands::expand(self, &args))
            }
            "codemmet.wrap" => {
                command_args(params.arguments).and_then(|args| commands::wrap(self, &args))
            }
            "codemmet.preview" => {
                command_args(params.arguments).map(|args| commands::preview(self, &args))
            }
            // Read-only: the snippet browser (every trigger for the document,
            // annotated with the context at the cursor).
            "codemmet.list" => {
                command_args(params.arguments).and_then(|args| commands::list(self, &args))
            }
            // Read-only: the context viewer (scope stack + collections).
            "codemmet.context" => {
                command_args(params.arguments).and_then(|args| commands::context(self, &args))
            }
            // Read-only: drop the per-document engine cache so the next request
            // re-reads packs from disk. The editor calls this after writing a
            // pack via the `codemmet capture` CLI, to pick the new trigger up
            // in-session. The server itself never writes.
            "codemmet.reloadSnippets" => {
                self.resolver.reload();
                Ok(serde_json::json!({ "reloaded": true }))
            }
            // Set (or clear, with a null `source`) the explicit source file
            // for a document; see `commands::set_source`.
            "codemmet.setSource" => {
                command_args(params.arguments).and_then(|args| commands::set_source(self, &args))
            }
            other => Err(CommandError(format!("unknown command `{other}`"))),
        };
        match result {
            Ok(value) => Response::new_ok(request.id, value),
            Err(CommandError(message)) => {
                Response::new_err(request.id, ErrorCode::RequestFailed as i32, message)
            }
        }
    }

    /// Apply one client notification to the document store.
    fn handle_notification(&mut self, method: &str, params: serde_json::Value) {
        match method {
            "textDocument/didOpen" => {
                let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(params) else {
                    return;
                };
                let doc = params.text_document;
                self.docs.insert(
                    doc.uri,
                    Document {
                        text: doc.text,
                        language_id: doc.language_id,
                        version: doc.version,
                    },
                );
            }
            "textDocument/didChange" => {
                let Ok(params) = serde_json::from_value::<DidChangeTextDocumentParams>(params)
                else {
                    return;
                };
                let Some(doc) = self.docs.get_mut(&params.text_document.uri) else {
                    return;
                };
                // Stale (out-of-order) versions are dropped.
                if params.text_document.version < doc.version {
                    return;
                }
                for change in params.content_changes {
                    if let Some(range) = change.range {
                        // Each range refers to the document as already
                        // modified by the preceding changes in this batch,
                        // so re-index before every splice.
                        let index = LineIndex::new(&doc.text);
                        let start = index.offset(&doc.text, range.start, self.encoding);
                        let end = index.offset(&doc.text, range.end, self.encoding).max(start);
                        doc.text.replace_range(start..end, &change.text);
                    } else {
                        // A rangeless change replaces the whole document.
                        doc.text = change.text;
                    }
                }
                doc.version = params.text_document.version;
            }
            "textDocument/didClose" => {
                if let Ok(params) = serde_json::from_value::<DidCloseTextDocumentParams>(params) {
                    self.docs.remove(&params.text_document.uri);
                    self.sources.remove(&params.text_document.uri);
                }
            }
            _ => {}
        }
    }

    /// The open document at `uri`, or a command error naming it.
    fn document(&self, uri: &Uri) -> Result<&Document, CommandError> {
        self.docs
            .get(uri)
            .ok_or_else(|| CommandError(format!("document not open: {}", uri.as_str())))
    }

    /// An owned snapshot of the document at `uri` for the context provider,
    /// carrying the canonical profile id (not the raw client `languageId`).
    ///
    /// The profile mapping is static (`profile::find`), so this does not need a
    /// resolved registry — decoupling it from the per-document engine.
    fn snapshot(&self, uri: &Uri) -> Result<DocSnapshot, CommandError> {
        let doc = self.document(uri)?;
        let profile = profile::find(&doc.language_id)
            .ok_or_else(|| CommandError(format!("unsupported language `{}`", doc.language_id)))?;
        Ok(DocSnapshot {
            language: profile.id.to_owned(),
            text: doc.text.clone(),
        })
    }
}

/// A code action carrying one plain-text edit to `uri`.
fn text_edit_action(
    uri: &Uri,
    title: String,
    range: Range,
    new_text: String,
) -> CodeActionOrCommand {
    let edit = WorkspaceEdit {
        changes: Some(HashMap::from([(
            uri.clone(),
            vec![TextEdit { range, new_text }],
        )])),
        ..WorkspaceEdit::default()
    };
    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        edit: Some(edit),
        ..CodeAction::default()
    })
}

/// The text of `doc` that counts as tests for marking `src.*` items: all of
/// it for a test file, only the test region for a document that is its own
/// source (none when its language has no in-file test convention).
fn tests_in(doc: &DocSnapshot, same_file: bool) -> &str {
    if same_file {
        let language = profile::find(&doc.language).map_or(doc.language.as_str(), |p| p.id);
        test_region(language, &doc.text).unwrap_or("")
    } else {
        &doc.text
    }
}

/// Whether the language's builtin `tfile` scaffold writes into the source
/// file itself (`target = "source"`), i.e. tests conventionally live beside
/// the code they test.
fn tests_live_in_source(lang: &codemmet_core::lang::registry::LanguageSnippets) -> bool {
    lang.set.get("tfile").is_some_and(|snippet| {
        snippet.multifile.as_ref().is_some_and(|multi| {
            multi
                .files
                .iter()
                .any(|piece| matches!(piece.target, FileTarget::Source))
        })
    })
}

/// The builtin test triggers that still have untested items in `collections`
/// (already passed through [`mark_tested`]): `(trigger, noun, items)` for
/// `tfn` over `src.fns` and `tmeth` over `src.methods`, keeping the items
/// that pass the trigger's own default filter and carry no `tested` mark.
fn missing_parts<'a>(
    lang: &'a codemmet_core::lang::registry::LanguageSnippets,
    collections: &BTreeMap<String, Collection>,
) -> Vec<(&'a str, &'a str, Vec<Item>)> {
    let mut parts = Vec::new();
    for (trigger, key, noun) in [
        ("tfn", "src.fns", "function"),
        ("tmeth", "src.methods", "method"),
    ] {
        let (Some(snippet), Some(collection)) = (lang.set.get(trigger), collections.get(key))
        else {
            continue;
        };
        let missing: Vec<Item> = collection
            .items
            .iter()
            .filter(|item| snippet.filter.iter().all(|p| p.matches(item)))
            .filter(|item| !item.members.contains_key(TESTED))
            .cloned()
            .collect();
        if !missing.is_empty() {
            parts.push((trigger, noun, missing));
        }
    }
    parts
}

/// The edit that adds tests to a source file: `(byte offset, text)`. When
/// `closing` is the line of the brace that ends a trailing test module, the
/// rendered `abbr` goes inside it, one `indent` level deeper; otherwise the
/// tests go after the code, inside a fresh `tmod` module when the language
/// has one (`wrap`) and bare otherwise (Python).
fn same_file_edit(
    text: &str,
    closing: Option<usize>,
    indent: &str,
    abbr: &str,
    wrap: bool,
    expand: impl Fn(&str) -> Option<String>,
) -> Option<(usize, String)> {
    if let Some(at) = closing {
        let indented = expand(abbr)?
            .lines()
            .map(|l| {
                if l.is_empty() {
                    String::new()
                } else {
                    format!("{indent}{l}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Some((at, format!("\n{indented}\n")));
    }
    let body = if wrap {
        expand(&format!("tmod>{abbr}"))?
    } else {
        expand(abbr)?
    };
    let lead = if text.is_empty() {
        ""
    } else if text.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    Some((text.len(), format!("{lead}{body}\n")))
}

/// The byte offset of the last line of `text` when that line is a lone `}`
/// (the closing brace of a trailing block, e.g. a `mod tests`): where new
/// members of that block are inserted.
fn closing_brace_line(text: &str) -> Option<usize> {
    let trimmed = text.trim_end();
    let line_start = trimmed.rfind('\n').map_or(0, |i| i + 1);
    (trimmed[line_start..].trim() == "}").then_some(line_start)
}

/// Deserialize the single object argument LSP commands carry.
fn command_args<T: serde::de::DeserializeOwned>(
    arguments: Vec<serde_json::Value>,
) -> Result<T, CommandError> {
    let mut arguments = arguments;
    if arguments.len() != 1 {
        return Err(CommandError(format!(
            "expected exactly one argument object, got {}",
            arguments.len()
        )));
    }
    let argument = arguments.remove(0);
    serde_json::from_value(argument)
        .map_err(|err| CommandError(format!("invalid command arguments: {err}")))
}

/// An `InvalidParams` error response.
fn invalid_params(id: lsp_server::RequestId, err: &dyn std::fmt::Display) -> Response {
    Response::new_err(id, ErrorCode::InvalidParams as i32, err.to_string())
}

/// An `InternalError` error response.
fn internal_error(id: lsp_server::RequestId, err: &dyn std::fmt::Display) -> Response {
    Response::new_err(id, ErrorCode::InternalError as i32, err.to_string())
}

// In-process integration tests: the real server lifecycle over
// `Connection::memory()`, no child process. The import-insertion test needs
// the tree-sitter provider, so the module rides the same feature.
#[cfg(all(test, feature = "tree-sitter"))]
mod tests {
    use std::time::Duration;

    use lsp_server::Notification;
    use serde_json::{Value, json};

    use super::*;

    /// A client half driving an in-process server thread.
    struct MemoryClient {
        connection: Connection,
        server: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
        next_id: i32,
    }

    impl MemoryClient {
        /// Spawn the server over a memory connection and run the handshake,
        /// negotiating UTF-8 positions and pointing `snippets_dir` at
        /// `pack_dir`.
        fn start(pack_dir: &Path) -> Self {
            let (server_side, client_side) = Connection::memory();
            let dir = pack_dir.to_path_buf();
            let server =
                std::thread::spawn(move || serve_connection(server_side, Some(&dir), None));
            let mut client = Self {
                connection: client_side,
                server: Some(server),
                next_id: 0,
            };
            let result = client.request(
                "initialize",
                json!({
                    "capabilities": { "general": { "positionEncodings": ["utf-8"] } },
                }),
            );
            assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");
            client.notify("initialized", json!({}));
            client
        }

        /// Send one request and return its result (panics on an error
        /// response or a ten-second stall).
        fn request(&mut self, method: &str, params: Value) -> Value {
            self.next_id += 1;
            let id = lsp_server::RequestId::from(self.next_id);
            self.connection
                .sender
                .send(Message::Request(Request::new(
                    id.clone(),
                    method.to_owned(),
                    params,
                )))
                .expect("send should succeed");
            loop {
                let message = self
                    .connection
                    .receiver
                    .recv_timeout(Duration::from_secs(10))
                    .expect("server should respond");
                if let Message::Response(response) = message
                    && response.id == id
                {
                    assert!(
                        response.error.is_none(),
                        "unexpected error: {:?}",
                        response.error
                    );
                    return response.result.unwrap_or(Value::Null);
                }
            }
        }

        fn notify(&self, method: &str, params: Value) {
            self.connection
                .sender
                .send(Message::Notification(Notification::new(
                    method.to_owned(),
                    params,
                )))
                .expect("send should succeed");
        }

        /// Open a Rust document at `uri` with `text`.
        fn open(&self, uri: &str, text: &str) {
            self.notify(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": "rust",
                        "version": 1,
                        "text": text,
                    }
                }),
            );
        }

        /// The completion items at `line`:`character` in `uri`.
        fn complete(&mut self, uri: &str, line: u32, character: u32) -> Value {
            self.request(
                "textDocument/completion",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
        }

        /// Clean shutdown: request + exit, then join the server thread.
        fn shutdown(mut self) {
            self.request("shutdown", Value::Null);
            self.notify("exit", Value::Null);
            self.server
                .take()
                .expect("server thread present")
                .join()
                .expect("server thread should not panic")
                .expect("server should exit cleanly");
        }
    }

    /// A scratch pack dir carrying a Rust trigger with `requires`.
    fn requires_pack(test: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("codemmet-lsp-memory")
            .join(format!("{test}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir should be creatable");
        std::fs::write(
            dir.join("rust.toml"),
            concat!(
                "[meta]\nlanguage = \"rust\"\n",
                "[snippets.zmap]\n",
                "requires = [\"std::collections::HashMap\"]\n",
                "body = \"let mut ${1:map} = HashMap::new();$0\"\n",
            ),
        )
        .expect("scratch pack should be writable");
        dir
    }

    /// The snippet-kind item labeled `label`, if any.
    fn snippet_item(items: &Value, label: &str) -> Option<Value> {
        items
            .as_array()
            .expect("completion result is an array")
            .iter()
            .find(|item| item["label"] == label && item["kind"] == 15)
            .cloned()
    }

    #[test]
    fn completion_attaches_missing_import_as_additional_text_edit() {
        let mut client = MemoryClient::start(&requires_pack("attach"));
        let uri = "file:///attach.rs";
        client.open(uri, "use std::fs;\n\nfn main() {\n    zmap\n}\n");

        let items = client.complete(uri, 3, 8);
        let item = snippet_item(&items, "zmap").expect("zmap snippet item expected");
        let edits = &item["additionalTextEdits"];
        assert_eq!(
            edits[0]["newText"], "use std::collections::HashMap;\n",
            "items: {items}"
        );
        // A pure insertion on the blank line after the last `use`.
        assert_eq!(
            edits[0]["range"]["start"],
            json!({ "line": 1, "character": 0 })
        );
        assert_eq!(edits[0]["range"]["start"], edits[0]["range"]["end"]);
        client.shutdown();
    }

    #[test]
    fn completion_omits_the_edit_when_the_import_exists() {
        let mut client = MemoryClient::start(&requires_pack("dedup"));
        let uri = "file:///dedup.rs";
        client.open(
            uri,
            "use std::collections::{HashMap, HashSet};\n\nfn main() {\n    zmap\n}\n",
        );

        let items = client.complete(uri, 3, 8);
        let item = snippet_item(&items, "zmap").expect("zmap snippet item expected");
        assert!(
            item.get("additionalTextEdits").is_none_or(Value::is_null),
            "no import edit expected: {item}"
        );
        client.shutdown();
    }

    #[test]
    fn second_expansion_sees_the_first_ones_import() {
        // Simulates accepting the first completion: the client applies both
        // edits and sends didChange; the next request must not re-insert.
        let mut client = MemoryClient::start(&requires_pack("twice"));
        let uri = "file:///twice.rs";
        client.open(uri, "use std::fs;\n\nfn main() {\n    zmap\n}\n");
        let items = client.complete(uri, 3, 8);
        assert!(snippet_item(&items, "zmap").expect("item")["additionalTextEdits"].is_array());

        // Whole-document change reflecting an accepted completion.
        client.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{
                    "text": "use std::fs;\nuse std::collections::HashMap;\n\nfn main() {\n    let mut map = HashMap::new();\n    zmap\n}\n"
                }],
            }),
        );
        let items = client.complete(uri, 5, 8);
        let item = snippet_item(&items, "zmap").expect("zmap snippet item expected");
        assert!(
            item.get("additionalTextEdits").is_none_or(Value::is_null),
            "dedup should catch the buffer's import: {item}"
        );
        client.shutdown();
    }

    #[test]
    fn expand_command_returns_extra_edits_for_missing_imports() {
        let mut client = MemoryClient::start(&requires_pack("command"));
        let uri = "file:///command.rs";
        client.open(uri, "use std::fs;\n\nfn main() {\n    zmap\n}\n");

        let result = client.request(
            "workspace/executeCommand",
            json!({
                "command": "codemmet.expand",
                "arguments": [{
                    "uri": uri,
                    "position": { "line": 3, "character": 8 },
                }],
            }),
        );
        assert_eq!(result["isSnippet"], true);
        assert!(
            result["snippet"]
                .as_str()
                .expect("snippet present")
                .contains("HashMap::new()"),
            "result: {result}"
        );
        let edits = &result["extraEdits"];
        assert_eq!(edits[0]["newText"], "use std::collections::HashMap;\n");
        assert_eq!(
            edits[0]["range"]["start"],
            json!({ "line": 1, "character": 0 })
        );

        // With the import already present the field is omitted entirely.
        let uri = "file:///command2.rs";
        client.open(
            uri,
            "use std::collections::HashMap;\n\nfn main() {\n    zmap\n}\n",
        );
        let result = client.request(
            "workspace/executeCommand",
            json!({
                "command": "codemmet.expand",
                "arguments": [{
                    "uri": uri,
                    "position": { "line": 3, "character": 8 },
                }],
            }),
        );
        assert!(result.get("extraEdits").is_none(), "result: {result}");
        client.shutdown();
    }
}
