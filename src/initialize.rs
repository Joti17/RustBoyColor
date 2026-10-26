#![allow(unused_variables, non_snake_case, dead_code, unused)]
pub trait Initializable {
    /// also means reset
    fn init(&mut self);
}
