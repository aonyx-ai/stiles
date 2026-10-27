//! What the provider asks the tsrs checker and program, turned into
//! decorations
//!
//! Each function takes tsrs handles and returns a type from
//! [`crate::decorations`], so no tsrs type leaves this module.

use std::path::Path;

use whisker_tsrs::tsrs_ast::{Kind, Node, SourceFile, SymbolFlags, get_type_annotation_node};
use whisker_tsrs::tsrs_checker::{Checker, LiteralValue, Type, TypeFlags};
use whisker_tsrs::tsrs_compiler::Program;
use whisker_tsrs::tsrs_core::P;
use whisker_tsrs::tsrs_core::tspath::is_external_module_name_relative;

use crate::decorations::{
    FnSignature, ImportSource, Parameter, Primitive, ResolvedType, StringUnion,
};

/// The declarations whose type annotation names the type of their value
///
/// A function declaration has an annotation too, but it names the return
/// type, not the type of the function.
const VALUE_DECLARATIONS: [Kind; 4] = [
    Kind::Parameter,
    Kind::VariableDeclaration,
    Kind::PropertySignature,
    Kind::PropertyDeclaration,
];

/// The tsrs kinds of a declaration that has a call signature of its own
const FUNCTION_DECLARATIONS: [Kind; 3] = [
    Kind::FunctionDeclaration,
    Kind::MethodDeclaration,
    Kind::MethodSignature,
];

/// Describes `ty`, the type of a value that `declaration` declares
///
/// The alias comes from the type when the checker kept one, as it does for
/// a union or an object type. An alias of a primitive leaves no trace on
/// the type. The alias then comes from the annotation that `declaration`
/// writes, such as `Name` in `name: Name`.
pub(crate) fn resolved_type(
    checker: &mut Checker,
    ty: P<Type>,
    declaration: Option<P<Node>>,
) -> ResolvedType {
    let display = checker.type_to_string_exported(ty);
    let alias = ty
        .alias()
        .and_then(|alias| alias.symbol())
        .map(|symbol| symbol.name())
        .or_else(|| {
            let declaration = declaration?;
            if !VALUE_DECLARATIONS.contains(&declaration.kind()) {
                return None;
            }
            written_alias(checker, get_type_annotation_node(declaration)?)
        });

    ResolvedType::new(&display, alias, primitive(ty))
}

/// Returns the alias an annotation names, if it names one
///
/// An imported alias reaches here as the local name of the import, which
/// is the name the source wrote.
fn written_alias(checker: &mut Checker, annotation: P<Node>) -> Option<&'static str> {
    if annotation.kind() != Kind::TypeReference {
        return None;
    }
    let name = annotation.as_type_reference_node().type_name();
    let symbol = checker.get_symbol_at_location_exported(name)?;

    match symbol
        .flags()
        .intersects(SymbolFlags::TypeAlias | SymbolFlags::Alias)
    {
        true => Some(symbol.name()),
        false => None,
    }
}

/// Returns the primitive `ty` comes down to
///
/// `boolean` is the union `true | false` to the checker, so its flag is
/// read before any other. An enum member is a literal of its enum, not of
/// its primitive.
pub(crate) fn primitive(ty: P<Type>) -> Primitive {
    let flags = ty.flags();

    if flags.intersects(TypeFlags::EnumLiteral | TypeFlags::Enum) {
        return Primitive::NotPrimitive;
    }
    if flags.intersects(TypeFlags::BooleanLike) {
        return Primitive::Boolean;
    }
    if flags.intersects(TypeFlags::String | TypeFlags::StringLiteral) {
        return Primitive::String;
    }
    if flags.intersects(TypeFlags::Number | TypeFlags::NumberLiteral) {
        return Primitive::Number;
    }
    if flags.intersects(TypeFlags::BigIntLike) {
        return Primitive::BigInt;
    }
    if flags.intersects(TypeFlags::ESSymbolLike) {
        return Primitive::Symbol;
    }

    Primitive::NotPrimitive
}

/// Returns the members of `ty` if it is a union of string literals
pub(crate) fn string_union(ty: P<Type>) -> Option<StringUnion> {
    if !ty.flags().intersects(TypeFlags::Union) {
        return None;
    }

    let members: Option<Vec<&'static str>> = ty
        .types()
        .iter()
        .map(|member| {
            if !member.is_string_literal() {
                return None;
            }
            match member.as_literal_type().value() {
                Some(LiteralValue::String(text)) => Some(text),
                Some(LiteralValue::Number(_))
                | Some(LiteralValue::Boolean(_))
                | Some(LiteralValue::BigInt(_))
                | None => None,
            }
        })
        .collect();

    members
        .filter(|members| !members.is_empty())
        .map(StringUnion::new)
}

/// Describes the signature `declaration` declares, if it declares a function
pub(crate) fn fn_signature(checker: &mut Checker, declaration: P<Node>) -> Option<FnSignature> {
    if !FUNCTION_DECLARATIONS.contains(&declaration.kind()) {
        return None;
    }

    let signature = checker.get_signature_from_declaration_exported(declaration);
    let mut parameters = Vec::new();
    for &parameter in signature.parameters() {
        let ty = checker.get_type_of_symbol_exported(parameter);
        let ty = resolved_type(checker, ty, parameter.value_declaration());
        parameters.push(Parameter::new(parameter.name(), ty));
    }

    let returned = checker.get_return_type_of_signature_exported(signature);
    let alias = get_type_annotation_node(declaration)
        .and_then(|annotation| written_alias(checker, annotation));
    let display = checker.type_to_string_exported(returned);
    let return_type = ResolvedType::new(&display, alias, primitive(returned));

    Some(FnSignature::new(parameters, return_type))
}

/// Returns where the module that `specifier`, a string literal, names comes from
pub(crate) fn import_source(
    program: &Program,
    file: P<SourceFile>,
    specifier: P<Node>,
) -> ImportSource {
    let Some(resolved) = program.get_resolved_module_from_module_specifier(file, specifier) else {
        return ImportSource::Unresolved;
    };
    if !resolved.is_resolved() {
        return ImportSource::Unresolved;
    }

    classify(specifier.text(), Path::new(resolved.resolved_file_name))
}

/// Sorts a resolved import by its specifier and the file it resolved to
///
/// A relative specifier is relative whatever it reached. Otherwise a file
/// under `node_modules` is a dependency. A package specifier that resolved
/// anywhere else came through a workspace link or a `paths` mapping. The
/// resolver follows a workspace link to the package's real directory.
/// Either way the package belongs to this repository.
fn classify(specifier: &str, resolved: &Path) -> ImportSource {
    if is_external_module_name_relative(specifier) {
        return ImportSource::Relative;
    }

    let in_node_modules = resolved
        .components()
        .any(|component| component.as_os_str() == "node_modules");
    match in_node_modules {
        true => ImportSource::Dependency,
        false => ImportSource::Workspace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_with_a_package_in_node_modules_returns_dependency() {
        let source = classify("lodash", Path::new("/repo/node_modules/lodash/index.d.ts"));

        assert_eq!(source, ImportSource::Dependency);
    }

    #[test]
    fn classify_with_a_package_outside_node_modules_returns_workspace() {
        let source = classify("@aonyx/sdk", Path::new("/repo/packages/sdk/src/index.ts"));

        assert_eq!(source, ImportSource::Workspace);
    }

    #[test]
    fn classify_with_a_relative_specifier_returns_relative() {
        let source = classify("../a", Path::new("/repo/node_modules/x/a.ts"));

        assert_eq!(source, ImportSource::Relative);
    }
}
