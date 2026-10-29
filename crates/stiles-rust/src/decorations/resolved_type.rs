use stabby::string;
use stiles_macros::Decoration;

use crate::decorations::{AdtKind, NeverKind, OptionKind, ResultKind};

/// Resolved type information attached to a tree-sitter node
///
/// Decoration providers populate this on match scrutinees, `else`
/// clauses, and `?` operands. Rules access it via
/// `node.decoration::<ResolvedType>()`.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out and the rendered name is stabby's `String`. Each fact about the
/// type is an enum with a byte discriminant, not a `bool`. A call site
/// that records one therefore names the answer it gives.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::{AdtKind, ResolvedType};
///
/// let ty = ResolvedType::new("MyEnum".to_string()).with_adt_kind(AdtKind::Enum);
///
/// assert!(ty.is_enum());
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct ResolvedType {
    display: string::String,
    adt_kind: AdtKind,
    never_kind: NeverKind,
    result_kind: ResultKind,
    option_kind: OptionKind,
}

impl ResolvedType {
    /// Creates a new resolved type
    ///
    /// Every kind starts as `Other`, so a type the provider says nothing
    /// more about is no enum, not `!`, not `Result`, and not `Option`.
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Vec<u8>".to_string());
    ///
    /// assert_eq!(ty.display(), "Vec<u8>");
    /// ```
    pub fn new(display: String) -> Self {
        Self {
            display: string::String::from(display.as_str()),
            adt_kind: AdtKind::Other,
            never_kind: NeverKind::Other,
            result_kind: ResultKind::Other,
            option_kind: OptionKind::Other,
        }
    }

    /// Records whether this type is an enum
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("Shape".to_string()).with_adt_kind(AdtKind::Enum);
    ///
    /// assert!(ty.is_enum());
    /// ```
    pub fn with_adt_kind(mut self, kind: AdtKind) -> Self {
        self.adt_kind = kind;
        self
    }

    /// Records whether this type is the never type (`!`)
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{NeverKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("!".to_string()).with_never_kind(NeverKind::Never);
    ///
    /// assert!(ty.is_never());
    /// ```
    pub fn with_never_kind(mut self, kind: NeverKind) -> Self {
        self.never_kind = kind;
        self
    }

    /// Records whether this type is `Result<T, E>`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{ResolvedType, ResultKind};
    ///
    /// let ty =
    ///     ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result);
    ///
    /// assert!(ty.is_result());
    /// ```
    pub fn with_result_kind(mut self, kind: ResultKind) -> Self {
        self.result_kind = kind;
        self
    }

    /// Records whether this type is `Option<T>`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{OptionKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("Option<u8>".to_string()).with_option_kind(OptionKind::Option);
    ///
    /// assert!(ty.is_option());
    /// ```
    pub fn with_option_kind(mut self, kind: OptionKind) -> Self {
        self.option_kind = kind;
        self
    }

    /// Returns the human-readable type name
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Shape".to_string());
    ///
    /// assert_eq!(ty.display(), "Shape");
    /// ```
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Returns whether this type is an enum, as a variant
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{AdtKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("Shape".to_string()).with_adt_kind(AdtKind::Enum);
    ///
    /// assert_eq!(ty.adt_kind(), AdtKind::Enum);
    /// ```
    pub fn adt_kind(&self) -> AdtKind {
        self.adt_kind
    }

    /// Returns whether this type is the never type (`!`), as a variant
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{NeverKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("!".to_string()).with_never_kind(NeverKind::Never);
    ///
    /// assert_eq!(ty.never_kind(), NeverKind::Never);
    /// ```
    pub fn never_kind(&self) -> NeverKind {
        self.never_kind
    }

    /// Returns whether this type is `Result<T, E>`, as a variant
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{ResolvedType, ResultKind};
    ///
    /// let ty =
    ///     ResolvedType::new("Result<(), Error>".to_string()).with_result_kind(ResultKind::Result);
    ///
    /// assert_eq!(ty.result_kind(), ResultKind::Result);
    /// ```
    pub fn result_kind(&self) -> ResultKind {
        self.result_kind
    }

    /// Returns whether this type is `Option<T>`, as a variant
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::{OptionKind, ResolvedType};
    ///
    /// let ty = ResolvedType::new("Option<u8>".to_string()).with_option_kind(OptionKind::Option);
    ///
    /// assert_eq!(ty.option_kind(), OptionKind::Option);
    /// ```
    pub fn option_kind(&self) -> OptionKind {
        self.option_kind
    }

    /// Returns whether this is an enum type
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Shape".to_string());
    ///
    /// assert!(!ty.is_enum());
    /// ```
    pub fn is_enum(&self) -> bool {
        match self.adt_kind {
            AdtKind::Enum => true,
            AdtKind::Other => false,
        }
    }

    /// Returns whether this is the never type (`!`)
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("!".to_string());
    ///
    /// assert!(!ty.is_never());
    /// ```
    pub fn is_never(&self) -> bool {
        match self.never_kind {
            NeverKind::Never => true,
            NeverKind::Other => false,
        }
    }

    /// Returns whether this is a `Result<T, E>` type
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Result<(), Error>".to_string());
    ///
    /// assert!(!ty.is_result());
    /// ```
    pub fn is_result(&self) -> bool {
        match self.result_kind {
            ResultKind::Result => true,
            ResultKind::Other => false,
        }
    }

    /// Returns whether this is an `Option<T>` type
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Option<u8>".to_string());
    ///
    /// assert!(!ty.is_option());
    /// ```
    pub fn is_option(&self) -> bool {
        match self.option_kind {
            OptionKind::Option => true,
            OptionKind::Other => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_type_adt_kind_returns_the_recorded_kind() {
        let ty = ResolvedType::new("T".into()).with_adt_kind(AdtKind::Enum);

        assert_eq!(ty.adt_kind(), AdtKind::Enum);
    }

    #[test]
    fn resolved_type_builder_clears_a_flag() {
        let ty = ResolvedType::new("MyEnum".into())
            .with_adt_kind(AdtKind::Enum)
            .with_never_kind(NeverKind::Other);

        assert!(!ty.is_never());
    }

    #[test]
    fn resolved_type_builder_defaults_to_false() {
        let ty = ResolvedType::new("i32".into());

        let flags = [ty.is_enum(), ty.is_never(), ty.is_result(), ty.is_option()];

        assert_eq!(flags, [false; 4]);
    }

    #[test]
    fn resolved_type_builder_sets_flags() {
        let ty = ResolvedType::new("MyEnum".into())
            .with_adt_kind(AdtKind::Enum)
            .with_never_kind(NeverKind::Other);

        assert!(ty.is_enum());
    }

    #[test]
    fn resolved_type_display_returns_the_text() {
        let ty = ResolvedType::new("i32".into());

        assert_eq!(ty.display(), "i32");
    }

    #[test]
    fn resolved_type_never_kind_returns_the_recorded_kind() {
        let ty = ResolvedType::new("!".into()).with_never_kind(NeverKind::Never);

        assert_eq!(ty.never_kind(), NeverKind::Never);
    }

    #[test]
    fn resolved_type_new_defaults_every_kind_to_other() {
        let ty = ResolvedType::new("i32".into());

        let kinds = (
            ty.adt_kind(),
            ty.never_kind(),
            ty.result_kind(),
            ty.option_kind(),
        );

        assert_eq!(
            kinds,
            (
                AdtKind::Other,
                NeverKind::Other,
                ResultKind::Other,
                OptionKind::Other
            )
        );
    }

    #[test]
    fn resolved_type_option_kind_returns_the_recorded_kind() {
        let ty = ResolvedType::new("Option<T>".into()).with_option_kind(OptionKind::Option);

        assert_eq!(ty.option_kind(), OptionKind::Option);
    }

    #[test]
    fn resolved_type_result_kind_returns_the_recorded_kind() {
        let ty = ResolvedType::new("Result<T, E>".into()).with_result_kind(ResultKind::Result);

        assert_eq!(ty.result_kind(), ResultKind::Result);
    }

    #[test]
    fn resolved_type_with_adt_kind_enum_sets_the_flag() {
        let ty = ResolvedType::new("T".into()).with_adt_kind(AdtKind::Enum);

        assert!(ty.is_enum());
    }

    #[test]
    fn resolved_type_with_adt_kind_other_clears_the_flag() {
        let ty = ResolvedType::new("T".into())
            .with_adt_kind(AdtKind::Enum)
            .with_adt_kind(AdtKind::Other);

        assert!(!ty.is_enum());
    }

    #[test]
    fn resolved_type_with_never_kind_never_sets_the_flag() {
        let ty = ResolvedType::new("T".into()).with_never_kind(NeverKind::Never);

        assert!(ty.is_never());
    }

    #[test]
    fn resolved_type_with_never_kind_other_clears_the_flag() {
        let ty = ResolvedType::new("T".into())
            .with_never_kind(NeverKind::Never)
            .with_never_kind(NeverKind::Other);

        assert!(!ty.is_never());
    }

    #[test]
    fn resolved_type_with_option_kind_option_sets_only_option() {
        let option_ty = ResolvedType::new("Option<T>".into()).with_option_kind(OptionKind::Option);

        let flags = (option_ty.is_option(), option_ty.is_result());

        assert_eq!(flags, (true, false));
    }

    #[test]
    fn resolved_type_with_option_kind_option_sets_the_flag() {
        let ty = ResolvedType::new("T".into()).with_option_kind(OptionKind::Option);

        assert!(ty.is_option());
    }

    #[test]
    fn resolved_type_with_option_kind_other_clears_the_flag() {
        let ty = ResolvedType::new("T".into())
            .with_option_kind(OptionKind::Option)
            .with_option_kind(OptionKind::Other);

        assert!(!ty.is_option());
    }

    #[test]
    fn resolved_type_with_result_kind_other_clears_the_flag() {
        let ty = ResolvedType::new("T".into())
            .with_result_kind(ResultKind::Result)
            .with_result_kind(ResultKind::Other);

        assert!(!ty.is_result());
    }

    #[test]
    fn resolved_type_with_result_kind_result_sets_only_result() {
        let result_ty =
            ResolvedType::new("Result<T, E>".into()).with_result_kind(ResultKind::Result);

        let flags = (result_ty.is_result(), result_ty.is_option());

        assert_eq!(flags, (true, false));
    }

    #[test]
    fn resolved_type_with_result_kind_result_sets_the_flag() {
        let ty = ResolvedType::new("T".into()).with_result_kind(ResultKind::Result);

        assert!(ty.is_result());
    }

    #[test]
    fn trait_send_resolved_type() {
        fn assert_send<T: Send>() {}
        assert_send::<ResolvedType>();
    }

    #[test]
    fn trait_sync_resolved_type() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ResolvedType>();
    }

    #[test]
    fn trait_unpin_resolved_type() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ResolvedType>();
    }
}
