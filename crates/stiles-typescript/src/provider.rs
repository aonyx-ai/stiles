use std::path::{Path, PathBuf};
use std::sync::Arc;

use bon::bon;
use stiles_tsrs::tsrs_compiler::{
    Program, ProgramOptions, new_cached_fs_compiler_host, new_program,
};
use stiles_tsrs::tsrs_core::P;
use stiles_tsrs::tsrs_core::context::Context;
use stiles_tsrs::tsrs_tsoptions::{ParseConfigHost, ParsedCommandLine};
use stiles_tsrs::tsrs_vfs::{FS, bundled, osvfs};
use stiles_tsrs::{tsrs_ast, tsrs_astnav, tsrs_tsoptions};
use stiles_types::{
    Coverage, CoverageGap, DecorateTreeError, DecoratedNode, DecoratedTree, DecorationMap,
    DecorationProvider, ProviderName,
};

use crate::decorations::{ImportSource, TypeText};
use crate::queries;

mod load_type_script_project_error;

pub use self::load_type_script_project_error::LoadTypeScriptProjectError;

/// The file that marks the root of a TypeScript project
const CONFIG_FILE: &str = "tsconfig.json";

/// The node kinds that name a value the checker can type
const TYPED_KINDS: [&str; 3] = [
    "identifier",
    "property_identifier",
    "shorthand_property_identifier",
];

/// The node kinds that declare a function or method with a signature
const FUNCTION_KINDS: [&str; 6] = [
    "function_declaration",
    "generator_function_declaration",
    "function_signature",
    "method_definition",
    "method_signature",
    "abstract_method_signature",
];

/// The node kinds whose `source` field names a module
const MODULE_REFERENCE_KINDS: [&str; 2] = ["import_statement", "export_statement"];

/// Decorates TypeScript trees with what the TypeScript checker knows
///
/// The provider loads one TypeScript project through tsrs, a Rust port of
/// the TypeScript 7 checker, and answers for the files that project's
/// program holds. It attaches a [`TypeText`] and a [`ResolvedType`] to
/// each identifier the checker can type. An identifier whose type is a union
/// of string literals also gets a [`StringUnion`]. A function or method declaration
/// gets an [`FnSignature`], and the module specifier of an import or
/// export gets an [`ImportSource`].
///
/// [`FnSignature`]: crate::decorations::FnSignature
/// [`ResolvedType`]: crate::decorations::ResolvedType
/// [`StringUnion`]: crate::decorations::StringUnion
///
/// The provider declines a file the program does not hold, with the gap
/// that leads to its remedy:
///
/// - [`CoverageGap::OutsideRoot`] for a file outside the project directory.
/// - [`CoverageGap::ExcludedByToolchain`] for a file that an `include`
///   pattern matches and `exclude` removed.
/// - [`CoverageGap::Unreachable`] for any other file.
///
/// A file whose text changed after the load is [`CoverageGap::StaleSource`],
/// because the checker's offsets would address the old text.
///
/// The program is built once and lives for the rest of the process, as it
/// does in tsrs's own command-line compiler. A check loads each project
/// once, so the memory is the run's.
///
/// # Examples
///
/// ```ignore
/// let provider = TypeScriptDecorationProvider::load(Path::new("."))?;
/// let coverage = provider.decorate(&tree)?;
/// ```
pub struct TypeScriptDecorationProvider {
    program: &'static Program,
    config: P<ParsedCommandLine>,
    root: Arc<Path>,
}

impl TypeScriptDecorationProvider {
    /// Identifies this provider in diagnostics
    ///
    /// A host can report a gap for a TypeScript file on the provider's
    /// behalf, such as a file no project holds. It names the provider with
    /// this.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_typescript::TypeScriptDecorationProvider;
    ///
    /// assert_eq!(TypeScriptDecorationProvider::NAME.0, "typescript");
    /// ```
    pub const NAME: ProviderName = ProviderName("typescript");

    /// Loads the TypeScript project that holds `path`
    ///
    /// The project is the one whose `tsconfig.json` is nearest at or above
    /// `path`, as `tsc` finds it. Its directory becomes the provider's
    /// root, which a gap names.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadTypeScriptProjectError`] if the current directory
    /// cannot be read, or if no `tsconfig.json` sits at or above `path`. It
    /// also returns one if a path is not valid UTF-8, or if the configuration
    /// cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use stiles_typescript::TypeScriptDecorationProvider;
    ///
    /// let provider = TypeScriptDecorationProvider::load(Path::new("."))?;
    /// # Ok::<(), stiles_typescript::LoadTypeScriptProjectError>(())
    /// ```
    pub fn load(path: &Path) -> Result<Self, LoadTypeScriptProjectError> {
        let path = std::path::absolute(path).map_err(|source| {
            LoadTypeScriptProjectError::UnresolvablePath {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let Some(root) = discover_root(&path) else {
            return Err(LoadTypeScriptProjectError::MissingConfig { path });
        };
        let config_file = root.join(CONFIG_FILE);
        let root: Arc<Path> = Arc::from(root);
        let Some(root_text) = root.to_str() else {
            return Err(LoadTypeScriptProjectError::NotUtf8 {
                path: root.to_path_buf(),
            });
        };
        let Some(config_text) = config_file.to_str() else {
            return Err(LoadTypeScriptProjectError::NotUtf8 { path: config_file });
        };

        let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
        let host = ConfigHost {
            fs: Arc::clone(&fs),
            current_directory: root_text.to_owned(),
        };
        let (config, errors) = tsrs_tsoptions::get_parsed_command_line_of_config_file(
            config_text,
            None,
            None,
            &host,
            None,
        );
        if !errors.is_empty() {
            return Err(LoadTypeScriptProjectError::InvalidConfig {
                config: config_file,
                messages: errors.iter().map(|error| error.localize()).collect(),
            });
        }
        let Some(config) = config else {
            return Err(LoadTypeScriptProjectError::EmptyConfig {
                config: config_file,
            });
        };
        let config = P::new(config);

        let compiler_host = new_cached_fs_compiler_host(
            root_text,
            fs,
            &bundled::default_lib_path(),
            None,
            None,
            None,
        );
        let program = new_program(ProgramOptions::new(config, compiler_host));

        Ok(Self {
            program,
            config,
            root,
        })
    }

    /// Returns the directory of the TypeScript project that holds `path`
    ///
    /// That is the directory of the `tsconfig.json` nearest at or above
    /// `path`, which is the project [`TypeScriptDecorationProvider::load`]
    /// loads for it. A caller with files from several projects groups them
    /// by this, and loads each project once.
    ///
    /// Returns [`None`] when no `tsconfig.json` sits at or above `path`, or
    /// when the current directory cannot be read to resolve a relative path.
    ///
    /// [`None`]: std::option::Option::None
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use stiles_typescript::TypeScriptDecorationProvider;
    ///
    /// let root = TypeScriptDecorationProvider::project_root(Path::new("src/main.ts"));
    /// ```
    pub fn project_root(path: &Path) -> Option<PathBuf> {
        let path = std::path::absolute(path).ok()?;
        discover_root(&path)
    }

    /// Returns the gap for a file the program does not hold
    fn gap_for_unknown_file(&self, file: &Path) -> CoverageGap {
        let root = Arc::clone(&self.root);

        if !file.starts_with(&self.root) {
            return CoverageGap::OutsideRoot { root };
        }

        let Some(file) = file.to_str() else {
            return CoverageGap::Unreachable { root };
        };
        let (include, _) = self.config.get_matched_include_spec(file);
        match include.is_empty() {
            true => CoverageGap::Unreachable { root },
            false => CoverageGap::ExcludedByToolchain { root },
        }
    }
}

impl DecorationProvider for TypeScriptDecorationProvider {
    fn name(&self) -> ProviderName {
        Self::NAME
    }

    fn decorate(&self, tree: &DecoratedTree) -> Result<Coverage, DecorateTreeError> {
        let file = tree.file();
        let file =
            std::path::absolute(file).map_err(|source| DecorateTreeError::UnresolvablePath {
                path: file.to_path_buf(),
                source,
            })?;

        let Some(source_file) = file
            .to_str()
            .and_then(|name| self.program.get_source_file(name))
        else {
            return Ok(Coverage::NotCovered(self.gap_for_unknown_file(&file)));
        };

        if source_file.text() != tree.source() {
            return Ok(Coverage::NotCovered(CoverageGap::StaleSource));
        }

        let mut targets = Targets::default();
        collect_targets(&tree.root_node(), &mut targets);

        let context = Context::background();
        let mut checker = self
            .program
            .get_type_checker_for_file(&context, source_file);
        let mut decorations = DecorationMap::new();

        for Target {
            node_id,
            start,
            end,
        } in targets.values
        {
            let token = tsrs_astnav::get_touching_property_name(source_file, start);
            if token.end().source_offset() != Some(end) {
                continue;
            }

            let ty = checker.get_type_at_location(token);
            let text = checker.type_to_string_exported(ty);
            decorations.insert(node_id, TypeText::new(&text));

            let declaration = checker
                .get_symbol_at_location_exported(token)
                .and_then(|symbol| symbol.value_declaration());
            decorations.insert(
                node_id,
                queries::resolved_type(&mut checker, ty, declaration),
            );
            if let Some(union) = queries::string_union(ty) {
                decorations.insert(node_id, union);
            }
        }

        for Target {
            node_id,
            start,
            end,
        } in targets.functions
        {
            let token = tsrs_astnav::get_touching_property_name(source_file, start);
            if token.end().source_offset() != Some(end) {
                continue;
            }
            let Some(declaration) = token.parent() else {
                continue;
            };
            if let Some(signature) = queries::fn_signature(&mut checker, declaration) {
                decorations.insert(node_id, signature);
            }
        }

        for Target {
            node_id,
            start,
            end,
        } in targets.modules
        {
            let token = tsrs_astnav::get_touching_token(source_file, start);
            if token.end().source_offset() != Some(end) || !tsrs_ast::is_string_literal_like(token)
            {
                continue;
            }
            let source: ImportSource = queries::import_source(self.program, source_file, token);
            decorations.insert(node_id, source);
        }

        Ok(Coverage::Covered(decorations))
    }
}

/// A node the provider asks tsrs about, and the byte range tsrs finds it by
///
/// The range is the node's own for an identifier and a module specifier.
/// For a function it is the range of the function's name, because tsrs
/// finds the declaration as the parent of that name. The decoration still
/// goes on the declaration's node.
struct Target {
    node_id: usize,
    start: u32,
    end: u32,
}

#[bon]
impl Target {
    /// Returns the target for `node`, found in tsrs by the range of `found_by`
    #[builder]
    fn new(node: &DecoratedNode<'_>, found_by: &DecoratedNode<'_>) -> Option<Self> {
        Some(Self {
            node_id: node.id(),
            start: u32::try_from(found_by.raw().start_byte()).ok()?,
            end: u32::try_from(found_by.raw().end_byte()).ok()?,
        })
    }
}

/// Every node of a file the provider decorates, by what it asks about it
#[derive(Default)]
struct Targets {
    /// Identifiers, each typed by the checker
    values: Vec<Target>,
    /// Function and method declarations, each found by its name
    functions: Vec<Target>,
    /// Module specifiers of imports and exports
    modules: Vec<Target>,
}

/// Collects every target under `node`
fn collect_targets(node: &DecoratedNode<'_>, targets: &mut Targets) {
    let kind = node.kind();

    if TYPED_KINDS.contains(&kind)
        && let Some(target) = Target::builder().node(node).found_by(node).build()
    {
        targets.values.push(target);
    }
    if FUNCTION_KINDS.contains(&kind)
        && let Some(name) = node.child_by_field_name("name")
        && let Some(target) = Target::builder().node(node).found_by(&name).build()
    {
        targets.functions.push(target);
    }
    if MODULE_REFERENCE_KINDS.contains(&kind)
        && let Some(source) = node.child_by_field_name("source")
        && let Some(target) = Target::builder().node(&source).found_by(&source).build()
    {
        targets.modules.push(target);
    }

    for child in node.named_children() {
        collect_targets(&child, targets);
    }
}

/// Returns the nearest directory at or above `path` that holds a
/// `tsconfig.json`
fn discover_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|directory| directory.join(CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
}

/// The file system and directory tsrs reads a configuration through
struct ConfigHost {
    fs: Arc<dyn FS>,
    current_directory: String,
}

impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }

    fn get_current_directory(&self) -> &str {
        &self.current_directory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_config_host() {
        fn assert_send<T: Send>() {}
        assert_send::<ConfigHost>();
    }

    #[test]
    fn trait_send_target() {
        fn assert_send<T: Send>() {}
        assert_send::<Target>();
    }

    #[test]
    fn trait_send_targets() {
        fn assert_send<T: Send>() {}
        assert_send::<Targets>();
    }

    #[test]
    fn trait_send_typescript_decoration_provider() {
        fn assert_send<T: Send>() {}
        assert_send::<TypeScriptDecorationProvider>();
    }

    #[test]
    fn trait_sync_config_host() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ConfigHost>();
    }

    #[test]
    fn trait_sync_target() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Target>();
    }

    #[test]
    fn trait_sync_targets() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Targets>();
    }

    #[test]
    fn trait_sync_typescript_decoration_provider() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypeScriptDecorationProvider>();
    }

    #[test]
    fn trait_unpin_config_host() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ConfigHost>();
    }

    #[test]
    fn trait_unpin_target() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Target>();
    }

    #[test]
    fn trait_unpin_targets() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Targets>();
    }

    #[test]
    fn trait_unpin_typescript_decoration_provider() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypeScriptDecorationProvider>();
    }
}
