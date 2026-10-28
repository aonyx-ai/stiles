//! Type information the Rust provider attaches to tree-sitter nodes
//!
//! Every type here crosses from the provider to the lint crates, so none of
//! them names an `ra_ap_*` type. What a lint can ask is exactly what this
//! module can express.

mod adt_flags;
mod error_type;
mod fn_signature;
mod import_source;
mod public_path;
mod reexports;
mod resolved_type;
mod return_mode;
mod type_path;
mod type_path_ref;

pub use self::adt_flags::AdtFlags;
pub use self::error_type::ErrorType;
pub use self::fn_signature::FnSignature;
pub use self::import_source::ImportSource;
pub use self::public_path::PublicPath;
pub use self::reexports::Reexports;
pub use self::resolved_type::ResolvedType;
pub use self::return_mode::ReturnMode;
pub use self::type_path::TypePath;
pub use self::type_path_ref::TypePathRef;
