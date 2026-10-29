use bon::bon;
use stabby::option;
use stiles_macros::Decoration;

use crate::decorations::{ErrorType, ResolvedType, ReturnMode};

/// Function signature information
///
/// Attached to `function_item` nodes, and to the `function_signature_item`
/// of a declaration without a body. Lints can then inspect the return type
/// without a new query to the semantic model. Every resolved function gets a
/// signature, even an empty one. An absent decoration would look like a
/// function the provider never reached, and those are different facts.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out. Its optional parts are stabby's [`Option`], because std promises
/// no layout for an `Option` of these types.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{ErrorType, FnSignature, ResolvedType, ResultKind, ReturnMode};
///
/// let signature = FnSignature::builder()
///     .return_type(ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result))
///     .error_type(ErrorType::Generic)
///     .return_mode(ReturnMode::Direct)
///     .build();
///
/// assert_eq!(signature.return_mode(), ReturnMode::Direct);
/// ```
///
/// [`Option`]: stabby::option::Option
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct FnSignature {
    return_type: option::Option<ResolvedType>,
    error_type: option::Option<ErrorType>,
    return_mode: ReturnMode,
}

#[bon]
impl FnSignature {
    /// Creates a function signature
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{FnSignature, ReturnMode};
    ///
    /// let signature = FnSignature::builder().return_mode(ReturnMode::Opaque).build();
    ///
    /// assert!(signature.return_type().is_none());
    /// ```
    #[builder]
    pub fn new(
        return_type: Option<ResolvedType>,
        error_type: Option<ErrorType>,
        return_mode: ReturnMode,
    ) -> Self {
        Self {
            return_type: return_type.into(),
            error_type: error_type.into(),
            return_mode,
        }
    }

    /// Returns the resolved return type, awaited for an `async fn`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{ErrorType, FnSignature, ResolvedType, ResultKind, ReturnMode};
    ///
    /// let signature = FnSignature::builder()
    ///     .return_type(
    ///         ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result),
    ///     )
    ///     .error_type(ErrorType::Generic)
    ///     .return_mode(ReturnMode::Direct)
    ///     .build();
    ///
    /// assert_eq!(signature.return_type().map(ResolvedType::display), Some("Result<(), Error>"));
    /// ```
    pub fn return_type(&self) -> Option<&ResolvedType> {
        self.return_type.as_ref()
    }

    /// Returns the `E` of the `Result<T, E>` this function returns
    ///
    /// [`None`] means the function does not return a `core::result::Result`.
    /// That includes an `async fn` whose awaited type is not one, and one
    /// whose future could not be projected.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{ErrorType, FnSignature, ResolvedType, ResultKind, ReturnMode};
    ///
    /// let signature = FnSignature::builder()
    ///     .return_type(
    ///         ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result),
    ///     )
    ///     .error_type(ErrorType::Generic)
    ///     .return_mode(ReturnMode::Direct)
    ///     .build();
    ///
    /// assert_eq!(signature.error_type(), Some(&ErrorType::Generic));
    /// ```
    pub fn error_type(&self) -> Option<&ErrorType> {
        self.error_type.as_ref()
    }

    /// Returns where the return type was read from
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{ErrorType, FnSignature, ResolvedType, ResultKind, ReturnMode};
    ///
    /// let signature = FnSignature::builder()
    ///     .return_type(
    ///         ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result),
    ///     )
    ///     .error_type(ErrorType::Generic)
    ///     .return_mode(ReturnMode::Direct)
    ///     .build();
    ///
    /// assert_eq!(signature.return_mode(), ReturnMode::Direct);
    /// ```
    pub fn return_mode(&self) -> ReturnMode {
        self.return_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decorations::{ResultKind, TypePath, TypePathRef};

    /// Returns the signature of a function that returns
    /// `Result<(), anyhow::Error>` directly
    fn returning_a_result() -> FnSignature {
        let ret = ResolvedType::new("Result<(), anyhow::Error>".into())
            .with_result_kind(ResultKind::Result);
        let error = ErrorType::Named(
            TypePath::builder()
                .krate("anyhow")
                .modules([] as [&str; 0])
                .name("Error")
                .build(),
        );

        FnSignature::builder()
            .return_type(ret)
            .error_type(error)
            .return_mode(ReturnMode::Direct)
            .build()
    }

    #[test]
    fn fn_signature_with_return_type_keeps_the_error_type() {
        let sig = returning_a_result();

        let error = sig.error_type().expect("the error type should be kept");

        assert!(
            error.is(TypePathRef::builder()
                .krate("anyhow")
                .modules(&[])
                .name("Error")
                .build())
        );
    }

    #[test]
    fn fn_signature_with_return_type_keeps_the_return_mode() {
        let sig = returning_a_result();

        assert_eq!(sig.return_mode(), ReturnMode::Direct);
    }

    #[test]
    fn fn_signature_with_return_type_keeps_the_return_type() {
        let sig = returning_a_result();

        let ret = sig.return_type().expect("the return type should be kept");

        assert!(ret.is_result());
    }

    #[test]
    fn fn_signature_without_return_type_has_no_error_type() {
        let sig = FnSignature::builder()
            .return_mode(ReturnMode::Opaque)
            .build();

        assert!(sig.error_type().is_none());
    }

    #[test]
    fn fn_signature_without_return_type_has_no_return_type() {
        let sig = FnSignature::builder()
            .return_mode(ReturnMode::Opaque)
            .build();

        assert!(sig.return_type().is_none());
    }

    #[test]
    fn fn_signature_without_return_type_keeps_the_return_mode() {
        let sig = FnSignature::builder()
            .return_mode(ReturnMode::Opaque)
            .build();

        assert_eq!(sig.return_mode(), ReturnMode::Opaque);
    }

    #[test]
    fn trait_send_fn_signature() {
        fn assert_send<T: Send>() {}
        assert_send::<FnSignature>();
    }

    #[test]
    fn trait_sync_fn_signature() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FnSignature>();
    }

    #[test]
    fn trait_unpin_fn_signature() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FnSignature>();
    }
}
