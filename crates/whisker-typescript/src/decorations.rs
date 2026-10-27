//! Type information the TypeScript provider attaches to tree-sitter nodes
//!
//! Every type here crosses from the provider to the lint crates, so none of
//! them names a tsrs type. What a lint can ask is exactly what this module
//! can express.

mod fn_signature;
mod import_source;
mod primitive;
mod resolved_type;
mod string_union;
mod type_text;

pub use self::fn_signature::{FnSignature, Parameter};
pub use self::import_source::ImportSource;
pub use self::primitive::Primitive;
pub use self::resolved_type::ResolvedType;
pub use self::string_union::StringUnion;
pub use self::type_text::TypeText;
