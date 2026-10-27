use stabby::{string, vec};
use whisker_macros::Decoration;

use crate::decorations::ResolvedType;

/// One parameter of a function, by name and type
///
/// # Examples
///
/// ```
/// use whisker_typescript::decorations::{Parameter, Primitive, ResolvedType};
///
/// let name = Parameter::new("name", ResolvedType::new("string", Some("Name"), Primitive::String));
///
/// assert_eq!(name.name(), "name");
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Parameter {
    name: string::String,
    ty: ResolvedType,
}

impl Parameter {
    /// Describes the parameter `name` of type `ty`
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{Parameter, Primitive, ResolvedType};
    ///
    /// let count = Parameter::new("count", ResolvedType::new("number", None, Primitive::Number));
    ///
    /// assert_eq!(count.ty().primitive(), Primitive::Number);
    /// ```
    pub fn new(name: &str, ty: ResolvedType) -> Self {
        Self {
            name: string::String::from(name),
            ty,
        }
    }

    /// Returns the parameter's name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{Parameter, Primitive, ResolvedType};
    ///
    /// let count = Parameter::new("count", ResolvedType::new("number", None, Primitive::Number));
    ///
    /// assert_eq!(count.name(), "count");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parameter's type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{Parameter, Primitive, ResolvedType};
    ///
    /// let count = Parameter::new("count", ResolvedType::new("number", None, Primitive::Number));
    ///
    /// assert_eq!(count.ty().display(), "number");
    /// ```
    pub fn ty(&self) -> &ResolvedType {
        &self.ty
    }
}

/// The parameters and return type of a function the source declares
///
/// The TypeScript provider attaches one to every function declaration,
/// method, and method or function signature it can resolve. Each type
/// carries its written alias and its primitive. A rule thus tells two
/// parameters a caller can swap from two the checker keeps apart.
///
/// # Examples
///
/// ```
/// use whisker_typescript::decorations::{FnSignature, Parameter, Primitive, ResolvedType};
///
/// let string = |alias| ResolvedType::new("string", Some(alias), Primitive::String);
/// let signature = FnSignature::new(
///     vec![Parameter::new("greeting", string("Greeting")), Parameter::new("name", string("Name"))],
///     ResolvedType::new("void", None, Primitive::NotPrimitive),
/// );
///
/// assert_eq!(signature.parameters().len(), 2);
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct FnSignature {
    parameters: vec::Vec<Parameter>,
    return_type: ResolvedType,
}

impl FnSignature {
    /// Describes a function by its parameters, in order, and its return type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{FnSignature, Primitive, ResolvedType};
    ///
    /// let signature = FnSignature::new(Vec::new(), ResolvedType::new("number", None, Primitive::Number));
    ///
    /// assert!(signature.parameters().is_empty());
    /// ```
    pub fn new(parameters: Vec<Parameter>, return_type: ResolvedType) -> Self {
        Self {
            parameters: parameters.into_iter().collect(),
            return_type,
        }
    }

    /// Returns the parameters in declaration order
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{FnSignature, Primitive, ResolvedType};
    ///
    /// let signature = FnSignature::new(Vec::new(), ResolvedType::new("void", None, Primitive::NotPrimitive));
    ///
    /// assert!(signature.parameters().is_empty());
    /// ```
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    /// Returns the return type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{FnSignature, Primitive, ResolvedType};
    ///
    /// let signature = FnSignature::new(Vec::new(), ResolvedType::new("void", None, Primitive::NotPrimitive));
    ///
    /// assert_eq!(signature.return_type().display(), "void");
    /// ```
    pub fn return_type(&self) -> &ResolvedType {
        &self.return_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decorations::Primitive;

    #[test]
    fn parameters_keep_their_order() {
        let number = ResolvedType::new("number", None, Primitive::Number);
        let signature = FnSignature::new(
            vec![
                Parameter::new("a", number.clone()),
                Parameter::new("b", number.clone()),
            ],
            number,
        );

        let names: Vec<&str> = signature.parameters().iter().map(Parameter::name).collect();

        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FnSignature>();
        assert_send::<Parameter>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FnSignature>();
        assert_sync::<Parameter>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FnSignature>();
        assert_unpin::<Parameter>();
    }
}
