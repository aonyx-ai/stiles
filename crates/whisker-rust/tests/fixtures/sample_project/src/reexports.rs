//! Imports that the `Reexports` decoration is tested against
//!
//! `facade` re-exports parts of `facade_core`, and `lookalike_json` depends
//! on `lookalike` without re-exporting it. Each `use` names its item on its
//! own line, so a test finds the path by its text.

#![allow(unused_imports)]

use core::fmt::Debug;

use anyhow::Context;
use facade::thing::Thing as FacadeThing;
use facade_core::Draw;
use facade_core::Shape::Circle;
use facade_core::hidden::Hidden;
use facade_core::thing::Thing;
use facade_core::unexported::Internal;
use facade_core::{Shape, thing::make};
use lookalike::Serialize;
use lookalike_json::Value;

use crate::Palette;
