//! A crate whose name has a dash, and whose enum sits in a module
//!
//! `facade` re-exports `Tint` at its own root, so the path a user writes
//! and the path of the definition differ in both the crate and the module.

pub mod tint {
    pub enum Tint {
        Warm,
        Cool,
    }
}
