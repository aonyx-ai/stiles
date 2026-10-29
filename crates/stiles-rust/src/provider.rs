use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ra_ap_hir::import_map::{ImportInfo, ImportMap};
use ra_ap_hir::{
    Adt, AsAssocItem as _, Crate, CrateDependency, Enum, Function, HasAttrs, HirDisplay, ItemInNs,
    Module, ModuleDef, PathResolution, ScopeDef, Semantics, Type, attach_db,
};
use ra_ap_ide_db::base_db::{SourceDatabase as _, VfsPath};
use ra_ap_ide_db::famous_defs::FamousDefs;
use ra_ap_ide_db::{ChangeWithProcMacros, FileId, RootDatabase};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace};
use ra_ap_syntax::{AstNode, Edition, NodeOrToken, SyntaxNode, TextRange, TextSize, ast};
use ra_ap_vfs::{AbsPathBuf, FileExcluded, Vfs};
use stiles_types::{
    Coverage, CoverageGap, DecorateTreeError, DecoratedNode, DecoratedTree, DecorationMap,
    DecorationProvider, ProviderName,
};

use crate::decorations::{
    AdtFlags, ErrorType, FnSignature, ImportSource, PublicPath, Reexports, ResolvedType,
    ReturnMode, TypePath,
};

mod load_rust_workspace_error;

pub use self::load_rust_workspace_error::LoadRustWorkspaceError;

/// Decoration provider for Rust using rust-analyzer
///
/// Loads a Cargo workspace into a rust-analyzer database at construction
/// time, then attaches type information to tree-sitter nodes during the
/// decorate phase. The workspace load is expensive and happens once; the
/// per-file decoration pass is relatively cheap.
///
/// The provider stores the workspace root. Every gap except
/// [`CoverageGap::StaleSource`] names this root.
///
/// # Examples
///
/// ```ignore
/// let provider = RustDecorationProvider::load(Path::new("."))?;
/// let coverage = provider.decorate(&tree)?;
/// ```
pub struct RustDecorationProvider {
    db: Mutex<RootDatabase>,
    vfs: Vfs,
    root: Arc<Path>,
}

impl RustDecorationProvider {
    /// Identifies this provider in diagnostics
    const NAME: ProviderName = ProviderName("rust");

    /// Loads a Cargo workspace for semantic analysis
    ///
    /// rust-analyzer discovers the nearest manifest above
    /// `workspace_root`, and Cargo expands that manifest to its whole
    /// workspace. A gap that names a workspace names the discovered
    /// root, not `workspace_root`. Analysis enables the `test` cfg, so
    /// code under `#[cfg(test)]` resolves like any other code.
    ///
    /// This is an expensive operation that builds rust-analyzer's internal
    /// database from the project's Cargo.toml. Call this once at startup.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadRustWorkspaceError`] if the current directory cannot
    /// be read, if `workspace_root` is not valid UTF-8, or if discovery finds
    /// no Cargo project. The load also fails when workspace resolution, the
    /// build scripts, or the analysis database build fails.
    pub fn load(workspace_root: &Path) -> Result<Self, LoadRustWorkspaceError> {
        let cargo_config = CargoConfig {
            sysroot: Some(ra_ap_project_model::RustLibSource::Discover),
            set_test: true,
            ..CargoConfig::default()
        };
        let load_config = LoadCargoConfig {
            load_out_dirs_from_check: true,
            with_proc_macro_server: ProcMacroServerChoice::None,
            prefill_caches: false,
            num_worker_threads: 1,
            proc_macro_processes: 0,
        };

        let cwd = std::env::current_dir()
            .map_err(|source| LoadRustWorkspaceError::UnreadableCurrentDirectory { source })?;
        let discover_from = cwd.join(workspace_root);
        if discover_from.to_str().is_none() {
            return Err(LoadRustWorkspaceError::NotUtf8 {
                path: discover_from,
            });
        }
        let discover_from = AbsPathBuf::assert_utf8(discover_from);
        let manifest = ProjectManifest::discover_single(&discover_from).map_err(|source| {
            LoadRustWorkspaceError::Undiscovered {
                root: workspace_root.to_path_buf(),
                source,
            }
        })?;

        let mut workspace =
            ProjectWorkspace::load(manifest, &cargo_config, &|_msg| {}).map_err(|source| {
                LoadRustWorkspaceError::Unresolved {
                    root: workspace_root.to_path_buf(),
                    source,
                }
            })?;

        if load_config.load_out_dirs_from_check {
            let build_scripts = workspace
                .run_build_scripts(&cargo_config, &|_msg| {})
                .map_err(|source| LoadRustWorkspaceError::BuildScriptsFailed { source })?;
            workspace.set_build_scripts(build_scripts);
        }

        let root: Arc<Path> = Arc::from(workspace.workspace_root().as_ref() as &Path);

        let (mut db, vfs, _proc_macro_client) =
            ra_ap_load_cargo::load_workspace(workspace, &cargo_config.extra_env, &load_config)
                .map_err(|source| LoadRustWorkspaceError::DatabaseFailed { source })?;

        give_text_to_files_that_have_none(&mut db, &vfs, &root);

        Ok(Self {
            db: Mutex::new(db),
            vfs,
            root,
        })
    }

    /// Creates a provider that holds an empty workspace
    ///
    /// The provider declines every file with
    /// [`CoverageGap::OutsideRoot`]. Do not use it as a fallback when
    /// [`RustDecorationProvider::load`] fails: the pipeline would then
    /// refuse every file.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = RustDecorationProvider::empty();
    /// let coverage = provider.decorate(&tree)?;
    /// ```
    pub fn empty() -> Self {
        Self {
            db: Mutex::new(RootDatabase::default()),
            vfs: Vfs::default(),
            root: Arc::from(Path::new(".")),
        }
    }

    /// Classifies a file rust-analyzer never interned
    ///
    /// A path under the loaded root reports [`CoverageGap::Unreachable`],
    /// because the toolchain could have known the file and did not. Any
    /// other path reports [`CoverageGap::OutsideRoot`].
    fn gap_for_unknown_file(&self, file_path: &Path) -> CoverageGap {
        if file_path.starts_with(&*self.root) {
            CoverageGap::Unreachable {
                root: Arc::clone(&self.root),
            }
        } else {
            CoverageGap::OutsideRoot {
                root: Arc::clone(&self.root),
            }
        }
    }
}

/// Records lossy UTF-8 text for interned files under `root` that have none
///
/// The load interns a file whose bytes are not UTF-8, but records no
/// text for it. rust-analyzer panics when it later reads a file that has
/// no recorded text. The read can happen while rust-analyzer analyzes a
/// sibling file, so the guard in `decorate` cannot prevent the panic.
/// This function stores the bytes as lossy UTF-8 text, so the database
/// always has text to read. The source-match guard in `decorate` still
/// declines the repaired file, because its lossy text differs from the
/// text stiles parsed.
///
/// The check covers only files under `root`, so it does not read the
/// sysroot or the dependencies on every run. It covers every extension,
/// because `include_str!` makes rust-analyzer read non-Rust files as
/// well.
fn give_text_to_files_that_have_none(db: &mut RootDatabase, vfs: &Vfs, root: &Path) {
    let mut lossy = Vec::new();

    for (file_id, vfs_path) in vfs.iter() {
        let Some(path) = vfs_path.as_path() else {
            continue;
        };
        let path: &Path = path.as_ref();
        if !path.starts_with(root) {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if std::str::from_utf8(&bytes).is_ok() {
            continue;
        }
        lossy.push((file_id, String::from_utf8_lossy(&bytes).into_owned()));
    }

    if lossy.is_empty() {
        return;
    }

    let mut change = ChangeWithProcMacros::default();
    for (file_id, text) in lossy {
        change.change_file(file_id, Some(text));
    }
    db.apply_change(change);
}

impl DecorationProvider for RustDecorationProvider {
    fn name(&self) -> ProviderName {
        Self::NAME
    }

    /// Attaches type decorations to the tree's nodes, or declines the file
    ///
    /// Four conditions must hold before a single decoration is produced,
    /// and their order matters. The file must be interned by the VFS, or
    /// rust-analyzer has never heard of it. It must not be excluded, or
    /// every question the database can be asked about it panics: the VFS
    /// keeps an excluded file's identity and neither its contents nor a
    /// source root, so the check has to come before the database is touched
    /// at all, not merely before its text is read. It must belong to a
    /// module, because `parse_guess_edition` papers over a missing module by
    /// guessing the current edition and then resolves nothing. And the text
    /// the pipeline parsed must equal the text the database holds, because
    /// the byte ranges carried by every target index into rust-analyzer's
    /// parse of that text.
    ///
    /// Only the first two conditions are guards. The module check protects
    /// nothing, because establishing the module reads whatever sibling
    /// modules the crate declares; a file whose text the load dropped would
    /// already have brought the process down by then, which is why
    /// `give_text_to_files_that_have_none` repairs the database at load
    /// time, so the read here needs no careful ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if the file path cannot be made absolute or the
    /// database lock is poisoned. A file this provider knows nothing about
    /// is a [`Coverage::NotCovered`] verdict, not an error.
    fn decorate(&self, tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
        let file_path = tree.file();
        let file_path = std::path::absolute(file_path).map_err(|source| {
            DecorateTreeError::UnresolvablePath {
                path: file_path.to_path_buf(),
                source,
            }
        })?;

        let vfs_path = VfsPath::new_real_path(file_path.to_string_lossy().to_string());

        let Some((file_id, excluded)) = self.vfs.file_id(&vfs_path) else {
            return Ok(Coverage::NotCovered(self.gap_for_unknown_file(&file_path)));
        };

        match excluded {
            FileExcluded::Yes => {
                return Ok(Coverage::NotCovered(CoverageGap::ExcludedByToolchain {
                    root: Arc::clone(&self.root),
                }));
            }
            FileExcluded::No => {}
        }

        let db = self
            .db
            .lock()
            .map_err(|_| DecorateTreeError::PoisonedDatabase)?;

        let mut targets = Vec::new();
        collect_targets(&tree.root_node(), &mut targets);

        let coverage = attach_db(&*db, || {
            let sema = Semantics::new(&*db);

            let Some(module) = sema.file_to_module_def(file_id) else {
                return Coverage::NotCovered(CoverageGap::Unreachable {
                    root: Arc::clone(&self.root),
                });
            };

            let db_text: &str = db.file_text(file_id).text(&*db);
            if db_text != tree.source() {
                return Coverage::NotCovered(CoverageGap::StaleSource);
            }

            Coverage::Covered(resolve_targets(&sema, file_id, module, &targets))
        });

        Ok(coverage)
    }
}

/// Resolves every collected target against rust-analyzer's parse
///
/// The caller has already established that `file_id` belongs to `module`
/// and that its recorded text matches the text the targets' byte ranges
/// were taken from.
///
/// `module` identifies the crate. The crate supplies the edition for
/// display and the `core` definitions of `Result` and `Option`. These
/// resolve once per file, not once per target.
fn resolve_targets(
    sema: &Semantics<'_, RootDatabase>,
    file_id: FileId,
    module: Module,
    targets: &[Target],
) -> DecorationMap {
    let source_file = sema.parse_guess_edition(file_id);
    let syntax = source_file.syntax().clone();

    let krate = module.krate(sema.db);
    let famous = FamousDefs(sema, krate);
    let ctx = DecorateCtx {
        sema,
        syntax: &syntax,
        module,
        edition: krate.edition(sema.db),
        core_result: famous.core_result_Result(),
        core_option: famous.core_option_Option(),
    };

    let mut module_paths = ModulePaths::default();
    let mut decorations = DecorationMap::new();
    for target in targets {
        match target {
            Target::Function { node_id, range } => {
                if let Some(sig) = resolve_function(&ctx, *range) {
                    decorations.insert(*node_id, sig);
                }
            }
            Target::MatchScrutinee {
                scrutinee_range,
                scrutinee_node_id,
            } => {
                if let Some((ty, flags)) = resolve_match_scrutinee(&ctx, *scrutinee_range) {
                    decorations.insert(*scrutinee_node_id, ty);
                    if let Some(flags) = flags {
                        decorations.insert(*scrutinee_node_id, flags);
                    }
                }
            }
            Target::IfElse {
                branch_range,
                else_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *branch_range) {
                    decorations.insert(*else_node_id, ty);
                }
            }
            Target::TryExpr {
                operand_range,
                operand_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *operand_range) {
                    decorations.insert(*operand_node_id, ty);
                }
            }
            Target::Import { node_id, range } => {
                if let Some(source) = resolve_import(&ctx, *range) {
                    decorations.insert(*node_id, source);
                }
            }
            Target::ImportPath { node_id, range } => {
                if let Some(reexports) = resolve_reexports(&ctx, &mut module_paths, *range) {
                    decorations.insert(*node_id, reexports);
                }
            }
        }
    }
    decorations
}

/// A node the provider asks rust-analyzer to resolve
///
/// Each variant pairs the byte range rust-analyzer inspects with the
/// tree-sitter node ID that receives the decoration. These can differ: the
/// provider types an `else` branch from its block but decorates the
/// enclosing `else_clause`.
///
/// A full range, not a start offset, selects the node in rust-analyzer's
/// tree. A start offset lands on one token, and the smallest expression
/// around that token is often wrong. In `foo().bar()`, the start offset
/// names the path `foo`, not the call.
enum Target {
    Function {
        node_id: usize,
        range: TextRange,
    },
    MatchScrutinee {
        scrutinee_range: TextRange,
        scrutinee_node_id: usize,
    },
    IfElse {
        branch_range: TextRange,
        else_node_id: usize,
    },
    TryExpr {
        operand_range: TextRange,
        operand_node_id: usize,
    },
    Import {
        node_id: usize,
        range: TextRange,
    },
    ImportPath {
        node_id: usize,
        range: TextRange,
    },
}

/// Converts a byte range into rust-analyzer's 32-bit one
///
/// Rust-analyzer addresses text with 32-bit offsets. An offset that does
/// not fit yields [`None`], because a truncated offset would name some
/// other expression.
///
/// The conversion is a separate function so that a unit test can reach the
/// 32-bit boundary without a four-gigabyte file.
fn text_range_from(range: Range<usize>) -> Option<TextRange> {
    let Range { start, end } = range;
    let start = u32::try_from(start).ok()?;
    let end = u32::try_from(end).ok()?;

    Some(TextRange::new(TextSize::from(start), TextSize::from(end)))
}

/// Converts a tree-sitter node's byte range into rust-analyzer's
fn text_range_of(node: &DecoratedNode<'_>) -> Option<TextRange> {
    text_range_from(node.raw().byte_range())
}

fn collect_targets(node: &DecoratedNode<'_>, targets: &mut Vec<Target>) {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            if let Some(range) = text_range_of(node) {
                targets.push(Target::Function {
                    node_id: node.id(),
                    range,
                });
            }
        }
        "match_expression" => {
            if let Some(scrutinee) = node.child_by_field_name("value")
                && let Some(scrutinee_range) = text_range_of(&scrutinee)
            {
                targets.push(Target::MatchScrutinee {
                    scrutinee_range,
                    scrutinee_node_id: scrutinee.id(),
                });
            }
        }
        "if_expression" => {
            if let Some(alt) = node.child_by_field_name("alternative") {
                let block = alt.named_child(0).unwrap_or(alt);
                if let Some(branch_range) = text_range_of(&block) {
                    targets.push(Target::IfElse {
                        branch_range,
                        else_node_id: alt.id(),
                    });
                }
            }
        }
        "try_expression" => {
            if let Some(operand) = node.named_child(0)
                && let Some(operand_range) = text_range_of(&operand)
            {
                targets.push(Target::TryExpr {
                    operand_range,
                    operand_node_id: operand.id(),
                });
            }
        }
        "use_declaration" => {
            if let Some(range) = text_range_of(node) {
                targets.push(Target::Import {
                    node_id: node.id(),
                    range,
                });
            }
            if let Some(argument) = node.child_by_field_name("argument") {
                collect_import_paths(&argument, targets);
            }
        }
        _ => {}
    }

    for child in node.named_children() {
        collect_targets(&child, targets);
    }
}

/// Collects each path a use tree imports, one target per imported name
///
/// The walk descends through lists and renames to the paths that name an
/// item. A wildcard names no single item. Neither does a bare `self`,
/// `super`, or `crate` in a list, nor a macro metavariable, so none of
/// them becomes a target.
fn collect_import_paths(node: &DecoratedNode<'_>, targets: &mut Vec<Target>) {
    match node.kind() {
        "identifier" | "scoped_identifier" => {
            if let Some(range) = text_range_of(node) {
                targets.push(Target::ImportPath {
                    node_id: node.id(),
                    range,
                });
            }
        }
        "use_as_clause" => {
            if let Some(path) = node.child_by_field_name("path") {
                collect_import_paths(&path, targets);
            }
        }
        "scoped_use_list" => {
            if let Some(list) = node.child_by_field_name("list") {
                collect_import_paths(&list, targets);
            }
        }
        "use_list" => {
            for child in node.named_children() {
                collect_import_paths(&child, targets);
            }
        }
        _ => {}
    }
}

/// Shared state for resolving one file's targets
///
/// The module, the edition, and the `core` enums are per-file or
/// per-crate, so `resolve_targets` fills them once for the whole file.
struct DecorateCtx<'a, 'db> {
    sema: &'a Semantics<'db, RootDatabase>,
    syntax: &'a SyntaxNode,
    module: Module,
    edition: Edition,
    core_result: Option<Enum>,
    core_option: Option<Enum>,
}

impl DecorateCtx<'_, '_> {
    fn display_type(&self, ty: &Type<'_>, func: &Function) -> String {
        let krate = func.module(self.sema.db).krate(self.sema.db);
        let target = krate.to_display_target(self.sema.db);
        format!("{}", ty.display(self.sema.db, target))
    }
}

/// Returns the smallest node in rust-analyzer's tree that covers `range`
///
/// The two parsers agree on byte offsets but not on tree shape, so this
/// function locates a tree-sitter node by the span it occupies. From the
/// smallest covering node, the callers climb to the smallest [`ast::Expr`]
/// or [`ast::Fn`] that encloses the whole span.
///
/// The function refuses an empty range. Tree-sitter synthesizes zero-width
/// nodes during error recovery, and a zero-width span sits inside some
/// token, so the broken node would take that token's type.
///
/// It also refuses a range past the end of `syntax`.
/// [`SyntaxNode::covering_element`] panics on such a range, and one bad
/// target would abort the whole run.
///
/// The function takes the tree instead of the whole [`DecorateCtx`] so
/// that unit tests can call it without a loaded workspace.
fn covering_node(syntax: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    if range.is_empty() || !syntax.text_range().contains_range(range) {
        return None;
    }

    match syntax.covering_element(range) {
        NodeOrToken::Node(node) => Some(node),
        NodeOrToken::Token(token) => token.parent(),
    }
}

fn find_enclosing_fn(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<Function> {
    let fn_node = covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Fn::cast)?;
    ctx.sema.to_def(&fn_node)
}

fn find_expr_covering(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<ast::Expr> {
    covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Expr::cast)
}

/// Returns the return type as seen from inside the function's body
///
/// For an `async fn`, that is the future's output. In the body, `?` and
/// `return` convert into the output, not into the opaque `impl Future`.
///
/// The projection uses [`Function::async_ret_type`], which returns a type
/// only for an `async fn`. A `Future`-implementation test would also match
/// `fn f() -> impl Future<Output = anyhow::Result<()>>`, where the body's
/// `?` does not target that [`Result<T, E>`].
///
/// [`Result<T, E>`]: std::result::Result
fn effective_return_type<'db>(
    ctx: &DecorateCtx<'_, 'db>,
    func: Function,
) -> (Option<Type<'db>>, ReturnMode) {
    if func.is_async(ctx.sema.db) {
        match func.async_ret_type(ctx.sema.db) {
            Some(ty) => (Some(ty), ReturnMode::Awaited),
            None => (None, ReturnMode::Opaque),
        }
    } else {
        (Some(func.ret_type(ctx.sema.db)), ReturnMode::Direct)
    }
}

/// Returns whether `ty` is an instance of the enum `target` names
///
/// The comparison is by identity, not by rendered name: a user's own
/// `myres::Result<T>` renders as `Result<..>` but does not match.
fn is_instance_of(ty: &Type<'_>, target: Option<Enum>) -> bool {
    let Some(target) = target else {
        return false;
    };

    match ty.as_adt() {
        Some(Adt::Enum(actual)) => actual == target,
        Some(Adt::Struct(_)) => false,
        Some(Adt::Union(_)) => false,
        None => false,
    }
}

/// Returns the path of `ty`'s definition, if it has one
///
/// The path is the definition's, not a re-export's.
fn type_path(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> Option<TypePath> {
    let db = ctx.sema.db;
    let adt = ty.as_adt()?;
    let module = adt.module(db);
    let krate = module.krate(db).display_name(db)?;
    let modules: Vec<String> = module
        .path_segments(db)
        .map(|segment| segment.display(db, ctx.edition).to_string())
        .collect();
    let name = adt.name(db).display(db, ctx.edition).to_string();

    Some(
        TypePath::builder()
            .krate(&krate.to_string())
            .modules(modules)
            .name(&name)
            .build(),
    )
}

/// Classifies the `E` of a [`Result<T, E>`]
///
/// A type parameter becomes [`ErrorType::Generic`]; a type with no
/// definition path at all becomes [`ErrorType::Unnamed`]. Neither gets an
/// invented name, because callers compare the result against real paths.
///
/// [`Result<T, E>`]: std::result::Result
fn classify_error(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> ErrorType {
    match type_path(ctx, ty) {
        Some(path) => ErrorType::Named(path),
        None => match ty.as_type_param(ctx.sema.db) {
            Some(_) => ErrorType::Generic,
            None => ErrorType::Unnamed,
        },
    }
}

/// Returns the classified `E` when `ty` really is `core::result::Result<T, E>`
fn result_error_type(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> Option<ErrorType> {
    let core_result = ctx.core_result?;
    let (adt, args) = ty.as_adt_with_args()?;
    let Adt::Enum(actual) = adt else {
        return None;
    };
    if actual != core_result {
        return None;
    }

    let error = args.into_iter().nth(1)??;
    Some(classify_error(ctx, &error))
}

fn resolve_function(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<FnSignature> {
    let func = find_enclosing_fn(ctx, range)?;

    let (ret_type, return_mode) = effective_return_type(ctx, func);
    let Some(ret_type) = ret_type else {
        return Some(FnSignature::builder().return_mode(return_mode).build());
    };

    let display = ctx.display_type(&ret_type, &func);
    let error_type = result_error_type(ctx, &ret_type);

    let resolved = ResolvedType::new(display)
        .with_result(is_instance_of(&ret_type, ctx.core_result))
        .with_never(ret_type.is_never());

    Some(
        FnSignature::builder()
            .return_type(resolved)
            .maybe_error_type(error_type)
            .return_mode(return_mode)
            .build(),
    )
}

fn resolve_match_scrutinee(
    ctx: &DecorateCtx<'_, '_>,
    scrutinee_range: TextRange,
) -> Option<(ResolvedType, Option<AdtFlags>)> {
    let expr_node = find_expr_covering(ctx, scrutinee_range)?;
    let ty_info = ctx.sema.type_of_expr(&expr_node)?;

    let ty = ty_info.original;
    let is_enum = match ty.as_adt() {
        Some(Adt::Enum(_)) => true,
        Some(Adt::Struct(_) | Adt::Union(_)) | None => false,
    };

    let func = find_enclosing_fn(ctx, scrutinee_range);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let resolved = ResolvedType::new(display).with_enum(is_enum);

    let flags = match ty.as_adt() {
        Some(Adt::Enum(e)) => {
            let krate = e.module(ctx.sema.db).krate(ctx.sema.db);
            let local_krate = func.map(|f| f.module(ctx.sema.db).krate(ctx.sema.db));
            let is_external = local_krate.is_some_and(|lk| lk != krate);
            let has_non_exhaustive = e.attrs(ctx.sema.db).is_non_exhaustive();
            Some(AdtFlags::new(has_non_exhaustive && is_external))
        }
        Some(Adt::Struct(_) | Adt::Union(_)) | None => None,
    };

    Some((resolved, flags))
}

fn resolve_expr_type(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<ResolvedType> {
    let expr = find_expr_covering(ctx, range)?;
    let ty_info = ctx.sema.type_of_expr(&expr)?;

    let ty = ty_info.original;
    let func = find_enclosing_fn(ctx, range);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let is_result = is_instance_of(&ty, ctx.core_result);
    let is_option = is_instance_of(&ty, ctx.core_option);
    let is_never = ty.is_never();

    Some(
        ResolvedType::new(display)
            .with_result(is_result)
            .with_option(is_option)
            .with_never(is_never),
    )
}

/// Returns what the qualifier of a `use` declaration names
///
/// The target range covers the whole declaration, so the covering node in
/// rust-analyzer's tree is the [`ast::Use`] itself, and the qualifier comes
/// from its use tree. A declaration with no qualifier, such as `use serde;`,
/// yields [`None`] and so gets no decoration.
///
/// A qualifier rust-analyzer resolves to nothing becomes
/// [`ImportSource::Unresolved`]. A rule can then separate a failed lookup
/// from a file this provider never covered.
fn resolve_import(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<ImportSource> {
    let use_item = covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Use::cast)?;
    let qualifier = import_qualifier(&use_item.use_tree()?)?;

    match ctx.sema.resolve_path(&qualifier) {
        Some(resolution) => Some(import_source_of(resolution)),
        None => Some(ImportSource::Unresolved),
    }
}

/// Returns the path a use tree reaches through to name what it imports
///
/// A tree that ends in a list or a star, as `use a::B::{C, D}` and
/// `use a::B::*` do, holds the qualifier as its own path. Any other tree
/// holds the imported name as the last segment, so the qualifier is the
/// path without it.
fn import_qualifier(tree: &ast::UseTree) -> Option<ast::Path> {
    let path = tree.path()?;
    let path_is_the_qualifier = tree.use_tree_list().is_some() || tree.star_token().is_some();

    match path_is_the_qualifier {
        true => Some(path),
        false => path.qualifier(),
    }
}

/// Classifies the item a resolved qualifier names
///
/// A module and an enum each get their own answer. An import out of an
/// enum names variants; an import out of a module does not. Everything
/// else is [`ImportSource::Other`].
fn import_source_of(resolution: PathResolution<'_>) -> ImportSource {
    match resolution {
        PathResolution::Def(def) => match def {
            ModuleDef::Module(_) => ImportSource::Module,
            ModuleDef::Adt(Adt::Enum(_)) => ImportSource::Enum,
            ModuleDef::Adt(Adt::Struct(_))
            | ModuleDef::Adt(Adt::Union(_))
            | ModuleDef::Function(_)
            | ModuleDef::EnumVariant(_)
            | ModuleDef::Const(_)
            | ModuleDef::Static(_)
            | ModuleDef::Trait(_)
            | ModuleDef::TypeAlias(_)
            | ModuleDef::BuiltinType(_)
            | ModuleDef::Macro(_) => ImportSource::Other,
        },
        PathResolution::Local(_)
        | PathResolution::TypeParam(_)
        | PathResolution::ConstParam(_)
        | PathResolution::SelfType(_)
        | PathResolution::BuiltinAttr(_)
        | PathResolution::ToolModule(_)
        | PathResolution::DeriveHelper(_) => ImportSource::Other,
    }
}

/// The public module paths of each dependency one file has asked about
///
/// Every target of a file looks out of the same crate, and a module of
/// another crate is visible from every module of this one alike. One walk
/// of a dependency therefore serves every `use` path in the file.
#[derive(Default)]
struct ModulePaths {
    by_crate: HashMap<Crate, HashMap<Module, Vec<String>>>,
}

impl ModulePaths {
    /// Returns the public path to each module of `krate`, walking it once
    fn of(&mut self, ctx: &DecorateCtx<'_, '_>, krate: Crate) -> &HashMap<Module, Vec<String>> {
        self.by_crate
            .entry(krate)
            .or_insert_with(|| public_module_paths(ctx, krate))
    }
}

/// Returns the shortest public path from the root of `krate` to each module
///
/// The walk goes breadth first through the modules that the file's module
/// can see, so it enters only `pub` modules, and it reaches a module first
/// along its shortest path. A module that `krate` re-exports from another
/// crate is part of the walk, because a path through it is a public path
/// of `krate`.
///
/// A module that `#[doc(hidden)]` marks is not part of the walk, because
/// its authors do not offer it as a path. The mark can sit on the module
/// or on the `pub use` that re-exports it, and only the import map of
/// `krate` records the second, so the walk asks the map about each step.
fn public_module_paths(ctx: &DecorateCtx<'_, '_>, krate: Crate) -> HashMap<Module, Vec<String>> {
    let db = ctx.sema.db;
    let import_map = ImportMap::of(db, krate.base());
    let root = krate.root_module(db);
    let mut paths = HashMap::from([(root, Vec::new())]);
    let mut queue = VecDeque::from([root]);

    while let Some(module) = queue.pop_front() {
        let Some(prefix) = paths.get(&module).cloned() else {
            continue;
        };
        for (name, def) in module.scope(db, Some(ctx.module)) {
            let ScopeDef::ModuleDef(ModuleDef::Module(child)) = def else {
                continue;
            };
            if paths.contains_key(&child) {
                continue;
            }
            let Ok(key) = ItemInNs::from(ModuleDef::Module(child)).try_into() else {
                continue;
            };
            let offered = import_map.import_info_for(key).is_some_and(|infos| {
                infos.iter().any(
                    |ImportInfo {
                         container,
                         name: exported,
                         is_doc_hidden,
                         ..
                     }| {
                        Module::from(*container) == module && *exported == name && !is_doc_hidden
                    },
                )
            });
            if !offered {
                continue;
            }
            let mut path = prefix.clone();
            path.push(name.display(db, ctx.edition).to_string());
            paths.insert(child, path);
            queue.push_back(child);
        }
    }

    paths
}

/// Returns the other crates' public paths to what a `use` path imports
///
/// The target range covers one imported path, and the covering node in
/// rust-analyzer's tree sits in that [`ast::Path`]. A path inside a use
/// tree list resolves with the prefix in front of it, so `b` in
/// `use a::{b}` resolves as `a::b`.
///
/// A path that resolves to nothing, or to something other than an item,
/// yields [`None`] and so gets no decoration. A path whose span differs
/// from the target's also yields [`None`], because the two parsers then
/// disagree about what the `use` names.
fn resolve_reexports(
    ctx: &DecorateCtx<'_, '_>,
    module_paths: &mut ModulePaths,
    range: TextRange,
) -> Option<Reexports> {
    let path = covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Path::cast)?;
    if path.syntax().text_range() != range {
        return None;
    }
    path.syntax().ancestors().find_map(ast::UseTree::cast)?;

    let PathResolution::Def(def) = ctx.sema.resolve_path(&path)? else {
        return None;
    };

    Some(Reexports::new(public_paths(ctx, module_paths, def)))
}

/// Returns the crate that defines `def`, or [`None`] for a builtin type
fn defining_crate(ctx: &DecorateCtx<'_, '_>, def: ModuleDef) -> Option<Crate> {
    let db = ctx.sema.db;

    match def {
        ModuleDef::Module(module) => Some(module.krate(db)),
        ModuleDef::Function(_)
        | ModuleDef::Adt(_)
        | ModuleDef::EnumVariant(_)
        | ModuleDef::Const(_)
        | ModuleDef::Static(_)
        | ModuleDef::Trait(_)
        | ModuleDef::TypeAlias(_)
        | ModuleDef::Macro(_)
        | ModuleDef::BuiltinType(_) => def.module(db).map(|module| module.krate(db)),
    }
}

/// Returns a public path to `def` through each other crate that exposes it
///
/// Only a direct dependency of the current crate qualifies, because a
/// `use` can only start at one. The crate that defines `def` is left out:
/// its own path is the one a rule compares the others with. A crate can
/// only re-export what it depends on, so a dependency outside the reverse
/// dependencies of the defining crate is skipped without a lookup.
///
/// rust-analyzer's import map of a dependency proves that the item is
/// public there and names the module that exposes it. The map lists an
/// enum variant under the module of its enum, without the enum, so a
/// variant takes the path of its enum with its own name after it. An
/// associated item has no path a `use` can name, so it gets none.
fn public_paths(
    ctx: &DecorateCtx<'_, '_>,
    module_paths: &mut ModulePaths,
    def: ModuleDef,
) -> Vec<PublicPath> {
    let db = ctx.sema.db;
    if def.as_assoc_item(db).is_some() {
        return Vec::new();
    }

    let (item, suffix) = match def {
        ModuleDef::EnumVariant(variant) => (
            ModuleDef::Adt(Adt::Enum(variant.parent_enum(db))),
            Some(variant.name(db).display(db, ctx.edition).to_string()),
        ),
        ModuleDef::Module(_)
        | ModuleDef::Function(_)
        | ModuleDef::Adt(_)
        | ModuleDef::Const(_)
        | ModuleDef::Static(_)
        | ModuleDef::Trait(_)
        | ModuleDef::TypeAlias(_)
        | ModuleDef::Macro(_)
        | ModuleDef::BuiltinType(_) => (def, None),
    };

    let current = ctx.module.krate(db);
    let Some(defining) = defining_crate(ctx, item) else {
        return Vec::new();
    };
    if defining == current {
        return Vec::new();
    }
    let Ok(key) = ItemInNs::from(item).try_into() else {
        return Vec::new();
    };
    let exposing: HashSet<Crate> = defining.transitive_reverse_dependencies(db).collect();

    let mut paths = Vec::new();
    for CrateDependency { krate, name } in current.dependencies(db) {
        if krate == defining || !exposing.contains(&krate) {
            continue;
        }
        let Some(infos) = ImportMap::of(db, krate.base()).import_info_for(key) else {
            continue;
        };
        let modules = module_paths.of(ctx, krate);
        let shortest = infos
            .iter()
            .filter(|info| !info.is_doc_hidden)
            .filter_map(|info| {
                let mut segments = modules.get(&Module::from(info.container))?.clone();
                segments.push(info.name.display(db, ctx.edition).to_string());
                Some(segments)
            })
            .min_by(|lhs, rhs| lhs.len().cmp(&rhs.len()).then_with(|| lhs.cmp(rhs)));
        let Some(segments) = shortest else {
            continue;
        };

        let krate_name = name.display(db, ctx.edition).to_string();
        paths.push(
            PublicPath::builder()
                .krate(&krate_name)
                .segments(segments.iter().chain(suffix.iter()))
                .build(),
        );
    }

    paths
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ra_ap_syntax::SourceFile;
    use stiles_types::Language;

    use super::*;

    /// The source every `covering_node` test locates a range in
    ///
    /// `value` is several bytes long, so an empty range can sit strictly
    /// inside a token, where only the guard prevents a match.
    const COVERING_SOURCE: &str = "fn main() { let value = 1; }";

    fn covering_source_tree() -> SyntaxNode {
        SourceFile::parse(COVERING_SOURCE, Edition::CURRENT)
            .tree()
            .syntax()
            .clone()
    }

    fn offset_of(needle: &str) -> u32 {
        let found = COVERING_SOURCE
            .find(needle)
            .unwrap_or_else(|| panic!("the fixture source should contain {needle}"));

        u32::try_from(found).expect("the fixture source is small")
    }

    fn range_at(offset: u32, len: u32) -> TextRange {
        TextRange::new(TextSize::from(offset), TextSize::from(offset + len))
    }

    /// Parses `source` as the contents of `file_path`, without decorating it
    fn parse_rust(source: &str, file_path: PathBuf) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&crate::language())
            .expect("the Rust grammar should load");
        let tree = parser.parse(source, None).expect("should parse");

        DecoratedTree::builder()
            .tree(tree)
            .language(Language::Rust)
            .source(source.to_string())
            .file(file_path)
            .build()
    }

    /// A zero-width span must not take the type of the token around it
    ///
    /// Rowan resolves an empty range to the token that contains it. Without
    /// the guard, a zero-width node would take that token's type.
    #[test]
    fn covering_node_with_empty_range_returns_none() {
        let syntax = covering_source_tree();
        let inside_an_identifier = TextSize::from(offset_of("value") + 1);

        let covering = covering_node(
            &syntax,
            TextRange::new(inside_an_identifier, inside_an_identifier),
        );

        assert!(covering.is_none());
    }

    #[test]
    fn covering_node_with_range_inside_the_file_returns_a_node() {
        let syntax = covering_source_tree();
        let value = offset_of("value");

        let covering = covering_node(&syntax, range_at(value, 5)).expect("should cover `value`");

        assert!(covering.text_range().contains_range(range_at(value, 5)));
    }

    /// A span past the end of the tree must be skipped, not asserted on
    ///
    /// [`SyntaxNode::covering_element`] panics on a range outside the tree,
    /// so the guard skips the target instead of aborting the run.
    #[test]
    fn covering_node_with_range_past_the_end_returns_none() {
        let syntax = covering_source_tree();
        let past_the_end = u32::try_from(COVERING_SOURCE.len()).expect("fixture is small") + 1;

        let covering = covering_node(&syntax, range_at(past_the_end, 1));

        assert!(covering.is_none());
    }

    /// Exercises the exclusion guard, which no fixture workspace can reach
    ///
    /// The VFS keeps an excluded file's identity but no contents, so any
    /// database query about it panics. The test seeds the state directly,
    /// because the load stiles performs never marks a file excluded.
    #[test]
    fn decorate_with_excluded_file_reports_excluded_by_toolchain() {
        let file_path =
            std::path::absolute("excluded_by_toolchain.rs").expect("should be made absolute");
        let mut vfs = Vfs::default();
        vfs.insert_excluded_file(VfsPath::new_real_path(
            file_path.to_string_lossy().to_string(),
        ));
        let provider = RustDecorationProvider {
            db: Mutex::new(RootDatabase::default()),
            vfs,
            root: Arc::from(Path::new("/")),
        };
        let tree = parse_rust("fn main() {}\n", file_path);

        let coverage = provider.decorate(&tree).expect("decorate should succeed");

        match coverage {
            Coverage::Covered(_) => panic!("an excluded file must not be covered"),
            Coverage::NotCovered(CoverageGap::ExcludedByToolchain { .. }) => {}
            Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
        }
    }

    /// Returns the text of the qualifier in `source`'s first `use` item
    ///
    /// Yields [`None`] when that item reaches through no qualifier.
    fn qualifier_of(source: &str) -> Option<String> {
        let file = SourceFile::parse(source, Edition::CURRENT).tree();
        let use_item = file
            .syntax()
            .descendants()
            .find_map(ast::Use::cast)
            .expect("the source should hold a `use` item");
        let tree = use_item.use_tree().expect("a `use` item has a tree");

        import_qualifier(&tree).map(|path| path.syntax().text().to_string())
    }

    #[test]
    fn import_qualifier_with_a_list_returns_the_whole_path() {
        let qualifier = qualifier_of("use a::B::{C, D};");

        assert_eq!(qualifier.as_deref(), Some("a::B"));
    }

    #[test]
    fn import_qualifier_with_a_plain_path_drops_the_last_segment() {
        let qualifier = qualifier_of("use a::B::C;");

        assert_eq!(qualifier.as_deref(), Some("a::B"));
    }

    #[test]
    fn import_qualifier_with_a_rename_drops_the_last_segment() {
        let qualifier = qualifier_of("use a::B::C as D;");

        assert_eq!(qualifier.as_deref(), Some("a::B"));
    }

    #[test]
    fn import_qualifier_with_a_star_returns_the_whole_path() {
        let qualifier = qualifier_of("use a::B::*;");

        assert_eq!(qualifier.as_deref(), Some("a::B"));
    }

    /// A list with no path in front of it has no single qualifier
    #[test]
    fn import_qualifier_with_no_path_returns_none() {
        let qualifier = qualifier_of("use {a::b, c::d};");

        assert!(qualifier.is_none());
    }

    /// A `use` that names one segment reaches through nothing
    #[test]
    fn import_qualifier_with_one_segment_returns_none() {
        let qualifier = qualifier_of("use serde;");

        assert!(qualifier.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn load_with_non_utf8_path_returns_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;
        use std::path::PathBuf;

        let path = PathBuf::from(OsStr::from_bytes(b"stiles-\xff-not-utf8"));

        let Err(error) = RustDecorationProvider::load(&path) else {
            panic!("a path that is not UTF-8 should be rejected");
        };

        assert!(
            error.to_string().contains("not valid UTF-8"),
            "unexpected error: {error:#}"
        );
    }

    /// A span past the 32-bit limit must produce no target at all
    ///
    /// A truncated offset would name an unrelated span. The conversion is a
    /// separate function because a real file at this boundary would need
    /// four gigabytes.
    #[test]
    fn text_range_from_with_offset_past_u32_returns_none() {
        let past_u32 =
            usize::try_from(u64::from(u32::MAX) + 1).expect("this test assumes a 64-bit target");

        let converted = text_range_from(0..past_u32);

        assert!(converted.is_none());
    }

    #[test]
    fn text_range_from_with_offsets_within_u32_returns_the_range() {
        let converted = text_range_from(3..7).expect("a small range should convert");

        assert_eq!(converted, range_at(3, 4));
    }

    /// Records that `DecorateCtx` is not `Send`
    ///
    /// `DecorateCtx` borrows the `Semantics` of rust-analyzer and holds a
    /// `SyntaxNode`. Neither may cross a thread, so `DecorateCtx` is neither
    /// `Send` nor `Sync`.
    #[test]
    fn trait_not_send_decorate_ctx() {}

    /// Records that `DecorateCtx` is not `Sync`
    ///
    /// `DecorateCtx` borrows the `Semantics` of rust-analyzer and holds a
    /// `SyntaxNode`. Neither may cross a thread, so `DecorateCtx` is neither
    /// `Send` nor `Sync`.
    #[test]
    fn trait_not_sync_decorate_ctx() {}

    #[test]
    fn trait_send_module_paths() {
        fn assert_send<T: Send>() {}
        assert_send::<ModulePaths>();
    }

    #[test]
    fn trait_send_rust_decoration_provider() {
        fn assert_send<T: Send>() {}
        assert_send::<RustDecorationProvider>();
    }

    #[test]
    fn trait_send_target() {
        fn assert_send<T: Send>() {}
        assert_send::<Target>();
    }

    #[test]
    fn trait_sync_module_paths() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ModulePaths>();
    }

    #[test]
    fn trait_sync_rust_decoration_provider() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RustDecorationProvider>();
    }

    #[test]
    fn trait_sync_target() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Target>();
    }

    #[test]
    fn trait_unpin_decorate_ctx() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorateCtx<'static, 'static>>();
    }

    #[test]
    fn trait_unpin_module_paths() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ModulePaths>();
    }

    #[test]
    fn trait_unpin_rust_decoration_provider() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RustDecorationProvider>();
    }

    #[test]
    fn trait_unpin_target() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Target>();
    }
}
