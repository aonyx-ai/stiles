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

pub use fn_signature::{FnSignature, Parameter};
pub use import_source::ImportSource;
pub use primitive::Primitive;
pub use resolved_type::ResolvedType;
pub use string_union::StringUnion;
pub use type_text::TypeText;
