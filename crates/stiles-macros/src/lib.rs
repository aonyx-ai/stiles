mod decoration;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Implements `Decoration` for a type, fixing how it reads back from a node
///
/// The cardinality attribute is required, and it decides the result of
/// `DecoratedNode::get` for this type. `one` yields `Option<&Self>`, for a
/// decoration that a provider records at most once per node. `many` yields
/// `Vec<&Self>`, for one that it may record repeatedly. The type declares
/// it, not each call site, so a rule cannot read a repeated decoration as
/// though it were singular.
///
/// `Decoration` is an unsafe trait: its key must name exactly one type
/// definition, because the decoration map recovers erased values by key
/// comparison. The derive meets that obligation. It builds the key from
/// the path of the defining module, the name of the type, and a hash of
/// the definition. It also rejects generic types, because their single key
/// would have to cover one layout per instantiation. Prefer the derive
/// over a manual implementation for exactly this reason.
///
/// # Examples
///
/// A plugin reads a decoration out of memory the host allocated, so
/// `Decoration` requires a layout stabby fixes. Annotate the type with
/// `#[stabby::stabby]` and give it fields stabby lays out.
///
/// ```
/// use stabby::string::String;
/// use stiles_macros::Decoration;
///
/// #[stabby::stabby]
/// #[derive(Decoration)]
/// #[decoration(cardinality = "one")]
/// pub struct ResolvedType {
///     display: String,
/// }
///
/// #[stabby::stabby]
/// #[derive(Decoration)]
/// #[decoration(cardinality = "many")]
/// pub struct TraitImpl {
///     name: String,
/// }
/// ```
#[proc_macro_derive(Decoration, attributes(decoration))]
pub fn derive_decoration(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    self::decoration::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
