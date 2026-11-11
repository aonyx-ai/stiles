//! Vocabulary for custom lint plugins
//!
//! A custom lint plugin is a dynamic library that stiles compiles and
//! loads at check time, given only a path. Rust has no stable ABI of its
//! own, so two kinds of layout cross the boundary. Stabby lays out the
//! values, such as a span or a rule id, and those layouts hold under any
//! compiler. A `#[repr(C)]` table of `extern "C"` functions carries every
//! call, such as the host's [`Registrar`] or the table of a plugin's
//! [`PassInstance`]. A later version appends entries to a table. A loaded
//! library is coherent with the stiles binary when both lay these out the
//! same way. That is a property of the source each was built from, not of
//! the rustc that built it. This module defines the declaration a plugin
//! exports and the constants stiles compares to establish exactly that
//! before it trusts anything else in the library.
//!
//! The handshake proceeds in order of decreasing layout stability:
//!
//! 1. [`PluginDeclaration::abi_version`] is a pair of bare integers at
//!    offset zero of a `#[repr(C)]` struct, so it reads correctly
//!    whatever else changed.
//! 2. The two fingerprints and the [`Provenance`] are plain integers,
//!    readable under any pair of compilers.
//! 3. When both sides are published, the matching version vouches for the
//!    layout. Otherwise the fingerprints must match the host's own
//!    constants too. Only then may [`PluginDeclaration::register`] be
//!    called. The plugin registers each lint through the host's
//!    [`Registrar`], and the host keeps a [`RegisteredLint`] for each.
//! 4. The registration names what the plugin uses: the length of each
//!    [`HostTable`] it was built against, and the decorations each lint
//!    reads. The host refuses a table longer than its own, or one left
//!    unnamed. The loader refuses a decoration that no provider offers at
//!    the plugin's layout. Each refusal names
//!    the item, so a newer plugin on an older stiles says what it needs.
//!
//! The side that allocated a value is the side that frees it, whatever the
//! allocator of the other side is. Every stabby allocation that crosses
//! carries the function that frees it. A pass's state goes back to the
//! plugin's own `drop` entry, which frees it in the plugin's image. A
//! plugin may therefore set its own `#[global_allocator]`.
//!
//! The handshake compares how both sides lay out the boundary. It does not
//! compare the source they compiled, the compiler, or the dependency graph
//! that each side resolved. Layout is what unsoundness turns on, and it is far
//! narrower than source text. A doc comment or a private helper moves
//! nothing, so a plugin stays loadable across most of stiles's own churn.
//! A plugin's lockfile picks its own `tree-sitter`, and that choice does
//! not reach the host's tree. A node carries tree-sitter's bytes for the
//! host and a table of the host's functions, and only those functions
//! read the bytes.

use stabby::IStable;

use crate::decoration_entry::Entry;
use crate::{
    DecoratedNode, DecorationIdentity, DecorationKey, FilePath, Panic, Position, RuleId,
    RuleOption, RuleOptions, Sink, Span,
};

mod abi_version;
mod declaration;
mod fingerprint;
mod host_table;
mod loaded_pass;
mod pass_instance;
mod pass_table;
mod provenance;
mod reads;
mod registered_lint;
mod registrar;
mod registrations;

pub use self::abi_version::AbiVersion;
pub use self::declaration::PluginDeclaration;
pub use self::fingerprint::{decoration_fingerprint, stable_fingerprint};
pub use self::host_table::HostTable;
pub use self::loaded_pass::LoadedPass;
pub use self::pass_instance::{Built, PassInstance};
pub use self::provenance::Provenance;
pub use self::reads::Reads;
pub use self::registered_lint::RegisteredLint;
pub use self::registrar::{Registered, Registrar};
pub use self::registrations::{RegisterLintsError, Registrations};

/// The version of the plugin declaration protocol this crate speaks
///
/// This guards what no fingerprint can read back. That is the shape of
/// [`PluginDeclaration`] and the meaning of its fields. It is also the
/// entries of every table: the [`Registrar`], the table of a pass, and the
/// tables behind a node and a sink. The two fingerprints guard everything
/// else.
///
/// The version is a promise about a release. It rises once between two
/// releases, when the boundary moved since the last one.
/// `rakko check-abi` refuses a moved boundary that kept the version of
/// that release. A build from unpublished source does not claim the
/// version alone, as [`Provenance`] says. [`AbiVersion`] says what a
/// version looks like in each era.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{ABI_VERSION, AbiVersion};
///
/// assert_eq!(ABI_VERSION, AbiVersion { major: 0, minor: 12 });
/// ```
pub const ABI_VERSION: AbiVersion = AbiVersion {
    major: 0,
    minor: 12,
};

/// Whether this crate is a build of published source
///
/// The build script sets the flag when cargo packaged this crate.
/// [`Provenance`] says what the value licenses.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{PROVENANCE, Provenance};
///
/// let joined = Provenance::all(&[PROVENANCE]);
///
/// assert_eq!(joined, PROVENANCE);
/// ```
#[cfg(stiles_published)]
pub const PROVENANCE: Provenance = Provenance::PUBLISHED;

/// Whether this crate is a build of published source
///
/// The build script sets the flag when cargo packaged this crate.
/// [`Provenance`] says what the value licenses.
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::{PROVENANCE, Provenance};
///
/// let joined = Provenance::all(&[PROVENANCE]);
///
/// assert_eq!(joined, PROVENANCE);
/// ```
#[cfg(not(stiles_published))]
pub const PROVENANCE: Provenance = Provenance::UNPUBLISHED;

/// A fingerprint of how this crate lays out the plugin boundary
///
/// The crate version cannot detect drift, because stiles's crates are
/// unpublished and hold one version between releases. A hash of the source
/// text detects far too much of it. A doc comment or a private helper would
/// refuse every plugin in the tree until each was rebuilt. That is the
/// whole cost of rules that ship as plugins. This hashes what the two
/// images must actually agree on instead.
///
/// Stabby lays out every type that crosses, and each contributes the
/// identity stabby derives from its report. That is a hash over the
/// type's name, its module, and the name and type of every field,
/// recursively. It refuses a field that moved, and a field whose type
/// changed to another of the same size.
///
/// The list names every value that crosses, and nothing else. A pass
/// receives a [`DecoratedNode`], a [`Sink`], and the [`RuleOptions`] a
/// project set. The node's table hands back a [`FilePath`], a
/// [`Position`], and the entry of a decoration by [`DecorationKey`]. An
/// entry is hidden, but its layout crosses, so it is on the list. Every
/// entry of a pass's table and every factory returns nothing or the
/// [`Panic`] that stopped it. A plugin registers each lint's rules as
/// [`RuleId`] values, and each decoration it reads as a
/// [`DecorationIdentity`]. A [`HostTable`] crosses as a byte, so it is not
/// on the list.
///
/// A pass reports each diagnostic through the sink's table, one part for
/// each call. The calls take a [`RuleId`] and a [`Span`] by reference, so
/// those two are on the list. A [`Severity`] crosses as a fixed byte, as a
/// [`Language`] does, so neither is on the list. The
/// diagnostic, its fix, its edits, and its locations do not cross. Each
/// side lays them out its own way, so they are not here.
///
/// A stabby result is not on the list, and neither is a stabby list. Both
/// are stabby's own layouts, which the stabby every plugin shares
/// decides, so the list names their payloads instead. The tree, the
/// decoration map, the collector, and the coverage types stay on the
/// host's side of the boundary, and so do the registrations.
/// A language crosses as a byte, so [`Language`] is not here either.
///
/// The tables are not on the list. The [`Registrar`], the table of a
/// pass, and the tables behind the node and the sink are `#[repr(C)]`
/// structs, not stabby types. Stabby cannot describe a function whose
/// arguments borrow for a lifetime of its own. A
/// [`PassInstance`] holds its table as a raw pointer, and the identities
/// of the node and the sink record each table only as a reference. A
/// change to a table's entries therefore belongs in [`ABI_VERSION`]. A
/// test in this module scans each table's source and fails when its
/// entries move, so the bump is not left to memory.
///
/// [`Language`]: crate::Language
/// [`Severity`]: crate::Severity
///
/// # Examples
///
/// ```
/// use stiles_types::plugin::TYPES_FINGERPRINT;
///
/// assert_ne!(TYPES_FINGERPRINT, 0);
/// ```
pub const TYPES_FINGERPRINT: u64 = stable_fingerprint(&[
    <DecoratedNode<'static> as IStable>::ID,
    <DecorationKey as IStable>::ID,
    <DecorationIdentity as IStable>::ID,
    <Entry as IStable>::ID,
    <FilePath as IStable>::ID,
    <RuleOptions as IStable>::ID,
    <RuleOption as IStable>::ID,
    <Panic as IStable>::ID,
    <Sink<'static> as IStable>::ID,
    <Span as IStable>::ID,
    <Position as IStable>::ID,
    <RuleId as IStable>::ID,
]);

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns each declaration of a table of functions, in source order
    ///
    /// The scan takes the function types and the struct of `source`, up to
    /// the table's first `impl` block. A source that declares no function
    /// type starts at its struct. The scan drops the doc comments and the
    /// attributes, and squeezes the whitespace. Each type comes out whole,
    /// and the struct comes out last.
    fn table_declarations(source: &str) -> Vec<String> {
        let start = source
            .find("pub(crate) type ")
            .or_else(|| source.find("#[repr(C)]"))
            .expect("the table should be declared in this source");
        let end = source
            .find("\nimpl ")
            .expect("the table should have an impl block");

        let code = source[start..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("///") && !line.starts_with("#["))
            .collect::<Vec<_>>()
            .join(" ");
        let code = code
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("( ", "(")
            .replace(", )", ")");

        code.split_inclusive(';')
            .map(|declaration| declaration.trim().to_owned())
            .collect()
    }

    #[test]
    fn abi_version_covers_the_node_table() {
        let context = table_declarations(include_str!("context.rs"));

        assert_eq!(
            context,
            [
                "pub(crate) type CountFn = extern \"C\" fn(&Context, &TsNode) -> usize;",
                "pub(crate) type TextFn = for<'a> extern \"C\" fn(&'a Context, &TsNode) -> Str<'a>;",
                "pub(crate) type PositionFn = extern \"C\" fn(&Context, &TsNode) -> Position;",
                "pub(crate) type FlagFn = extern \"C\" fn(&Context, &TsNode) -> bool;",
                "pub(crate) type StepFn = extern \"C\" fn(&Context, &TsNode, &mut MaybeUninit<TsNode>) -> bool;",
                "pub(crate) type ChildFn = extern \"C\" fn(&Context, &TsNode, u32, &mut MaybeUninit<TsNode>) -> bool;",
                "pub(crate) type FieldFn = for<'n> extern \"C\" fn(&Context, &TsNode, Str<'n>, &mut MaybeUninit<TsNode>) -> bool;",
                "pub(crate) type FileFn = for<'a> extern \"C\" fn(&'a Context) -> &'a FilePath;",
                "pub(crate) type DecorationFn = for<'a> extern \"C\" fn(&'a Context, &TsNode, DecorationKey, usize) -> Option<&'a Entry>;",
                "pub(crate) type ParseFn = for<'a, 'n> extern \"C\" fn(&'a Context, u8, Str<'n>, &mut MaybeUninit<DecoratedNode<'a>>) -> bool;",
                "pub struct Context { pub(crate) length: usize, pub(crate) data: *const c_void, \
                 pub(crate) kind: TextFn, pub(crate) text: TextFn, pub(crate) id: CountFn, \
                 pub(crate) start_byte: CountFn, pub(crate) end_byte: CountFn, \
                 pub(crate) start_position: PositionFn, pub(crate) end_position: PositionFn, \
                 pub(crate) file: FileFn, pub(crate) is_named: FlagFn, pub(crate) has_error: FlagFn, \
                 pub(crate) child_count: CountFn, pub(crate) named_child_count: CountFn, \
                 pub(crate) child: ChildFn, pub(crate) named_child: ChildFn, \
                 pub(crate) child_by_field_name: FieldFn, pub(crate) parent: StepFn, \
                 pub(crate) next_sibling: StepFn, pub(crate) prev_sibling: StepFn, \
                 pub(crate) next_named_sibling: StepFn, pub(crate) prev_named_sibling: StepFn, \
                 pub(crate) decoration: DecorationFn, pub(crate) parse: ParseFn, }",
            ],
            "the node's table moved, which no fingerprint reads; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn abi_version_covers_the_pass_instance() {
        let instance = table_declarations(include_str!("plugin/pass_instance.rs"));

        assert_eq!(
            instance,
            ["pub struct PassInstance { data: *mut c_void, table: *const PassTable, }"],
            "a pass's instance moved, which no fingerprint reads; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn abi_version_covers_the_pass_table() {
        let pass = table_declarations(include_str!("plugin/pass_table.rs"));

        assert_eq!(
            pass,
            [
                "pub(crate) type DropFn = extern \"C\" fn(*mut c_void);",
                "pub(crate) type ConfigureFn = extern \"C\" fn(*mut c_void, &RuleOptions) -> Configured;",
                "pub(crate) type CheckNodeFn = extern \"C\" fn(*mut c_void, &DecoratedNode<'_>, &Sink<'_>) -> Checked;",
                "pub struct PassTable { pub(crate) length: usize, pub(crate) drop: DropFn, \
                 pub(crate) configure: Option<ConfigureFn>, \
                 pub(crate) check_node: Option<CheckNodeFn>, }",
            ],
            "a pass's table moved, which no fingerprint reads; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn abi_version_covers_the_registrar() {
        let registrar = table_declarations(include_str!("plugin/registrar.rs"));

        assert_eq!(
            registrar,
            [
                "pub(crate) type BuildFn = extern \"C\" fn(&mut MaybeUninit<PassInstance>) -> Built;",
                "pub(crate) type LintFn = extern \"C\" fn(&Registrar, u8, BuildFn, *const RuleId, usize) -> usize;",
                "pub(crate) type ReadsFn = extern \"C\" fn(&Registrar, usize, *const DecorationIdentity, usize);",
                "pub(crate) type RequiresFn = extern \"C\" fn(&Registrar, u8, usize);",
                "pub struct Registrar { pub(crate) length: usize, pub(crate) data: *const c_void, \
                 pub(crate) lint: LintFn, pub(crate) reads: ReadsFn, \
                 pub(crate) requires: RequiresFn, }",
            ],
            "the registrar moved, which no fingerprint reads; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn abi_version_covers_the_sink_table() {
        let sink = table_declarations(include_str!("sink_table.rs"));

        assert_eq!(
            sink,
            [
                "pub(crate) type ReportFn = for<'n> extern \"C\" fn(&SinkTable, &RuleId, u8, Str<'n>, &Span) -> usize;",
                "pub(crate) type AttachFn = for<'n> extern \"C\" fn(&SinkTable, usize, &Span, Str<'n>);",
                "pub(crate) type NameFn = for<'n> extern \"C\" fn(&SinkTable, usize, Str<'n>);",
                "pub struct SinkTable { pub(crate) length: usize, pub(crate) data: *const c_void, \
                 pub(crate) report: ReportFn, pub(crate) origin: AttachFn, \
                 pub(crate) related: AttachFn, pub(crate) fix_message: NameFn, \
                 pub(crate) edit: AttachFn, }",
            ],
            "the sink's table moved, which no fingerprint reads; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn types_fingerprint_covers_every_boundary_type() {
        let one = stable_fingerprint(&[<Span as IStable>::ID]);

        let all = TYPES_FINGERPRINT;

        assert_ne!(all, one);
    }

    #[test]
    fn types_fingerprint_is_not_zero() {
        let all = TYPES_FINGERPRINT;

        assert_ne!(all, 0);
    }
}
