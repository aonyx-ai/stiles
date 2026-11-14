//! The library target, which shares a module with the binary

pub mod shared;
pub mod test_code;

#[cfg(test)]
mod declared;
mod gated;
