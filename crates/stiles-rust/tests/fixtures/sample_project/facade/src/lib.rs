//! A facade that re-exports a module and an enum of `facade_core`
//!
//! It also re-exports the enum `Tint` of `tint-core` at its root, away from
//! the module that defines it.
//!
//! It re-exports `Hidden` and the `unexported` module too, but marks both
//! re-exports `#[doc(hidden)]`, so it offers neither path to its users.

pub use facade_core::Shape;
pub use facade_core::thing;
pub use tint_core::tint::Tint;

#[doc(hidden)]
pub use facade_core::hidden::Hidden;
#[doc(hidden)]
pub use facade_core::unexported;
