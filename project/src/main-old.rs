use std::env;
use std::fs::File;
use std::io::Write;
use std::collections::{HashMap, HashSet, BTreeSet, VecDeque};
use std::fs;


const EOF: &str = "$";

#[derive(Clone, Debug)]
enum Symbol {
    Terminal(String),
    NonTerminal(String),
}
 
#[derive(Clone, Debug)]
struct Production {
    lhs: String,
    rhs: Vec<Symbol>,
}
 
impl Production {
    fn is_epsilon(&self) -> bool {
        self.rhs.is_empty()
            || (self.rhs.len() == 1
                && matches!(&self.rhs[0], Symbol::Terminal(t) if t == "lambda"))
    }
}
 
#[derive(Debug)]
enum ParseTree {
    Leaf(String),
    Epsilon,
    Interior { symbol: String, children: Vec<ParseTree> },
}
 
impl ParseTree {
    fn pretty(&self, indent: usize) -> String {
        let pad = " ".repeat(indent * 2);
        match self {
            ParseTree::Leaf(t)    => format!("{}Leaf({})\n", pad, t),
            ParseTree::Epsilon    => format!("{}Epsilon\n", pad),
            ParseTree::Interior { symbol, children } => {
                let mut s = format!("{}Interior({})\n", pad, symbol);
                for child in children { s.push_str(&child.pretty(indent + 1)); }
                s
            }
        }
    }
}


struct NFA {
    alphabet: Vec<char>,
    lambda: char,
    start_state: i32,
    accepting: Vec<i32>,
    nfa: HashMap<i32, HashMap<char, Vec<i32>>>,
}

struct DFA {
    start: BTreeSet<i32>,
    dfa: HashMap<BTreeSet<i32>, HashMap<char, BTreeSet<i32>>>,
    accepting: Vec<BTreeSet<i32>>,
}

struct CFG {
    productions: Vec<Production>,
    start: String,
    terminals: HashSet<String>,
    non_terminals: HashSet<String>,

    // Computed properties:
    nullable: HashSet<String>,
    first_sets: HashMap<String, HashSet<String>>,
    follow_sets: HashMap<String, HashSet<String>>,
    parse_table: HashMap<String, HashMap<String, usize>>,
}

fn decode_symbol(sym: &str) -> Result<char, ()> {
    if sym.starts_with('x') && sym.len() == 3 {
        let byte = u8::from_str_radix(&sym[1..], 16).map_err(|_| ())?;
        Ok(byte as char)
    } else if sym.len() == 1 {
        Ok(sym.chars().next().unwrap())
    } else {
        Err(())
    }
}

impl NFA {
    fn file_to_tran_table(path: &str) -> Result<Self, ()> {
        let contents = fs::read_to_string(path).map_err(|_| ())?;

        let lines: Vec<Vec<&str>> = contents
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| line.split_whitespace().collect())
            .collect();

        if lines.is_empty() { return Err(()); }

        let lambda: char = decode_symbol(lines[0][1]).map_err(|_| ())?;
        let start_state: i32 = lines[1][1].parse::<i32>().map_err(|_| ())?;
        let alphabet: Vec<char> = lines[0][2..]
            .iter()
            .map(|s| decode_symbol(s))
            .collect::<Result<Vec<char>, ()>>()?;

        let mut nfa: HashMap<i32, HashMap<char, Vec<i32>>> = HashMap::new();
        let mut accepting: Vec<i32> = Vec::new();

        for line in &lines[1..] {
            let from_state = line[1].parse::<i32>().map_err(|_| ())?;
            let to_state   = line[2].parse::<i32>().map_err(|_| ())?;

            if line[0] == "+" && !accepting.contains(&from_state) {
                accepting.push(from_state);
            }

            for symbol in &line[3..] {
                let ch = decode_symbol(symbol).map_err(|_| ())?;
                nfa.entry(from_state).or_default()
                   .entry(ch).or_default()
                   .push(to_state);
            }
        }

        Ok(Self { alphabet, lambda, accepting, nfa, start_state })
    }
}


impl CFG {
    // ~ ~ ~ Load from file ~ ~ ~
 
    pub fn from_file(path: &str) -> Result<Self, String> {
        let src = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read CFG file '{}': {}", path, e))?;
        Self::from_str(&src)
    }
 
    pub fn from_str(src: &str) -> Result<Self, String> {
        let productions = Self::parse_productions(src)?;
        if productions.is_empty() { return Err("CFG file is empty".into()); }
 
        let start = productions[0].lhs.clone();
        let mut terminals:      HashSet<String> = HashSet::new();
        let mut non_terminals:  HashSet<String> = HashSet::new();
 
        for p in &productions { non_terminals.insert(p.lhs.clone()); }
        for p in &productions {
            for sym in &p.rhs {
                match sym {
                    Symbol::Terminal(t)    => { if t != "lambda" { terminals.insert(t.clone()); } }
                    Symbol::NonTerminal(n) => { non_terminals.insert(n.clone()); }
                }
            }
        }
 
        let mut cfg = CFG {
            productions, start, terminals, non_terminals,
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
 
    // ~ ~ ~ Grammar-file parser ~ ~ ~
 
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
                productions.push(Production { lhs, rhs: Self::parse_rhs(&line[1..].trim()) });
            } else {
                return Err(format!("Line {}: cannot parse '{}'", lineno + 1, line));
            }
        }
        Ok(productions)
    }
 
    fn parse_rhs(s: &str) -> Vec<Symbol> {
        s.split_whitespace().map(Self::classify).collect()
    }
 
    // ~ ~ ~ NULLABLE ~ ~ ~
 
    fn compute_nullable(&mut self) {
        loop {
            let before = self.nullable.len();
            for p in &self.productions {
                if self.nullable.contains(&p.lhs) { continue; }
                let nullable = p.rhs.is_empty() || p.rhs.iter().all(|s| match s {
                    Symbol::Terminal(t)    => t == "lambda",
                    Symbol::NonTerminal(n) => self.nullable.contains(n),
                });
                if nullable { self.nullable.insert(p.lhs.clone()); }
            }
            if self.nullable.len() == before { break; }
        }
    }
 
    // ~ ~ ~ FIRST sets ~ ~ ~
 
    /// FIRST of a sequence of grammar symbols.
    fn first_of_seq(&self, seq: &[Symbol]) -> HashSet<String> {
        let mut result = HashSet::new();
        for sym in seq {
            match sym {
                Symbol::Terminal(t) => {
                    if t != "lambda" { result.insert(t.clone()); return result; }
                    // t == "lambda": whole position is ε, continue
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
        for nt in &self.non_terminals { self.first_sets.insert(nt.clone(), HashSet::new()); }
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
 
    // ~ ~ ~ FOLLOW sets ~ ~ ~
 
    fn compute_follow(&mut self) {
        for nt in &self.non_terminals { self.follow_sets.insert(nt.clone(), HashSet::new()); }
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
 
    // ~ ~ ~ LL(1) parse table ~ ~ ~
 
    fn build_parse_table(&mut self) -> Result<(), String> {
        for nt in &self.non_terminals { self.parse_table.insert(nt.clone(), HashMap::new()); }
 
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
 
            // If rhs ⇒* ε, add for each token in FOLLOW(lhs)
            let rhs_nullable = p.is_epsilon() || p.rhs.iter().all(|s| match s {
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
 
    // ~ ~ ~ LL(1) parse ~ ~ ~
 
    pub fn parse(&self, tokens: &[String]) -> Result<ParseTree, String> {
        enum WorkItem {
            Sym(Symbol),
            Close { lhs: String, child_count: usize },
        }
 
        let mut input: VecDeque<String> = tokens.iter().cloned().collect();
        input.push_back(EOF.to_string());
 
        let mut tree_stack: Vec<Vec<ParseTree>> = vec![vec![]];
        let mut work: Vec<WorkItem> = vec![WorkItem::Sym(Symbol::NonTerminal(self.start.clone()))];
 
        while let Some(item) = work.pop() {
            match item {
                WorkItem::Close { lhs, child_count } => {
                    let frame = tree_stack.last_mut().unwrap();
                    let start = frame.len().saturating_sub(child_count);
                    let children: Vec<ParseTree> = frame.drain(start..).collect();
                    tree_stack.pop();
                    tree_stack.last_mut().unwrap()
                        .push(ParseTree::Interior { symbol: lhs, children });
                }
 
                WorkItem::Sym(sym) => {
                    let lookahead = input.front()
                        .ok_or_else(|| "Unexpected end of input".to_string())?
                        .clone();
 
                    match &sym {
                        Symbol::Terminal(t) => {
                            if t == "lambda" {
                                tree_stack.last_mut().unwrap().push(ParseTree::Epsilon);
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
 
        let mut root_frame = tree_stack.pop()
            .ok_or_else(|| "Internal error: empty tree stack".to_string())?;
        if root_frame.len() != 1 {
            return Err(format!("Internal error: root frame has {} nodes", root_frame.len()));
        }
        Ok(root_frame.remove(0))
    }
 
    pub fn parse_scan_file(&self, path: &str) -> Result<ParseTree, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read scan file '{}': {}", path, e))?;
 
        let mut tokens: Vec<String> = Vec::new();
        for (i, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if i == 0 { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            for tok in parts.iter().skip(1) {
                tokens.push(tok.to_string());
            }
        }
 
        self.parse(&tokens)
    }
 
    // ~ ~ ~ Debug helpers ~ ~ ~
 
    pub fn print_first_follow(&self) {
        let mut nts: Vec<&String> = self.non_terminals.iter().collect();
        nts.sort();
        for nt in nts {
            let first:  Vec<&String> = self.first_sets.get(nt).map_or(vec![], |s| { let mut v: Vec<_> = s.iter().collect(); v.sort(); v });
            let follow: Vec<&String> = self.follow_sets.get(nt).map_or(vec![], |s| { let mut v: Vec<_> = s.iter().collect(); v.sort(); v });
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
 
 
fn main() {
    let args: Vec<String> = env::args().collect();
 
    if args.len() < 3 {
        eprintln!("Usage:");
        eprintln!("  REC mode:   <program> REC <cfg-file> <input-lut> <output-file>");
        eprintln!("  CHECK mode: <program> CHECK <cfg-file> <input-dat> <depth>");
        std::process::exit(1);
    }
 
    let mode     = args[1].to_uppercase();
    let cfg_file = &args[2];
 
    match mode.as_str() {
        "REC" => {
            if args.len() < 5 {
                eprintln!("REC mode requires: <cfg-file> <input-lut> <output-file>");
                eprintln!("Number of arguments provided: {}", args.len());
                std::process::exit(1);
            }
            let input_file  = &args[3];
            let output_file = &args[4];
 
            let cfg = CFG::from_file(cfg_file).unwrap_or_else(|e| {
                eprintln!("CFG error: {}", e);
                std::process::exit(1);
            });
 
            let tree = cfg.parse_scan_file(input_file).unwrap_or_else(|e| {
                eprintln!("Parse error: {}", e);
                std::process::exit(1);
            });
 
            let tree_str = tree.pretty(0);
            fs::write(output_file, &tree_str).unwrap_or_else(|e| {
                eprintln!("Cannot write output file '{}': {}", output_file, e);
                std::process::exit(1);
            });
 
            println!("REC mode — parse tree written to '{}'", output_file);
        }
 
        "CHECK" => {
            println!("We are undergrads, we don't have to implement this :)");
        }
 
        _ => {
            eprintln!("Error: unknown mode '{}'. Expected REC or CHECK.", mode);
            std::process::exit(1);
        }
    }
}