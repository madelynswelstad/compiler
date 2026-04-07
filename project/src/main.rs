use std::env;
use std::fs::File;
use std::io::Write;
use std::collections::{HashMap, HashSet, BTreeSet};
use std::fs;

// Proof that i can push here -Thomas

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

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: REC <cfg-file> <dfa-output-file> [token1] [token2 ...]");
        std::process::exit(1);
    }

    let nfa = NFA::file_to_tran_table(&args[1]).unwrap_or_else(|_| std::process::exit(1));
    let dfa = DFA::from_nfa(&nfa);
    let mut min = dfa.minimize(&nfa.alphabet);
    min.prune_dead_states(); // re-prune after merging in case any edges became dead

    min.write_to_file(&args[2], &nfa.alphabet).unwrap_or_else(|e| {
        eprintln!("Error writing DFA: {}", e);
        std::process::exit(1);
    });

    for token in &args[3..] {
        match min.match_token(token) {
            Ok(())  => println!("OUTPUT :M:"),
            Err(n)  => println!("OUTPUT {}", n),
        }
    }
}