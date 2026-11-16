//! A constituent crate that `facade` re-exports in part

pub mod thing {
    pub struct Thing;

    pub fn make() -> Thing {
        Thing
    }
}

pub mod r#type {
    pub struct Kind;
}

pub mod unexported {
    pub struct Internal;
}

pub mod hidden {
    pub struct Hidden;
}

pub enum Shape {
    Circle,
    Square,
}

pub trait Draw {
    fn draw(&self);
}
