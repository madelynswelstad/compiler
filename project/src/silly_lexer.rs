#[derive(Debug, Clone)]
pub struct Token {
    pub category: String, // "char", "pipe", "open", etc.
    pub value: Option<char>, // actual character, only for "char" tokens
}

impl Token {
    fn meta(category: &str) -> Self {
        Token { category: category.to_string(), value: None }
    }
    fn char_tok(c: char) -> Self {
        Token { category: "char".to_string(), value: Some(c) }
    }
}

pub fn silly_lex(regex: &str, alphabet: &[char]) -> Result<Vec<(String, Option<char>)>, char> {
    let whitespace_chars: Vec<String> = alphabet.iter()
        .filter(|c| c.is_whitespace())
        .map(|c| format!("x{:02x}", *c as u8))
        .collect();

    let regex = if whitespace_chars.is_empty() {
        regex.to_string()
    } else {
        let expanded = format!("({})", whitespace_chars.join("|"));
        regex.replace("\\s", &expanded)
    };

    let mut tokens = Vec::new();
    let mut chars = regex.chars().peekable();

    while let Some(c) = chars.next() {
        let tok = match c {
            '|'  => ("pipe".to_string(),   None),
            '('  => ("open".to_string(),   None),
            ')'  => ("close".to_string(),  None),
            '*'  => ("kleene".to_string(), None),
            '+'  => ("plus".to_string(),   None),
            '.'  => ("dot".to_string(),    None),
            '-'  => ("dash".to_string(),   None),
            '\\' => {
                match chars.next() {
                    Some(next) => {
                        if !alphabet.contains(&next) { return Err(next); }
                        ("char".to_string(), Some(next))
                    }
                    None => return Err('\\'),
                }
            }
            ' ' | '\t' => continue,
            'x' => {
                let h1 = chars.next().ok_or('x')?;
                let h2 = chars.next().ok_or('x')?;
                let hex = format!("{}{}", h1, h2);
                let byte = u8::from_str_radix(&hex, 16).map_err(|_| 'x')?;
                let c = byte as char;
                if !alphabet.contains(&c) {
                    return Err(c);
                }
                ("char".to_string(), Some(c))
            }
            _ => {
                if !alphabet.contains(&c) { return Err(c); }
                ("char".to_string(), Some(c))
            }
        };
        tokens.push(tok);
    }

    Ok(tokens)
}

pub fn decode_alphabet_line(line: &str) -> Result<Vec<char>, ()> {
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