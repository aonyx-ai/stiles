//! A private module that the crate root declares and does not re-export

pub trait Sealed {}

pub struct Entries;

#[cfg(any())]
pub fn exposed_under_a_disabled_cfg() {}
