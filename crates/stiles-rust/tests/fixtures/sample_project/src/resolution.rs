//! Names that the `Resolution` decoration is tested against
//!
//! Each function holds the names that a few tests read, and a test finds a
//! name by its text inside the function that holds it.

#![allow(dead_code)]

use facade_core::thing::make;

use self::globbed::*;
use self::kinds::Kind::*;
use self::kinds::Opened;

pub mod kinds {
    pub struct Opened(pub u8);

    pub enum Kind {
        Pending(u8),
        Closed,
    }
}

pub mod globbed {
    pub fn tally() -> u8 {
        1
    }
}

pub struct Point {
    pub x: u8,
    pub y: u8,
}

impl Point {
    pub fn origin() -> Self {
        Self { x: 0, y: 0 }
    }

    pub fn sum(&self) -> u8 {
        self.x + self.y
    }
}

pub trait Describe {
    fn describe(&self) -> u8;
}

impl Describe for Point {
    fn describe(&self) -> u8 {
        self.x
    }
}

macro_rules! twice {
    ($value:expr) => {
        $value * 2
    };
}

pub fn calls_through_a_use() -> facade_core::thing::Thing {
    make()
}

pub fn calls_through_a_glob() -> u8 {
    tally()
}

pub fn calls_through_a_reexport() -> facade_core::thing::Thing {
    facade::thing::make()
}

pub fn calls_a_method(point: &Point) -> u8 {
    point.sum()
}

pub fn calls_a_trait_method(point: &Point) -> u8 {
    point.describe()
}

pub fn destructures_a_struct(point: Point) -> u8 {
    let Point { x, y } = point;
    x + y
}

pub fn matches_a_struct_and_a_variant(opened: Option<Opened>, kind: Option<kinds::Kind>) -> u8 {
    let from_struct = match opened {
        Some(Opened(count)) => count,
        None => 0,
    };
    let from_variant = match kind {
        Some(Pending(count)) => count,
        Some(Closed) | None => 0,
    };
    from_struct + from_variant
}

pub fn reads_a_local_binding() -> u8 {
    let total = 1;
    total + 1
}

pub fn takes_a_point_in_its_signature(point: Point) -> Point {
    point
}

pub fn calls_a_macro() -> u8 {
    twice!(2)
}
