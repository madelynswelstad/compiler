use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;

use crate::ast::ParseTree;
use crate::grammar::{Production, Symbol};

const EOF: &str = "$";

// Load a CFG from a file, compute nullable/first/follow sets, and build the LL(1) parse table
pub struct CFG {
    pub productions:  Vec<Production>,
    pub start:        String,
    pub terminals:    HashSet<String>,
    pub non_terminals: HashSet<String>,

    // Computed during construction:
    pub nullable:    HashSet<String>,
    pub first_sets:  HashMap<String, HashSet<String>>,
    pub follow_sets: HashMap<String, HashSet<String>>,
    pub parse_table: HashMap<String, HashMap<String, usize>>,
}

impl CFG {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let src = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read CFG file '{}': {}", path, e))?;
        Self::from_str(&src)
    }

    pub fn from_str(src: &str) -> Result<Self, String> {
        let productions = Self::parse_productions(src)?;
        if productions.is_empty() {
            return Err("CFG file is empty".into());
        }

        let start = productions[0].lhs.clone();
        let mut terminals:     HashSet<String> = HashSet::new();
        let mut non_terminals: HashSet<String> = HashSet::new();

        for p in &productions {
            non_terminals.insert(p.lhs.clone());
        }
        for p in &productions {
            for sym in &p.rhs {
                match sym {
                    Symbol::Terminal(t)    => { if t != "lambda" { terminals.insert(t.clone()); } }
                    Symbol::NonTerminal(n) => { non_terminals.insert(n.clone()); }
                }
            }
        }

        let mut cfg = CFG {
            productions,
            start,
            terminals,
            non_terminals,
            nullable:    HashSet::new(),
            first_sets:  HashMap::new(),
            follow_sets: HashMap::new(),
            parse_table: HashMap::new(),
        };
        cfg.compute_nullable();
        cfg.compute_first();
        cfg.compute_follow();
        cfg.build_parse_table()?;
        Ok(cfg)
    }
}

// Grammar file parsing
impl CFG {
    fn classify(token: &str) -> Symbol {
        if token.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            Symbol::NonTerminal(token.to_string())
        } else {
            Symbol::Terminal(token.to_string())
        }
    }

    fn parse_productions(src: &str) -> Result<Vec<Production>, String> {
        let mut productions: Vec<Production> = Vec::new();
        let mut current_lhs: Option<String>  = None;

        for (lineno, raw) in src.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() { continue; }

            if let Some(arrow) = line.find("->") {
                let lhs   = line[..arrow].trim().to_string();
                let rhs_s = line[arrow + 2..].trim();
                if lhs.is_empty() {
                    return Err(format!("Line {}: LHS is empty", lineno + 1));
                }
                current_lhs = Some(lhs.clone());
                productions.push(Production { lhs, rhs: Self::parse_rhs(rhs_s) });
            } else if line.starts_with('|') {
                let lhs = current_lhs.clone()
                    .ok_or_else(|| format!("Line {}: '|' without preceding rule", lineno + 1))?;
                productions.push(Production { lhs, rhs: Self::parse_rhs(line[1..].trim()) });
            } else {
                return Err(format!("Line {}: cannot parse '{}'", lineno + 1, line));
            }
        }
        Ok(productions)
    }

    fn parse_rhs(s: &str) -> Vec<Symbol> {
        s.split_whitespace().map(Self::classify).collect()
    }
}

// Compute nullable set: all non-terminals that can derive lambda
impl CFG {
    fn compute_nullable(&mut self) {
        loop {
            let before = self.nullable.len();
            for p in &self.productions {
                if self.nullable.contains(&p.lhs) { continue; }
                let nullable = p.derives_lambda() || p.rhs.iter().all(|s| match s {
                    Symbol::Terminal(t)    => t == "lambda",
                    Symbol::NonTerminal(n) => self.nullable.contains(n),
                });
                if nullable { self.nullable.insert(p.lhs.clone()); }
            }
            if self.nullable.len() == before { break; }
        }
    }
}

// Compute first sets
impl CFG {
    pub fn first_of_seq(&self, seq: &[Symbol]) -> HashSet<String> {
        let mut result = HashSet::new();
        for sym in seq {
            match sym {
                Symbol::Terminal(t) => {
                    if t != "lambda" { result.insert(t.clone()); return result; }
                }
                Symbol::NonTerminal(n) => {
                    if let Some(fs) = self.first_sets.get(n) {
                        result.extend(fs.iter().cloned());
                    }
                    if !self.nullable.contains(n) { return result; }
                }
            }
        }
        result
    }

    fn compute_first(&mut self) {
        for nt in &self.non_terminals {
            self.first_sets.insert(nt.clone(), HashSet::new());
        }
        loop {
            let mut changed = false;
            for p in &self.productions {
                let derived = self.first_of_seq(&p.rhs);
                let set = self.first_sets.get_mut(&p.lhs).unwrap();
                let before = set.len();
                set.extend(derived);
                if set.len() != before { changed = true; }
            }
            if !changed { break; }
        }
    }
}

// Compute follow sets
impl CFG {
    fn compute_follow(&mut self) {
        for nt in &self.non_terminals {
            self.follow_sets.insert(nt.clone(), HashSet::new());
        }
        self.follow_sets.get_mut(&self.start).unwrap().insert(EOF.to_string());

        loop {
            let mut changed = false;
            let prods: Vec<Production> = self.productions.clone();
            for p in &prods {
                for (i, sym) in p.rhs.iter().enumerate() {
                    let nt = match sym {
                        Symbol::NonTerminal(n) => n.clone(),
                        _ => continue,
                    };
                    let rest = &p.rhs[i + 1..];
                    let first_rest = self.first_of_seq(rest);
                    let set = self.follow_sets.get_mut(&nt).unwrap();
                    let before = set.len();
                    set.extend(first_rest);
                    if set.len() != before { changed = true; }

                    let rest_nullable = rest.iter().all(|s| match s {
                        Symbol::Terminal(t)    => t == "lambda",
                        Symbol::NonTerminal(n) => self.nullable.contains(n),
                    });
                    if rest_nullable {
                        let follow_lhs = self.follow_sets[&p.lhs].clone();
                        let set = self.follow_sets.get_mut(&nt).unwrap();
                        let before = set.len();
                        set.extend(follow_lhs);
                        if set.len() != before { changed = true; }
                    }
                }
            }
            if !changed { break; }
        }
    }
}

// Construct LL(1) parse table
impl CFG {
    fn build_parse_table(&mut self) -> Result<(), String> {
        for nt in &self.non_terminals {
            self.parse_table.insert(nt.clone(), HashMap::new());
        }

        for (prod_idx, p) in self.productions.iter().enumerate() {
            // Add for each token in FIRST(rhs)
            for t in self.first_of_seq(&p.rhs).iter().cloned().collect::<Vec<_>>() {
                let row = self.parse_table.get_mut(&p.lhs).unwrap();
                if let Some(existing) = row.insert(t.clone(), prod_idx) {
                    if existing != prod_idx {
                        return Err(format!(
                            "Grammar is not LL(1): conflict on [{}, {}] — productions {} and {}",
                            p.lhs, t, existing, prod_idx
                        ));
                    }
                }
            }

            // If rhs derives to lambda, add for each token in FOLLOW(lhs)
            let rhs_nullable = p.derives_lambda() || p.rhs.iter().all(|s| match s {
                Symbol::Terminal(t)    => t == "lambda",
                Symbol::NonTerminal(n) => self.nullable.contains(n),
            });

            if rhs_nullable {
                for t in self.follow_sets[&p.lhs].clone().iter().cloned().collect::<Vec<_>>() {
                    let row = self.parse_table.get_mut(&p.lhs).unwrap();
                    if let Some(existing) = row.insert(t.clone(), prod_idx) {
                        if existing != prod_idx {
                            return Err(format!(
                                "Grammar is not LL(1): conflict on [{}, {}] — productions {} and {}",
                                p.lhs, t, existing, prod_idx
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// LL(1) parse
impl CFG {
    pub fn parse(&self, tokens: &[String]) -> Result<ParseTree, String> {
        enum WorkItem {
            Sym(Symbol),
            Close { lhs: String, child_count: usize },
        }

        let mut input: VecDeque<String> = tokens.iter().cloned().collect();
        input.push_back(EOF.to_string());

        let mut tree_stack: Vec<Vec<ParseTree>> = vec![vec![]];
        let mut work: Vec<WorkItem> =
            vec![WorkItem::Sym(Symbol::NonTerminal(self.start.clone()))];

        while let Some(item) = work.pop() {
            match item {
                WorkItem::Close { lhs, child_count } => {
                    let frame = tree_stack.last_mut().unwrap();
                    let start = frame.len().saturating_sub(child_count);
                    let children: Vec<ParseTree> = frame.drain(start..).collect();
                    tree_stack.pop();
                    tree_stack
                        .last_mut()
                        .unwrap()
                        .push(ParseTree::Interior { symbol: lhs, children });
                }

                WorkItem::Sym(sym) => {
                    let lookahead = input
                        .front()
                        .ok_or_else(|| "Unexpected end of input".to_string())?
                        .clone();

                    match &sym {
                        Symbol::Terminal(t) => {
                            if t == "lambda" {
                                tree_stack.last_mut().unwrap().push(ParseTree::Lambda);
                            } else if *t == lookahead {
                                input.pop_front();
                                tree_stack.last_mut().unwrap().push(ParseTree::Leaf(t.clone()));
                            } else {
                                return Err(format!(
                                    "Parse error: expected '{}', got '{}'", t, lookahead
                                ));
                            }
                        }

                        Symbol::NonTerminal(nt) => {
                            let row = self.parse_table.get(nt)
                                .ok_or_else(|| format!("No parse-table row for '{}'", nt))?;
                            let prod_idx = row.get(&lookahead).copied()
                                .ok_or_else(|| format!(
                                    "Parse error: no production for '{}' on input '{}'",
                                    nt, lookahead
                                ))?;
                            let prod = &self.productions[prod_idx];

                            tree_stack.push(vec![]);
                            work.push(WorkItem::Close {
                                lhs: nt.clone(),
                                child_count: prod.rhs.len(),
                            });
                            for sym in prod.rhs.iter().rev() {
                                work.push(WorkItem::Sym(sym.clone()));
                            }
                        }
                    }
                }
            }
        }

        if input.front().map(|s| s.as_str()) == Some(EOF) { input.pop_front(); }
        if !input.is_empty() {
            return Err(format!(
                "Parse error: unconsumed input starting with '{}'", input[0]
            ));
        }

        let mut root_frame = tree_stack
            .pop()
            .ok_or_else(|| "Internal error: empty tree stack".to_string())?;
        if root_frame.len() != 1 {
            return Err(format!("Internal error: root frame has {} nodes", root_frame.len()));
        }
        Ok(root_frame.remove(0))
    }

    // Helper to parse the scanner output file into a parse tree
    pub fn parse_scan_file(&self, path: &str) -> Result<ParseTree, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read scan file '{}': {}", path, e))?;

        let mut tokens: Vec<String> = Vec::new();
        for (i, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || i == 0 { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            for tok in parts.iter().skip(1) {
                tokens.push(tok.to_string());
            }
        }

        self.parse(&tokens)
    }
}

// Debug helpers 

impl CFG {
    pub fn print_first_follow(&self) {
        let mut nts: Vec<&String> = self.non_terminals.iter().collect();
        nts.sort();
        for nt in nts {
            let first: Vec<&String> = {
                let mut v: Vec<_> = self.first_sets.get(nt).map_or(vec![], |s| s.iter().collect());
                v.sort(); v
            };
            let follow: Vec<&String> = {
                let mut v: Vec<_> = self.follow_sets.get(nt).map_or(vec![], |s| s.iter().collect());
                v.sort(); v
            };
            println!("  {:12}  FIRST={:?}  FOLLOW={:?}", nt, first, follow);
        }
    }

    pub fn print_parse_table(&self) {
        let mut nts: Vec<&String> = self.parse_table.keys().collect();
        nts.sort();
        for nt in nts {
            let row = &self.parse_table[nt];
            let mut entries: Vec<_> = row.iter().collect();
            entries.sort_by_key(|(t, _)| *t);
            for &(tok, pidx) in &entries {
                let p = &self.productions[*pidx];
                let rhs: Vec<_> = p.rhs.iter().map(|s| match s {
                    Symbol::Terminal(t)    => t.as_str(),
                    Symbol::NonTerminal(n) => n.as_str(),
                }).collect();
                println!("  [{:12}, {:10}] -> {} -> {}", nt, tok, nt, rhs.join(" "));
            }
        }
    }
}