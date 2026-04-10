/// A node in the parse tree produced by the LL(1) parser.
#[derive(Debug)]
pub enum ParseTree {
    /// A matched terminal token.
    Leaf(String),
    /// An ε-production was applied here.
    Epsilon,
    /// An interior node: the non-terminal that was expanded, plus its children.
    Interior {
        symbol:   String,
        children: Vec<ParseTree>,
    },
}

#[derive(Debug, Clone)]
pub enum AstNode {
    Alt(Box<AstNode>, Box<AstNode>),
    Seq(Box<AstNode>, Box<AstNode>),
    Star(Box<AstNode>),
    Plus(Box<AstNode>),
    Range(char, char),
    Ch(char),
    Dot,
    Lambda,
}

impl ParseTree {
    /// Recursively pretty-print the tree with 2-space indentation per level.
    pub fn pretty(&self, indent: usize) -> String {
        let pad = " ".repeat(indent * 2);
        match self {
            ParseTree::Leaf(t) => format!("{}Leaf({})\n", pad, t),
            ParseTree::Epsilon => format!("{}Epsilon\n", pad),
            ParseTree::Interior { symbol, children } => {
                let mut s = format!("{}Interior({})\n", pad, symbol);
                for child in children {
                    s.push_str(&child.pretty(indent + 1));
                }
                s
            }
        }
    }
}

pub fn build_ast(tree: &ParseTree, tokens: &[(String, Option<char>)], idx: &mut usize) -> Result<AstNode, String> {
    match tree {
        ParseTree::Interior { symbol, children } => {
            match symbol.as_str() {
                "RE"      => build_ast(&children[0], tokens, idx),
                "ALT"     => build_alt(children, tokens, idx),
                "SEQ"     => build_seq(children, tokens, idx),
                "ATOM"    => build_atom(children, tokens, idx),
                "NUCLEUS" => build_nucleus(children, tokens, idx),
                _ => Err(format!("unexpected symbol {}", symbol)),
            }
        }
        ParseTree::Leaf(t)  => Err(format!("unexpected leaf {}", t)),
        ParseTree::Epsilon  => Ok(AstNode::Lambda),
    }
}

fn build_alt(children: &[ParseTree], tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    let seq = build_seq(match &children[0] {
        ParseTree::Interior { children, .. } => children,
        _ => return Err("ALT: expected SEQ".into()),
    }, tokens, idx, alphabet)?;
    build_altlist(seq, &children[1], tokens, idx, alphabet)
}

fn build_altlist(left: AstNode, tree: &ParseTree, tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(left),
        ParseTree::Interior { symbol, children } if symbol == "ALTLIST" => {
            match children.as_slice() {
                [] => Ok(left),
                [ParseTree::Epsilon] => Ok(left),
                [_, seq, altlist] => {
                    *idx += 1; // consume pipe
                    let right = build_seq(match seq {
                        ParseTree::Interior { children, .. } => children,
                        _ => return Err("ALTLIST: expected SEQ".into()),
                    }, tokens, idx, alphabet)?;
                    let node = AstNode::Alt(Box::new(left), Box::new(right));
                    build_altlist(node, altlist, tokens, idx, alphabet)
                }
                _ => Err("ALTLIST: unexpected shape".into()),
            }
        }
        _ => Ok(left),
    }
}

fn build_seq(children: &[ParseTree], tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match children {
        [] | [ParseTree::Epsilon] => Ok(AstNode::Lambda),
        [atom, seqlist] => {
            let left = build_atom(match atom {
                ParseTree::Interior { children, .. } => children,
                _ => return Err("SEQ: expected ATOM".into()),
            }, tokens, idx, alphabet)?;
            build_seqlist(left, seqlist, tokens, idx, alphabet)
        }
        _ => Err(format!("SEQ: unexpected shape: {:?}", children)),
    }
}

fn build_seqlist(left: AstNode, tree: &ParseTree, tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(left),
        ParseTree::Interior { symbol, children } if symbol == "SEQLIST" => {
            match children.as_slice() {
                [] | [ParseTree::Epsilon] => Ok(left),
                [atom, seqlist] => {
                    let right = build_atom(match atom {
                        ParseTree::Interior { children, .. } => children,
                        _ => return Err("SEQLIST: expected ATOM".into()),
                    }, tokens, idx, alphabet)?;
                    let node = AstNode::Seq(Box::new(left), Box::new(right));
                    build_seqlist(node, seqlist, tokens, idx, alphabet)
                }
                _ => Err(format!("SEQLIST: unexpected shape: {:?}", children)),
            }
        }
        _ => Ok(left),
    }
}

fn build_atom(children: &[ParseTree], tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match children {
        [nucleus, atommod] => {
            let inner = build_nucleus(match nucleus {
                ParseTree::Interior { children, .. } => children,
                _ => return Err("ATOM: expected NUCLEUS".into()),
            }, tokens, idx, alphabet)?;
            build_atommod(inner, atommod, tokens, idx)
        }
        _ => Err("ATOM: unexpected shape".into()),
    }
}
fn build_atommod(inner: AstNode, tree: &ParseTree, tokens: &[(String, Option<char>)], idx: &mut usize) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(inner),
        ParseTree::Interior { symbol, children } if symbol == "ATOMMOD" => {
            match children.as_slice() {
                [ParseTree::Leaf(t)] if t == "kleene" => {
                    *idx += 1; // consume kleene
                    Ok(AstNode::Star(Box::new(inner)))
                }
                [ParseTree::Leaf(t)] if t == "plus" => {
                    *idx += 1; // consume plus
                    Ok(AstNode::Plus(Box::new(inner)))
                }
                _ => Ok(inner),
            }
        }
        _ => Ok(inner),
    }
}

fn build_nucleus(children: &[ParseTree], tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match children {
        [ParseTree::Leaf(t)] if t == "dot" => {
            *idx += 1; // consume dot
            Ok(AstNode::Dot)
        }
        [ParseTree::Leaf(o), alt, ParseTree::Leaf(c)]
            if o == "open" && c == "close" =>
        {
            *idx += 1; // consume open
            let inner = build_ast(alt, tokens, idx, alphabet)?;
            *idx += 1; // consume close
            Ok(inner)
        }
        [ParseTree::Leaf(ch), charrng] if ch == "char" => {
            let c = tokens[*idx].1.ok_or("char token has no value")?;
            *idx += 1; // consume char
            build_charrng(c, charrng, tokens, idx, alphabet)
        }
        _ => Err("NUCLEUS: unexpected shape".into()),
    }
}


fn build_charrng(left_char: char, tree: &ParseTree, tokens: &[(String, Option<char>)], idx: &mut usize, alphabet: &[char]) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(AstNode::Ch(left_char)),
        ParseTree::Interior { symbol, children } if symbol == "CHARRNG" => {
            match children.as_slice() {
                [ParseTree::Leaf(d), ParseTree::Leaf(r)]
                    if d == "dash" && r == "char" =>
                {
                    *idx += 1; // consume dash
                    let right_char = tokens[*idx].1.ok_or("char token has no value")?;
                    *idx += 1; // consume char

                    if left_char > right_char {
                        return Err(format!(
                            "SEM:range '{}-{}' is invalid: start must be <= end",
                            left_char, right_char
                        ));
                    }

                    Ok(AstNode::Range(left_char, right_char))
                }
                _ => Ok(AstNode::Ch(left_char)),
            }
        }
        _ => Ok(AstNode::Ch(left_char)),
    }
}