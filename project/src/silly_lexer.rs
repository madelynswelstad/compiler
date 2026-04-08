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

pub fn silly_lex(regex: &str, alphabet: &[char]) -> Result<Vec<Token>, char> {
    let regex = regex.replace("\\s", "(x20|x0a)");
    
    let mut tokens = Vec::new();
    let mut chars = regex.chars().peekable();

    while let Some(c) = chars.next() {
        let tok = match c {
            '|'  => Token::meta("pipe"),
            '('  => Token::meta("open"),
            ')'  => Token::meta("close"),
            '*'  => Token::meta("kleene"),
            '+'  => Token::meta("plus"),
            '.'  => Token::meta("dot"),
            '-'  => Token::meta("dash"),
            '\\' => {
                match chars.next() {
                    Some(next) => {
                        if !alphabet.contains(&next) {
                            return Err(next); // exit code 4
                        }
                        Token::char_tok(next)
                    }
                    None => return Err('\\'),
                }
            }
            ' ' | '\t' => continue,
            'x' => {
                // consume next two hex digits
                let h1 = chars.next().ok_or('x')?;
                let h2 = chars.next().ok_or('x')?;
                let hex = format!("{}{}", h1, h2);
                let byte = u8::from_str_radix(&hex, 16).map_err(|_| 'x')?;
                let c = byte as char;
                if !alphabet.contains(&c) {
                    return Err(c);
                }
                Token::char_tok(c)
            }
            _ => {
                if !alphabet.contains(&c) {
                    return Err(c); // exit code 4
                }
                Token::char_tok(c)
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