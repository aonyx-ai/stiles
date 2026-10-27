use bon::bon;
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
/// let ty = ResolvedType::builder()
///     .display("string")
///     .alias("Name")
///     .primitive(Primitive::String)
///     .build();
/// let name = Parameter::builder()
///     .name("name")
///     .ty(ty)
///     .build();
///
/// assert_eq!(name.name(), "name");
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Parameter {
    name: string::String,
    ty: ResolvedType,
}

#[bon]
impl Parameter {
    /// Describes the parameter `name` of type `ty`
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{Parameter, Primitive, ResolvedType};
    ///
    /// let ty = ResolvedType::builder()
    ///     .display("number")
    ///     .primitive(Primitive::Number)
    ///     .build();
    /// let count = Parameter::builder()
    ///     .name("count")
    ///     .ty(ty)
    ///     .build();
    ///
    /// assert_eq!(count.ty().primitive(), Primitive::Number);
    /// ```
    #[builder]
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
    /// let ty = ResolvedType::builder()
    ///     .display("number")
    ///     .primitive(Primitive::Number)
    ///     .build();
    /// let count = Parameter::builder()
    ///     .name("count")
    ///     .ty(ty)
    ///     .build();
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
    /// let ty = ResolvedType::builder()
    ///     .display("number")
    ///     .primitive(Primitive::Number)
    ///     .build();
    /// let count = Parameter::builder()
    ///     .name("count")
    ///     .ty(ty)
    ///     .build();
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
/// let string = |alias| ResolvedType::builder()
///     .display("string")
///     .alias(alias)
///     .primitive(Primitive::String)
///     .build();
/// let greeting = Parameter::builder()
///     .name("greeting")
///     .ty(string("Greeting"))
///     .build();
/// let name = Parameter::builder()
///     .name("name")
///     .ty(string("Name"))
///     .build();
/// let void = ResolvedType::builder()
///     .display("void")
///     .primitive(Primitive::NotPrimitive)
///     .build();
/// let signature = FnSignature::builder()
///     .parameters(vec![greeting, name])
///     .return_type(void)
///     .build();
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

#[bon]
impl FnSignature {
    /// Describes a function by its parameters, in order, and its return type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::{FnSignature, Primitive, ResolvedType};
    ///
    /// let return_type = ResolvedType::builder()
    ///     .display("number")
    ///     .primitive(Primitive::Number)
    ///     .build();
    /// let signature = FnSignature::builder()
    ///     .parameters(Vec::new())
    ///     .return_type(return_type)
    ///     .build();
    ///
    /// assert!(signature.parameters().is_empty());
    /// ```
    #[builder]
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
    /// let return_type = ResolvedType::builder()
    ///     .display("void")
    ///     .primitive(Primitive::NotPrimitive)
    ///     .build();
    /// let signature = FnSignature::builder()
    ///     .parameters(Vec::new())
    ///     .return_type(return_type)
    ///     .build();
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
    /// let return_type = ResolvedType::builder()
    ///     .display("void")
    ///     .primitive(Primitive::NotPrimitive)
    ///     .build();
    /// let signature = FnSignature::builder()
    ///     .parameters(Vec::new())
    ///     .return_type(return_type)
    ///     .build();
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
        let number = ResolvedType::builder()
            .display("number")
            .primitive(Primitive::Number)
            .build();
        let signature = FnSignature::builder()
            .parameters(vec![
                Parameter::builder().name("a").ty(number.clone()).build(),
                Parameter::builder().name("b").ty(number.clone()).build(),
            ])
            .return_type(number)
            .build();

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
