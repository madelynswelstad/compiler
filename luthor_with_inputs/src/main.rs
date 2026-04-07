use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;



// data structures
struct Alphabet {
    symbols: Vec<char>,
    index: HashMap<char, usize>,
}

struct DFA {
    token_id: String,
    constant_value: Option<String>,
    transitions: Vec<Vec<Option<usize>>>,
    accepting: Vec<bool>,
}

struct Scanner {
    alphabet: Alphabet,
    dfas: Vec<DFA>,
}


// encoding helpers
fn must_encode(c: char) -> bool {
    c == 'x' || c == '\\' || c == ':' || c.is_whitespace()
}

fn encode_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if must_encode(c) {
        out.push_str(&format!("x{:02x}", c as u8));
        } else {
            out.push(c);
        }
    }
    out
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


// parsing scan.u
impl Alphabet {
    fn from_line(line: &str) -> Result<Self, ()> {
        let symbols = decode_alphabet_line(line)?;

        let mut index = HashMap::new();
        for (i, c) in symbols.iter().enumerate() {
            index.insert(*c, i);
        }

        Ok(Alphabet { symbols, index })
    }
}

impl DFA {
    fn from_definition(
        line: &str,
        alpha: &Alphabet,
    ) -> Result<Self, ()> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(());
        }

        let table_path = PathBuf::from(parts[0]);
        let token_id = parts[1].to_string();
        let constant_value = parts.get(2).map(|s| s.to_string());

        let table = fs::read_to_string(&table_path)
            .map_err(|e| {
                eprintln!("read error: {}", e);
                ()
            })?;

        let mut transitions = Vec::new();
        let mut accepting = Vec::new();

        for line in table.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() {
                continue;
            }

            let is_accepting = fields[0] == "+";
            accepting.push(is_accepting);

            let mut row = Vec::new();
            for f in &fields[2..] {
                if *f == "E" {
                row.push(None);
                } else {
                row.push(Some(f.parse::<usize>().map_err(|_| ())?));
                }
            }

            if row.len() != alpha.symbols.len() {
                return Err(());
            }
            transitions.push(row);
        }

        Ok(DFA {
        token_id,
        constant_value,
        transitions,
        accepting,
        })
    }
}

fn decode_alphabet_line(line: &str) -> Result<Vec<char>, ()> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut symbols = Vec::new();
    
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if bytes[i] == b'x' {
            if i + 2 >= bytes.len() {
                return Err(());
            }

            let hex = std::str::from_utf8(&bytes[i+1..i+3]).map_err(|_| ())?;
            let byte = u8::from_str_radix(hex, 16).map_err(|_| ())?;
            symbols.push(byte as char);
            i += 3;
        } else {
            symbols.push(bytes[i] as char);
            i += 1;
        }
    }

    Ok(symbols)
}

impl Scanner {
    fn from_scan_def(path: &str) -> Result<Self, ()> {
        let contents = fs::read_to_string(path).map_err(|_| ())?;

        let lines: Vec<&str> = contents
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();

        if lines.is_empty() {
            return Err(());   // EXIT STATUS 1
        }

        let alphabet = Alphabet::from_line(lines[0])?;
        let mut dfas = Vec::new();

        for line in &lines[1..] {
            dfas.push(DFA::from_definition(line, &alphabet)?);
        }

        Ok(Scanner { alphabet, dfas })
    }
}

// scanning
impl Scanner {
    fn scan(&self, src: &str, out: &str) -> Result<(), ()> {
        let input = fs::read_to_string(src).map_err(|_| ())?;
        let mut output = File::create(out).map_err(|_| ())?;

        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        let mut line = 1;
        let mut col = 1;

        while i < chars.len() {
        let start_i = i;
        let start_line = line;
        let start_col = col;

        let mut states: Vec<Option<usize>> = vec![Some(0); self.dfas.len()];
        let mut longest: Vec<Option<usize>> = vec![None; self.dfas.len()];

        let mut j = i;
        let mut temp_line = line;
        let mut temp_col = col;

        while j < chars.len() {
            let c = chars[j];
            let idx = match self.alphabet.index.get(&c) {
            Some(i) => *i,
            None => return Err(()),
            };

            let mut any_alive = false;

            for (k, dfa) in self.dfas.iter().enumerate() {
            if let Some(state) = states[k] {
                if let Some(next) = dfa.transitions[state][idx] {
                states[k] = Some(next);
                any_alive = true;
                if dfa.accepting[next] {
                    longest[k] = Some(j - start_i + 1);
                }
                } else {
                states[k] = None;
                }
            }
            }

            if !any_alive {
            break;
            }

            if c == '\n' {
            temp_line += 1;
            temp_col = 1;
            } else {
            temp_col += 1;
            }

            j += 1;
        }

        // Find longest match length first
        let mut best_len = 0;
        for l in longest.iter() {
            if let Some(len) = l {
                if *len > best_len {
                    best_len = *len;
                }
            }
        }

        // Find first DFA that matches this length (tie-breaking rule)
        let mut best = None;
        for (k, l) in longest.iter().enumerate() {
            if let Some(len) = l {
                if *len == best_len {
                    best = Some(k);
                    break;  // Take first DFA with longest match
                }
            }
        }

        let winner = match best {
            Some(w) => w,
            None => return Err(()),
        };

        let dfa = &self.dfas[winner];
        let lexeme: String = chars[i..i + best_len].iter().collect();

        let value = match &dfa.constant_value {
            Some(v) => v.clone(),
            None => encode_string(&lexeme),
        };

        writeln!(
            output,
            "{} {} {} {}",
            dfa.token_id, value, start_line, start_col
        ).map_err(|_| ())?;


        for c in chars[i..i + best_len].iter() {
            if *c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        i += best_len;
        }

        Ok(())
    }
}


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: LUTHOR <scan.u> <program.src> <tokens.dat>");
        std::process::exit(1);
    }

    let scan_u = &args[1];
    let src = &args[2];
    let out = &args[3];

    let scanner = Scanner::from_scan_def(scan_u).unwrap_or_else(|_| {
        std::process::exit(1);
    });

    if scanner.scan(src, out).is_err() {
        std::process::exit(1);
    }

}