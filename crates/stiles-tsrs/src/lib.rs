//! The crates of [tsrs] at commit 4a61321929d0d62b86c5fb35f15bfa1607e03b50, one module each
//!
//! `tools/bundle-tsrs` generates this crate. Do not edit it: change
//! the tool or the submodule, and run `mise run rakko -- bundle-tsrs`.
//!
//! [tsrs]: https://github.com/oxc-project/tsrs

// The code is upstream's, and its lints are upstream's to fix.
#![allow(warnings, clippy::all, clippy::pedantic, clippy::nursery)]

#[path = "../tsrs_ast/src/lib.rs"]
pub mod tsrs_ast;

#[path = "../tsrs_astnav/src/lib.rs"]
pub mod tsrs_astnav;

#[path = "../tsrs_binder/src/lib.rs"]
pub mod tsrs_binder;

#[path = "../tsrs_checker/src/lib.rs"]
pub mod tsrs_checker;

#[path = "../tsrs_compiler/src/lib.rs"]
pub mod tsrs_compiler;

#[path = "../tsrs_contentmapper/src/lib.rs"]
pub mod tsrs_contentmapper;

#[path = "../tsrs_core/src/lib.rs"]
pub mod tsrs_core;

#[path = "../tsrs_declarations/src/lib.rs"]
pub mod tsrs_declarations;

#[path = "../tsrs_diagnostics/src/lib.rs"]
pub mod tsrs_diagnostics;

#[path = "../tsrs_ipc/src/lib.rs"]
pub mod tsrs_ipc;

#[path = "../tsrs_module/src/lib.rs"]
pub mod tsrs_module;

#[path = "../tsrs_modulespecifiers/src/lib.rs"]
pub mod tsrs_modulespecifiers;

#[path = "../tsrs_parser/src/lib.rs"]
pub mod tsrs_parser;

#[path = "../tsrs_printer/src/lib.rs"]
pub mod tsrs_printer;

#[path = "../tsrs_pseudochecker/src/lib.rs"]
pub mod tsrs_pseudochecker;

#[path = "../tsrs_scanner/src/lib.rs"]
pub mod tsrs_scanner;

#[path = "../tsrs_sourcemap/src/lib.rs"]
pub mod tsrs_sourcemap;

#[path = "../tsrs_spanmap/src/lib.rs"]
pub mod tsrs_spanmap;

#[path = "../tsrs_transformers/src/lib.rs"]
pub mod tsrs_transformers;

#[path = "../tsrs_tsoptions/src/lib.rs"]
pub mod tsrs_tsoptions;

#[path = "../tsrs_vfs/src/lib.rs"]
pub mod tsrs_vfs;
