//! Vocabulary for custom lint plugins
//!
//! A custom lint plugin is a dynamic library that stiles compiles and
//! loads at check time, given only a path. Rust has no stable ABI of its
//! own, so stabby lays out every type that crosses the boundary. Those
//! layouts hold under any compiler. A loaded library is coherent with the
//! stiles binary when both lay those types out the same way. That is a
//! property of the source each was built from, not of the rustc that
//! built it. This module defines the declaration a plugin exports and the
//! constants stiles compares to establish exactly that before it trusts
//! anything else in the library.
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
//!    constants too. Only then may [`PluginDeclaration::load`] be called.
//!    What it hands back is only meaningful once the two images are known
//!    to lay it out the same way.
//!
//! Stabby lays out every value that crosses the boundary. Every allocation
//! among them carries the function that frees it. The side that allocated
//! a value is the side that frees it, whatever the allocator of
//! the other side is. A plugin may therefore set its own
//! `#[global_allocator]`.
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
    BoxedLintPass, DecoratedNode, DecorationKey, Diagnostic, Edit, FilePath, Fix, Language,
    Location, Panic, Position, RuleId, RuleOption, RuleOptions, Severity, Span,
};

mod abi_version;
mod declaration;
mod factory;
mod fingerprint;
mod provenance;

pub use self::abi_version::AbiVersion;
pub use self::declaration::PluginDeclaration;
pub use self::factory::{
    Constructed, ExportedLint, LintPassFactory, Loaded, Plugin, construct, factory,
};
pub use self::fingerprint::{decoration_fingerprint, stable_fingerprint};
pub use self::provenance::Provenance;

/// The version of the plugin declaration protocol this crate speaks
///
/// This guards what no fingerprint can read back. That is the shape of
/// [`PluginDeclaration`], the meaning of its fields, the signatures of
/// [`LintPass`]'s methods, and the entries of the node's table. The two
/// fingerprints guard everything else.
///
/// The version is a promise about a release. It rises once between two
/// releases, when the boundary moved since the last one.
/// `rakko check-abi` refuses a moved boundary that kept the version of
/// that release. A build from unpublished source does not claim the
/// version alone, as [`Provenance`] says. [`AbiVersion`] says what a
/// version looks like in each era.
///
/// [`LintPass`]: crate::LintPass
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
/// receives a [`DecoratedNode`] and the [`RuleOptions`] a project set. The
/// node's table hands back a [`FilePath`], a [`Position`], and the entry of
/// a decoration by [`DecorationKey`]. An entry is hidden, but its layout
/// crosses, so it is on the list. A pass names its [`Language`], and returns
/// [`Diagnostic`]s or the [`Panic`] that stopped it. A plugin hands its
/// exports over as a [`Plugin`], and each factory builds a
/// [`BoxedLintPass`].
///
/// A stabby result is not on the list, and neither is a stabby list. Both
/// are stabby's own layouts, which the stabby every plugin shares
/// decides, so the list names their payloads instead. The tree, the
/// decoration map, and the coverage types stay on the host's side of the
/// boundary, so they are not here.
///
/// The node's table of host functions is not on the list either. It is a
/// `#[repr(C)]` struct, not a stabby type, because stabby cannot describe
/// a function whose arguments borrow for a lifetime of its own. The
/// node's identity records the table only as a reference. A change to the
/// table's entries therefore belongs in [`ABI_VERSION`], as a change to a
/// signature of [`LintPass`] does.
///
/// What it does not cover is the signatures of [`LintPass`]'s methods.
/// The identity of the factory reaches the vtable and names its methods in
/// order, so a method added, removed, or moved is refused. The report
/// records each method as a pointer, though, and not what it takes and
/// returns. A
/// changed signature is a change to the protocol and belongs in
/// [`ABI_VERSION`]. The `abi_version_covers_the_boundary_traits` test in
/// this module fails when the trait's method list moves, so the bump is
/// not left to memory.
///
/// [`LintPass`]: crate::LintPass
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
    <Entry as IStable>::ID,
    <FilePath as IStable>::ID,
    <Language as IStable>::ID,
    <RuleOptions as IStable>::ID,
    <RuleOption as IStable>::ID,
    <Panic as IStable>::ID,
    <LintPassFactory as IStable>::ID,
    <Plugin as IStable>::ID,
    <ExportedLint as IStable>::ID,
    <BoxedLintPass as IStable>::ID,
    <Diagnostic as IStable>::ID,
    <Span as IStable>::ID,
    <Fix as IStable>::ID,
    <Edit as IStable>::ID,
    <Location as IStable>::ID,
    <Position as IStable>::ID,
    <RuleId as IStable>::ID,
    <Severity as IStable>::ID,
]);

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns each method signature `LintPass` declares, in declaration order
    ///
    /// The scan takes the lines inside the block of the trait that open a
    /// method, and squeezes their whitespace. A doc comment or a reflowed
    /// line therefore changes nothing. An added, removed, reordered, or
    /// re-signed method does.
    fn lint_pass_methods() -> Vec<String> {
        let body = include_str!("lint_pass.rs")
            .split_once("pub trait LintPass: Send + Sync {")
            .expect("the trait should be declared in this source")
            .1;
        let body = body
            .split_once("\n}")
            .expect("the trait block should be closed")
            .0;

        body.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("fn ") || line.starts_with("extern \"C\" fn "))
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    }

    /// Returns each declaration of the node's table, in source order
    ///
    /// The scan takes the function types and the struct of `context.rs`,
    /// drops the doc comments and the attributes, and squeezes the
    /// whitespace. Each type comes out whole, and the struct comes out last.
    fn context_declarations() -> Vec<String> {
        let source = include_str!("context.rs");
        let start = source
            .find("pub(crate) type ")
            .expect("the table's function types should be declared in this source");
        let end = source
            .find("impl Context {")
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
    fn abi_version_covers_the_boundary_traits() {
        let lint_pass = lint_pass_methods();

        assert_eq!(
            lint_pass,
            vec![
                "extern \"C\" fn language(&self) -> Language;".to_owned(),
                "extern \"C\" fn configure(&mut self, options: &RuleOptions) -> Configured;"
                    .to_owned(),
                "extern \"C\" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked;"
                    .to_owned()
            ],
            "the boundary trait's methods moved, which changes its vtable; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn abi_version_covers_the_node_table() {
        let context = context_declarations();

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
    fn types_fingerprint_covers_every_boundary_type() {
        let one = stable_fingerprint(&[<Diagnostic as IStable>::ID]);

        let all = TYPES_FINGERPRINT;

        assert_ne!(all, one);
    }

    #[test]
    fn types_fingerprint_is_not_zero() {
        let all = TYPES_FINGERPRINT;

        assert_ne!(all, 0);
    }
}
