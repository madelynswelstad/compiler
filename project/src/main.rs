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

impl DFA {
    fn eps_closure(seed: impl IntoIterator<Item = i32>, nfa: &NFA) -> BTreeSet<i32> {
        let mut closure = BTreeSet::new();
        let mut stack: Vec<i32> = seed.into_iter().collect();
        while let Some(s) = stack.pop() {
            if closure.insert(s) {
                if let Some(eps) = nfa.nfa.get(&s).and_then(|t| t.get(&nfa.lambda)) {
                    for &t in eps { stack.push(t); }
                }
            }
        }
        closure
    }

    fn from_nfa(nfa: &NFA) -> Self {
        let mut dfa_trans: HashMap<BTreeSet<i32>, HashMap<char, BTreeSet<i32>>> = HashMap::new();
        let mut dfa_acc:   Vec<BTreeSet<i32>> = Vec::new();
        let mut visited:   HashSet<BTreeSet<i32>> = HashSet::new();
        let mut queue:     Vec<BTreeSet<i32>> = Vec::new();

        let start = Self::eps_closure([nfa.start_state], nfa);
        queue.push(start.clone());

        while let Some(cur) = queue.pop() {
            if !visited.insert(cur.clone()) { continue; }

            if cur.iter().any(|s| nfa.accepting.contains(s)) {
                dfa_acc.push(cur.clone());
            }

            for &sym in &nfa.alphabet {
                let targets: Vec<i32> = cur.iter()
                    .filter_map(|s| nfa.nfa.get(s)?.get(&sym))
                    .flatten().copied().collect();
                if targets.is_empty() { continue; }

                let nxt = Self::eps_closure(targets, nfa);
                dfa_trans.entry(cur.clone()).or_default().insert(sym, nxt.clone());
                if !visited.contains(&nxt) { queue.push(nxt); }
            }
        }

        let mut dfa = DFA { start, dfa: dfa_trans, accepting: dfa_acc };
        dfa.prune_dead_states();
        dfa
    }

    fn prune_dead_states(&mut self) {
        let mut reverse: HashMap<BTreeSet<i32>, Vec<BTreeSet<i32>>> = HashMap::new();
        for (from, trans) in &self.dfa {
            for to in trans.values() {
                reverse.entry(to.clone()).or_default().push(from.clone());
            }
        }

        let mut productive: HashSet<BTreeSet<i32>> = HashSet::new();
        let mut stack = self.accepting.clone();
        while let Some(s) = stack.pop() {
            if productive.insert(s.clone()) {
                for p in reverse.get(&s).into_iter().flatten() {
                    stack.push(p.clone());
                }
            }
        }

        self.dfa.retain(|s, _| productive.contains(s));
        for trans in self.dfa.values_mut() { trans.retain(|_, t| productive.contains(t)); }
        self.accepting.retain(|s| productive.contains(s));
    }
    fn merge_states_once(
        &self,
        alphabet: &[char],
    ) -> DFA {
        let mut all_states: Vec<BTreeSet<i32>> = self.dfa.keys().cloned()
            .chain(self.dfa.values().flat_map(|t| t.values().cloned()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all_states.sort(); // deterministic order

        let n = all_states.len();
        if n == 0 { 
            return DFA { start: self.start.clone(), dfa: self.dfa.clone(), accepting: self.accepting.clone() };
        }

        let idx_of: HashMap<&BTreeSet<i32>, usize> =
            all_states.iter().enumerate().map(|(i, s)| (s, i)).collect();

        let acc_set: HashSet<usize> = self.accepting.iter()
            .filter_map(|s| idx_of.get(s).copied()).collect();

        let sym_idx: HashMap<char, usize> =
            alphabet.iter().enumerate().map(|(i, &c)| (c, i)).collect();

        let mut table: Vec<Vec<Option<usize>>> = vec![vec![None; alphabet.len()]; n];
        for (s, trans) in &self.dfa {
            if let Some(&si) = idx_of.get(s) {
                for (&sym, t) in trans {
                    if let (Some(&ti), Some(&ci)) = (idx_of.get(t), sym_idx.get(&sym)) {
                        table[si][ci] = Some(ti);
                    }
                }
            }
        }

        let accepting_group: Vec<usize>     = (0..n).filter(|i| acc_set.contains(i)).collect();
        let non_accepting_group: Vec<usize> = (0..n).filter(|i| !acc_set.contains(i)).collect();

        let full_c: Vec<usize> = (0..alphabet.len()).collect();

        // M = list of groups to merge (each group is a Vec<usize> of state indices)
        let mut m: Vec<Vec<usize>> = Vec::new();
        // L = stack of (S, C)
        let mut l: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();

        if accepting_group.len() > 1 {
            l.push((accepting_group, full_c.clone()));
        }
        if non_accepting_group.len() > 1 {
            l.push((non_accepting_group, full_c.clone()));
        }

        while let Some((s_group, mut c_remaining)) = l.pop() {
            if c_remaining.is_empty() { continue; }

            // Remove one symbol c from C
            let c = c_remaining.remove(0);

            let mut buckets: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
            for &si in &s_group {
                buckets.entry(table[si][c]).or_default().push(si);
            }

            for (_, xi) in buckets {
                if xi.len() <= 1 { continue; }
                if c_remaining.is_empty() {
                    // C is now empty -> add Xi to M
                    m.push(xi);
                } else {
                    // push (Xi, remaining C) onto L
                    l.push((xi, c_remaining.clone()));
                }
            }
        }

        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x { parent[x] = find(parent, parent[x]); }
            parent[x]
        }
        fn union(parent: &mut Vec<usize>, a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb { parent[rb] = ra; }
        }

        for group in &m {
            for &si in &group[1..] {
                union(&mut parent, group[0], si);
            }
        }

        for i in 0..n { find(&mut parent, i); } // path compression pass

        let mut rep_to_set: HashMap<usize, BTreeSet<i32>> = HashMap::new();
        for (i, orig) in all_states.iter().enumerate() {
            let rep = find(&mut parent, i);
            rep_to_set.entry(rep).or_default().extend(orig.iter().copied());
        }

        // Finalise parent array: full path compression so parent[i] is always the root
        for i in 0..n { find(&mut parent, i); }

        let state_to_rep: Vec<BTreeSet<i32>> = (0..n)
            .map(|i| rep_to_set[&parent[i]].clone())
            .collect();

        // Build new transition table
        let mut new_dfa: HashMap<BTreeSet<i32>, HashMap<char, BTreeSet<i32>>> = HashMap::new();
        // Only emit one row per representative
        let mut seen_reps: HashSet<usize> = HashSet::new();
        for i in 0..n {
            let rep = parent[i];
            if !seen_reps.insert(rep) { continue; } // already emitted this merged state

            let from = state_to_rep[i].clone();
            for (ci, &sym) in alphabet.iter().enumerate() {
                if let Some(Some(target_idx)) = table.get(i).map(|row| row[ci]) {
                    let to = state_to_rep[target_idx].clone();
                    new_dfa.entry(from.clone()).or_default().insert(sym, to);
                }
            }
        }

        let new_acc: Vec<BTreeSet<i32>> = acc_set.iter()
            .map(|&i| state_to_rep[i].clone())
            .collect::<HashSet<_>>().into_iter().collect();

        let start_idx = idx_of.get(&self.start).copied().unwrap_or(0);
        let new_start = state_to_rep[start_idx].clone();

        DFA { start: new_start, dfa: new_dfa, accepting: new_acc }
    }

    fn minimize(&self, alphabet: &[char]) -> DFA {
        let mut current = DFA {
            start: self.start.clone(),
            dfa: self.dfa.clone(),
            accepting: self.accepting.clone(),
        };
        loop {
            let next = current.merge_states_once(alphabet);
            if next.dfa.len() == current.dfa.len() { return next; }
            current = next;
        }
    }

    fn write_to_file(&self, path: &str, alphabet: &[char]) -> Result<(), std::io::Error> {
        let mut file = File::create(path)?;
        let acc_set: HashSet<&BTreeSet<i32>> = self.accepting.iter().collect();

        let mut states: Vec<BTreeSet<i32>> = self.dfa.keys().cloned()
            .chain(self.dfa.values().flat_map(|t| t.values().cloned()))
            .collect::<HashSet<_>>().into_iter()
            .chain(std::iter::once(self.start.clone())) // ensure start is always present
            .collect::<HashSet<_>>().into_iter()
            .collect();
        states.sort_by(|a, b| {
            if a == &self.start { std::cmp::Ordering::Less }
            else if b == &self.start { std::cmp::Ordering::Greater }
            else { a.cmp(b) }
        });

        let id: HashMap<&BTreeSet<i32>, usize> = states.iter().enumerate().map(|(i,s)| (s,i)).collect();

        for s in &states {
            let marker = if acc_set.contains(s) { "+" } else { "-" };
            write!(file, "{} {}", marker, id[s])?;
            for &sym in alphabet {
                match self.dfa.get(s).and_then(|t| t.get(&sym)) {
                    Some(t) => write!(file, " {}", id[t])?,
                    None    => write!(file, " E")?,
                }
            }
            writeln!(file)?;
        }
        Ok(())
    }

    fn match_token(&self, token: &str) -> Result<(), usize> {
        let acc_set: HashSet<&BTreeSet<i32>> = self.accepting.iter().collect();

        // Empty string matches iff the start state itself is accepting
        if token.is_empty() {
            return if acc_set.contains(&self.start) { Ok(()) } else { Err(0) };
        }

        let mut cur = &self.start;

        for (i, ch) in token.chars().enumerate() {
            match self.dfa.get(cur).and_then(|t| t.get(&ch)) {
                Some(nxt) => cur = nxt,
                None      => return Err(i + 1),
            }
        }

        if acc_set.contains(cur) { Ok(()) } else { Err(token.chars().count() + 1) }
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