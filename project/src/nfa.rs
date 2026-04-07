use std::collections::HashMap;

use crate::ast::ParseTree;

pub const LAMBDA: char = '\0';

#[derive(Debug, Clone)]
pub struct NFA {
    pub alphabet: Vec<char>,
    pub start: i32, // start state
    pub accepting: Vec<i32>,
    pub transitions: HashMap<i32, HashMap<char, Vec<i32>>>,
    next_state: i32, // for generating new states during construction
}

impl NFA {
    fn new() -> Self {
        NFA {
            alphabet:    Vec::new(),
            start:       0,
            accepting:   Vec::new(),
            transitions: HashMap::new(),
            next_state:  0,
        }
    }

    pub fn from_ast(tree: &ParseTree) -> Result<Self, String> {
        todo!()
    }
}