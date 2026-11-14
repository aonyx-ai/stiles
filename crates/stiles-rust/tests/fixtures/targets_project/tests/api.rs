//! An integration test target with a module of its own

mod common;

#[test]
fn greets() {
    common::setup();
}
