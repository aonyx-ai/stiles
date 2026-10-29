use stabby::string;
use stiles_macros::Decoration;

/// Resolved type information attached to a tree-sitter node
///
/// Decoration providers populate this on match scrutinees, `else`
/// clauses, and `?` operands. Rules access it via
/// `node.decoration::<ResolvedType>()`.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out and the rendered name is stabby's `String`.
///
/// # Examples
///
/// ```
/// use stiles_rust::decorations::ResolvedType;
///
/// let ty = ResolvedType::new("MyEnum".to_string()).with_enum(true);
///
/// assert_eq!(ty.display(), "MyEnum");
/// assert!(ty.is_enum());
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct ResolvedType {
    display: string::String,
    is_enum: bool,
    is_never: bool,
    is_result: bool,
    is_option: bool,
}

impl ResolvedType {
    /// Creates a new resolved type
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
            is_enum: false,
            is_never: false,
            is_result: false,
            is_option: false,
        }
    }

    /// Marks this type as an enum
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Shape".to_string()).with_enum(true);
    ///
    /// assert!(ty.is_enum());
    /// ```
    pub fn with_enum(mut self, is_enum: bool) -> Self {
        self.is_enum = is_enum;
        self
    }

    /// Marks this type as the never type (`!`)
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("!".to_string()).with_never(true);
    ///
    /// assert!(ty.is_never());
    /// ```
    pub fn with_never(mut self, is_never: bool) -> Self {
        self.is_never = is_never;
        self
    }

    /// Marks this type as `Result<T, E>`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Result<(), Error>".to_string()).with_result(true);
    ///
    /// assert!(ty.is_result());
    /// ```
    pub fn with_result(mut self, is_result: bool) -> Self {
        self.is_result = is_result;
        self
    }

    /// Marks this type as `Option<T>`
    ///
    /// # Examples
    ///
    /// ```
    /// use stiles_rust::decorations::ResolvedType;
    ///
    /// let ty = ResolvedType::new("Option<u8>".to_string()).with_option(true);
    ///
    /// assert!(ty.is_option());
    /// ```
    pub fn with_option(mut self, is_option: bool) -> Self {
        self.is_option = is_option;
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
        self.is_enum
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
        self.is_never
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
        self.is_result
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
        self.is_option
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_type_builder_clears_a_flag() {
        let ty = ResolvedType::new("MyEnum".into())
            .with_enum(true)
            .with_never(false);

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
            .with_enum(true)
            .with_never(false);

        assert!(ty.is_enum());
    }

    #[test]
    fn resolved_type_display_returns_the_text() {
        let ty = ResolvedType::new("i32".into());

        assert_eq!(ty.display(), "i32");
    }

    #[test]
    fn resolved_type_with_option_sets_only_option() {
        let option_ty = ResolvedType::new("Option<T>".into()).with_option(true);

        let flags = (option_ty.is_option(), option_ty.is_result());

        assert_eq!(flags, (true, false));
    }

    #[test]
    fn resolved_type_with_result_sets_only_result() {
        let result_ty = ResolvedType::new("Result<T, E>".into()).with_result(true);

        let flags = (result_ty.is_result(), result_ty.is_option());

        assert_eq!(flags, (true, false));
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
