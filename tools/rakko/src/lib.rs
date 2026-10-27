//! The actions that only this repository mounts
//!
//! The bundles of Rakko check every workspace of the repository, but some
//! checks of Whisker reach past a workspace. The lint plugin that the tests of
//! whisker-rust load stays outside every workspace, because whisker-rust
//! depends on it by path. Other checks build the whisker binary and run it,
//! which no action of Rakko knows how to do. The actions of this crate cover
//! both, and the harness mounts them next to the bundles.

/// Types for the action that compares the plugin boundary with the one on main
pub mod check_abi;
/// Types for the action that checks the language crates without their providers
pub mod check_features;
/// Types for the action that runs whisker on this repository
pub mod check_self;
/// The error that stops a run of an action of this crate
mod error;
/// Types for the action that runs another action in a directory of the project
pub mod in_directory;
/// The steps that the actions of this crate share
mod step;
/// Types for the action that loads a plugin built with another toolchain
pub mod test_plugin_toolchain;

pub use self::check_abi::CheckAbi;
pub use self::check_features::CheckFeatures;
pub use self::check_self::CheckSelf;
pub use self::error::HarnessError;
pub use self::in_directory::InDirectory;
pub use self::test_plugin_toolchain::TestPluginToolchain;
