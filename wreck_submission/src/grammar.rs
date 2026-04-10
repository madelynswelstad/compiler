// grammar.rs
#[derive(Clone, Debug)]
pub enum Symbol {
    Terminal(String),
    NonTerminal(String),
}

/// A production rule: lhs -> rhs
#[derive(Clone, Debug)]
pub struct Production {
    pub lhs: String,
    pub rhs: Vec<Symbol>,
}

impl Production {
    pub fn derives_lambda(&self) -> bool {
        self.rhs.is_empty()
            || (self.rhs.len() == 1
                && matches!(&self.rhs[0], Symbol::Terminal(t) if t == "lambda"))
    }
}