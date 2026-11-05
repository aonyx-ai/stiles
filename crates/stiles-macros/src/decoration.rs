use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Fields, LitStr, Result};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// How many times a provider may record a decoration on a single node
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Cardinality {
    Many,
    One,
}

/// Builds the `Decoration` implementation for a derive input
///
/// A key should name exactly one type definition. The derive therefore
/// builds it from the path of the defining module, the name of the type,
/// and [`definition_hash`]. It rejects generic types, because their single
/// key would have to cover one layout per instantiation. A key that names
/// another type is a wrong answer rather than a wrong read. Stabby checks
/// the stored type against the requested one before it casts.
///
/// # Errors
///
/// Returns an error if the type is generic, or if the `decoration`
/// attribute is missing. Returns an error too if the attribute names an
/// option other than `cardinality`, or gives a cardinality other than `one`
/// or `many`. A cardinality declared more than once is an error as well.
pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    if let Some(parameter) = input.generics.params.first() {
        return Err(Error::new_spanned(
            parameter,
            "a decoration cannot be generic, because its key must name exactly one type",
        ));
    }

    let cardinality = cardinality(input)?;
    let name = &input.ident;
    let hash = definition_hash(input);
    let where_clause = &input.generics.where_clause;

    let (reference, lookup) = match cardinality {
        Cardinality::Many => (
            quote!(::std::vec::Vec<&'decoration Self>),
            quote!(node.decorations_of_type::<Self>()),
        ),
        Cardinality::One => (
            quote!(::core::option::Option<&'decoration Self>),
            quote!(node.decoration::<Self>()),
        ),
    };

    Ok(quote! {
        #[automatically_derived]
        impl ::stiles_types::Decoration for #name #where_clause {
            const KEY: ::stiles_types::DecorationKey = ::stiles_types::DecorationKey::new(
                ::core::concat!(
                    ::core::module_path!(),
                    "::",
                    ::core::stringify!(#name),
                    "#",
                    #hash,
                ),
            );

            type Ref<'decoration> = #reference;

            fn lookup<'decoration>(
                node: &::stiles_types::DecoratedNode<'decoration>,
            ) -> Self::Ref<'decoration> {
                #lookup
            }
        }
    })
}

/// Hashes a type's definition into sixteen hexadecimal digits
///
/// [`module_path!`] stops at module boundaries, so two types of one name
/// in two function bodies of the same module share a path and a name.
/// Their definitions still differ, and folding those into the key keeps
/// the two apart. The hash covers the item as written, so two images that
/// compile one source derive the same digits. That is what lets a key
/// travel across the plugin boundary at all.
///
/// The hash leaves out the documentation, which [`without_docs`] removes.
/// A doc comment moves no field, so it moves no fingerprint either, and the
/// plugin handshake accepts a plugin built before the edit. A key that
/// followed the prose would make every lookup of that plugin miss, and its
/// rules would stop reporting without an error.
///
/// Two definitions in two scopes of one module that differ only in their
/// documentation remain indistinguishable. They describe the same layout,
/// so reading one as the other cannot misread a field. The plugin
/// handshake already rests on one compiler that lays identical definitions
/// out identically.
///
/// [`module_path!`]: std::module_path
fn definition_hash(input: &DeriveInput) -> String {
    let input = without_docs(input);
    let mut hash = FNV_OFFSET_BASIS;

    for byte in quote!(#input).to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:016x}")
}

/// Returns a copy of a definition without its `doc` attributes
///
/// The copy loses every `doc` attribute: on the type, on each variant, and
/// on each field. That covers the `#[doc = "..."]` that a `///` comment
/// becomes, and an explicit `#[doc(...)]` such as `#[doc(hidden)]`. Both
/// only feed rustdoc. Every other attribute stays, because one such as
/// `repr` or `decoration` can change what the type means, and the key must
/// follow every such change. The compiler expands a `cfg_attr` before a
/// derive sees the item. A `doc` behind a `cfg_attr` therefore arrives here
/// as a plain `doc` attribute and leaves too.
fn without_docs(input: &DeriveInput) -> DeriveInput {
    let mut input = input.clone();
    strip_docs(&mut input.attrs);

    match &mut input.data {
        Data::Struct(data) => strip_field_docs(&mut data.fields),
        Data::Enum(data) => {
            for variant in &mut data.variants {
                strip_docs(&mut variant.attrs);
                strip_field_docs(&mut variant.fields);
            }
        }
        Data::Union(data) => {
            for field in &mut data.fields.named {
                strip_docs(&mut field.attrs);
            }
        }
    }

    input
}

/// Removes the `doc` attributes of every field in a list of fields
fn strip_field_docs(fields: &mut Fields) {
    for field in fields.iter_mut() {
        strip_docs(&mut field.attrs);
    }
}

/// Removes every `doc` attribute from a list of attributes
fn strip_docs(attrs: &mut Vec<Attribute>) {
    attrs.retain(|attr| !attr.path().is_ident("doc"));
}

/// Reads the cardinality declared by the type's `decoration` attribute
fn cardinality(input: &DeriveInput) -> Result<Cardinality> {
    let mut declared: Option<Cardinality> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("decoration") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("cardinality") {
                return Err(meta.error("expected `cardinality`"));
            }

            let value: LitStr = meta.value()?.parse()?;
            let parsed = match value.value().as_str() {
                "many" => Cardinality::Many,
                "one" => Cardinality::One,
                other => {
                    return Err(Error::new(
                        value.span(),
                        format!("expected `one` or `many`, found `{other}`"),
                    ));
                }
            };

            if declared.replace(parsed).is_some() {
                return Err(Error::new(value.span(), "cardinality declared twice"));
            }

            Ok(())
        })?;
    }

    declared.ok_or_else(|| {
        Error::new_spanned(
            &input.ident,
            "missing `#[decoration(cardinality = \"one\")]` or \
             `#[decoration(cardinality = \"many\")]`",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> DeriveInput {
        syn::parse_str(source).expect("should parse as a derive input")
    }

    #[test]
    fn definition_hash_ignores_doc_comments_on_the_type_and_fields() {
        let plain = parse(r#"#[decoration(cardinality = "one")] struct Flag { count: u32, }"#);
        let documented = parse(
            r#"
            /// Marks a node
            #[decoration(cardinality = "one")]
            struct Flag {
                /// How often
                count: u32,
            }
            "#,
        );

        let documented = definition_hash(&documented);

        assert_eq!(documented, definition_hash(&plain));
    }

    #[test]
    fn definition_hash_ignores_doc_comments_on_variants_and_their_fields() {
        let plain =
            parse(r#"#[decoration(cardinality = "one")] enum Flag { On { level: u8, }, Off, }"#);
        let documented = parse(
            r#"
            #[decoration(cardinality = "one")]
            enum Flag {
                /// Switched on
                On {
                    /// How far
                    level: u8,
                },
                /// Switched off
                Off,
            }
            "#,
        );

        let documented = definition_hash(&documented);

        assert_eq!(documented, definition_hash(&plain));
    }

    #[test]
    fn definition_hash_ignores_explicit_doc_attributes() {
        let plain = parse(r#"#[decoration(cardinality = "one")] struct Flag(u32);"#);
        let documented = parse(
            r#"#[doc = "Marks a node"] #[doc(hidden)] #[decoration(cardinality = "one")] struct Flag(#[doc(alias = "n")] u32);"#,
        );

        let documented = definition_hash(&documented);

        assert_eq!(documented, definition_hash(&plain));
    }

    #[test]
    fn definition_hash_with_another_attribute_differs() {
        let plain = parse(r#"#[decoration(cardinality = "one")] enum Flag { On, Off }"#);
        let represented =
            parse(r#"#[decoration(cardinality = "one")] #[repr(u8)] enum Flag { On, Off }"#);

        let represented = definition_hash(&represented);

        assert_ne!(represented, definition_hash(&plain));
    }

    #[test]
    fn definition_hash_with_another_field_type_differs() {
        let plain = parse(
            r#"#[doc = "Counts"] #[decoration(cardinality = "one")] struct Flag { count: u32 }"#,
        );
        let widened = parse(
            r#"#[doc = "Counts"] #[decoration(cardinality = "one")] struct Flag { count: u64 }"#,
        );

        let widened = definition_hash(&widened);

        assert_ne!(widened, definition_hash(&plain));
    }

    #[test]
    fn expand_builds_key_from_module_path_name_and_definition() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        let definition = format!("{:?}", definition_hash(&input));
        let missing: Vec<&str> = [
            "const KEY : :: stiles_types :: DecorationKey",
            ":: core :: module_path ! ()",
            ":: core :: stringify ! (Flag)",
            definition.as_str(),
        ]
        .into_iter()
        .filter(|part| !code.contains(part))
        .collect();
        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn expand_emits_an_unsafe_impl() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("impl :: stiles_types :: Decoration for Flag"));
    }

    #[test]
    fn expand_gives_one_definition_the_same_key_twice() {
        let source = r#"#[decoration(cardinality = "one")] struct Flag(u32);"#;

        let first = expand(&parse(source)).expect("should expand").to_string();
        let second = expand(&parse(source)).expect("should expand").to_string();

        assert_eq!(first, second);
    }

    #[test]
    fn expand_gives_same_named_types_of_different_shape_different_keys() {
        let counted = parse(r#"#[decoration(cardinality = "one")] struct Flag(u32);"#);
        let named = parse(r#"#[decoration(cardinality = "one")] struct Flag(String);"#);

        let counted = definition_hash(&counted);
        let named = definition_hash(&named);

        assert_ne!(counted, named);
    }

    #[test]
    fn expand_with_generic_type_returns_error() {
        let input =
            parse(r#"#[decoration(cardinality = "one")] struct Wrapper<T>(T) where T: Clone;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("cannot be generic"));
    }

    #[test]
    fn expand_with_lifetime_parameter_returns_error() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Borrowed<'a>(&'a str);"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("cannot be generic"));
    }

    #[test]
    fn expand_with_many_cardinality_looks_up_every_instance() {
        let input = parse(r#"#[decoration(cardinality = "many")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("decorations_of_type :: < Self > ()"));
    }

    #[test]
    fn expand_with_many_cardinality_returns_a_vec() {
        let input = parse(r#"#[decoration(cardinality = "many")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains(":: std :: vec :: Vec < & 'decoration Self >"));
    }

    #[test]
    fn expand_with_one_cardinality_looks_up_one_instance() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("decoration :: < Self > ()"));
    }

    #[test]
    fn expand_with_one_cardinality_returns_an_option() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains(":: core :: option :: Option < & 'decoration Self >"));
    }

    #[test]
    fn expand_with_repeated_cardinality_returns_error() {
        let input =
            parse(r#"#[decoration(cardinality = "one", cardinality = "many")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("declared twice"));
    }

    #[test]
    fn expand_with_unknown_cardinality_returns_error() {
        let input = parse(r#"#[decoration(cardinality = "several")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("expected `one` or `many`"));
    }

    #[test]
    fn expand_with_unknown_option_returns_error() {
        let input = parse(r#"#[decoration(shape = "one")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("expected `cardinality`"));
    }

    #[test]
    fn expand_without_attribute_returns_error() {
        let input = parse("struct Flag;");

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn trait_send_cardinality() {
        fn assert_send<T: Send>() {}
        assert_send::<Cardinality>();
    }

    #[test]
    fn trait_sync_cardinality() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Cardinality>();
    }

    #[test]
    fn trait_unpin_cardinality() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Cardinality>();
    }
}
