use std::collections::HashMap;

use crate::ast::AstNode;

pub const LAMBDA: char = '\0';

#[derive(Debug, Clone)]
pub struct NFA {
    pub alphabet:    Vec<char>,
    pub start:       i32,
    pub accepting:   Vec<i32>,
    pub transitions: HashMap<i32, HashMap<char, Vec<i32>>>,
    next_state:      i32,
}

 
// Small helper: an NFA fragment produced by Thompson's construction.
// `start` and `end` are the single entry/exit states of the fragment.
struct Fragment {
    start: i32,
    end:   i32,
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

    // Allocate a fresh state number.
    fn new_state(&mut self) -> i32 {
        let s = self.next_state;
        self.next_state += 1;
        s
    }

    // Add a single edge (from) --symbol--> (to).
    fn add_edge(&mut self, from: i32, symbol: char, to: i32) {
        self.transitions
            .entry(from)
            .or_default()
            .entry(symbol)
            .or_default()
            .push(to);
    }

    // Convenience: λ-edge.
    fn add_lambda(&mut self, from: i32, to: i32) {
        self.add_edge(from, LAMBDA, to);
    }


    // Thompson's construction – returns a Fragment (start, end).
    // The `end` state of every fragment is NOT yet marked accepting;
    // only `from_ast` does that after the top-level build.

    fn build(&mut self, node: &AstNode) -> Fragment {
        match node {
            
            // Ch(c): two states, one labelled edge.
            
            AstNode::Ch(c) => {
                let s = self.new_state();
                let e = self.new_state();
                self.add_edge(s, *c, e);
                Fragment { start: s, end: e }
            }

            
            // Dot: one edge per alphabet symbol.
            
            AstNode::Dot => {
                let s = self.new_state();
                let e = self.new_state();
                for &c in &self.alphabet.clone() {
                    self.add_edge(s, c, e);
                }
                Fragment { start: s, end: e }
            }

            
            // Range(lo, hi): one edge per character in [lo..=hi].
            
            AstNode::Range(lo, hi) => {
                let s = self.new_state();
                let e = self.new_state();
                for c in (*lo as u8)..=(*hi as u8) {
                    self.add_edge(s, c as char, e);
                }
                Fragment { start: s, end: e }
            }

            
            // Lambda (ε): two states connected by a λ-edge.
            
            AstNode::Lambda => {
                let s = self.new_state();
                let e = self.new_state();
                self.add_lambda(s, e);
                Fragment { start: s, end: e }
            }

            
            // Seq(a, b): concatenate two fragments.
            
            AstNode::Seq(a, b) => {
                let fa = self.build(a);
                let fb = self.build(b);
                self.add_lambda(fa.end, fb.start);
                Fragment { start: fa.start, end: fb.end }
            }

            
            // Alt(a, b): union of two fragments.
            
            AstNode::Alt(a, b) => {
                let s = self.new_state();
                let e = self.new_state();
                let fa = self.build(a);
                let fb = self.build(b);
                self.add_lambda(s, fa.start);
                self.add_lambda(s, fb.start);
                self.add_lambda(fa.end, e);
                self.add_lambda(fb.end, e);
                Fragment { start: s, end: e }
            }

            
            // Star(a): Kleene star.

            AstNode::Star(a) => {
                let s = self.new_state();
                let e = self.new_state();
                let fa = self.build(a);
                self.add_lambda(s, e);           // zero repetitions
                self.add_lambda(s, fa.start);    // enter loop
                self.add_lambda(fa.end, fa.start); // repeat
                self.add_lambda(fa.end, e);      // exit loop
                Fragment { start: s, end: e }
            }

            
            // Plus(a): one-or-more.  Equivalent to Seq(a, Star(a)).
            
            AstNode::Plus(a) => {
                let s = self.new_state();
                let e = self.new_state();
                let fa = self.build(a);
                self.add_lambda(s, fa.start);      // enter (mandatory)
                self.add_lambda(fa.end, fa.start); // repeat
                self.add_lambda(fa.end, e);        // exit
                Fragment { start: s, end: e }
            }
        }
    }

    // Public entry point.
    /// Build an NFA from an AST.

    pub fn from_ast(node: &AstNode, alphabet: Vec<char>) -> Self {
        let mut nfa = NFA::new();
        nfa.alphabet = alphabet;

        let frag = nfa.build(node);

        nfa.start     = frag.start;
        nfa.accepting = vec![frag.end];
        nfa
    }
    
    // Size: n × n where n = number of states allocated.
    pub fn build_lambda_matrix(&self) -> Vec<Vec<u8>> {
        let n = self.next_state as usize;
        let mut l = vec![vec![0u8; n]; n];

        for (&state, trans) in &self.transitions {
            if let Some(targets) = trans.get(&LAMBDA) {
                for &t in targets {
                    l[state as usize][t as usize] = 1;
                }
            }
        }

        l
    }

    // Transition table  T[state][symbol] = [target states]
    // Contains only non-λ edges.
    pub fn build_transition_table(&self) -> HashMap<i32, HashMap<char, Vec<i32>>> {
        let mut table: HashMap<i32, HashMap<char, Vec<i32>>> = HashMap::new();

        for (&state, trans) in &self.transitions {
            for (&symbol, targets) in trans {
                if symbol != LAMBDA {
                    table
                        .entry(state)
                        .or_default()
                        .entry(symbol)
                        .or_default()
                        .extend(targets);
                }
            }
        }

        table
    }

    pub fn write_to_file(&self, path: &str, lambda_char: char) -> Result<(), std::io::Error> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;

        fn encode(c: char) -> String {
            if (c as u8) < 32 || c == 'x' || c == '\\' || c == ':' || c.is_whitespace() {
                format!("x{:02x}", c as u8)
            } else {
                c.to_string()
            }
        }

        let n = self.next_state as usize;

        // Line 1: num_states lambda alphabet
        write!(file, "{} {}", n, encode(lambda_char))?;
        for &c in &self.alphabet {
            write!(file, " {}", encode(c))?;
        }
        writeln!(file)?;

        // Non-lambda transitions
        for (&from, trans) in &self.transitions {
            for (&symbol, targets) in trans {
                if symbol == LAMBDA { continue; }
                for &to in targets {
                    writeln!(file, "- {} {} {}", from, to, encode(symbol))?;
                }
            }
        }

        // Lambda transitions
        for (&from, trans) in &self.transitions {
            if let Some(targets) = trans.get(&LAMBDA) {
                for &to in targets {
                    writeln!(file, "- {} {} {}", from, to, encode(lambda_char))?;
                }
            }
        }

        // Accepting state
        for &acc in &self.accepting {
            writeln!(file, "+ {} {}", acc, acc)?;
        }

        Ok(())
    }
}

// Printing utilities for debugging.
pub fn print_lambda_matrix(L: &Vec<Vec<u8>>) {
    println!("Lambda Matrix (L):");

    // header
    print!("   ");
    for j in 0..L.len() {
        print!("{:3}", j);
    }
    println!();

    for (i, row) in L.iter().enumerate() {
        print!("{:3}", i);
        for val in row {
            print!("{:3}", val);
        }
        println!();
    }
}

pub fn print_transition_table(
    T: &HashMap<i32, HashMap<char, Vec<i32>>>
) {
    println!("Transition Table (T):");

    let mut states: Vec<_> = T.keys().collect();
    states.sort();

    for state in states {
        println!("State {}:", state);

        let mut symbols: Vec<_> = T[state].keys().collect();
        symbols.sort();

        for symbol in symbols {
            let mut targets = T[state][symbol].clone();
            targets.sort();

            print!("  --{}--> ", symbol);
            for t in targets {
                print!("{} ", t);
            }
            println!();
        }
    }
}