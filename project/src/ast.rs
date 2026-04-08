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

pub fn build_ast(tree: &ParseTree) -> Result<AstNode, String> {
    match tree {
        ParseTree::Interior { symbol, children } => {
            match symbol.as_str() {
                "RE"      => build_ast(&children[0]), // unwrap to ALT
                "ALT"     => build_alt(children),
                "SEQ"     => build_seq(children),
                "ATOM"    => build_atom(children),
                "NUCLEUS" => build_nucleus(children),
                _ => Err(format!("unexpected symbol {}", symbol)),
            }
        }
        ParseTree::Leaf(t)  => Err(format!("unexpected leaf {}", t)),
        ParseTree::Epsilon  => Ok(AstNode::Lambda),
    }
}

// ALT -> SEQ ALTLIST
fn build_alt(children: &[ParseTree]) -> Result<AstNode, String> {
    let seq = build_seq(match &children[0] {
        ParseTree::Interior { children, .. } => children,
        _ => return Err("ALT: expected SEQ".into()),
    })?;
    build_altlist(seq, &children[1])
}

// ALTLIST -> pipe SEQ ALTLIST | lambda
fn build_altlist(left: AstNode, tree: &ParseTree) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(left),
        ParseTree::Interior { symbol, children } if symbol == "ALTLIST" => {
            match children.as_slice() {
                // lambda production
                [] => Ok(left),
                [ParseTree::Epsilon] => Ok(left),
                // pipe SEQ ALTLIST
                [_, seq, altlist] => {
                    let right = build_seq(match seq {
                        ParseTree::Interior { children, .. } => children,
                        _ => return Err("ALTLIST: expected SEQ".into()),
                    })?;
                    let node = AstNode::Alt(Box::new(left), Box::new(right));
                    build_altlist(node, altlist)
                }
                _ => Err("ALTLIST: unexpected shape".into()),
            }
        }
        _ => Ok(left),
    }
}

// SEQ -> ATOM SEQLIST | lambda
fn build_seq(children: &[ParseTree]) -> Result<AstNode, String> {
    match children {
        [] => Ok(AstNode::Lambda),
        [ParseTree::Epsilon] => Ok(AstNode::Lambda),
        [atom, seqlist] => {
            let left = build_atom(match atom {
                ParseTree::Interior { children, .. } => children,
                _ => return Err("SEQ: expected ATOM".into()),
            })?;
            build_seqlist(left, seqlist)
        }
        _ => Err("SEQ: unexpected shape".into()),
    }
}

// SEQLIST -> ATOM SEQLIST | lambda
fn build_seqlist(left: AstNode, tree: &ParseTree) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(left),
        ParseTree::Interior { symbol, children } if symbol == "SEQLIST" => {
            match children.as_slice() {
                [] => Ok(left),
                [ParseTree::Epsilon] => Ok(left),
                [atom, seqlist] => {
                    let right = build_atom(match atom {
                        ParseTree::Interior { children, .. } => children,
                        _ => return Err("SEQLIST: expected ATOM".into()),
                    })?;
                    let node = AstNode::Seq(Box::new(left), Box::new(right));
                    build_seqlist(node, seqlist)
                }
                _ => Err("SEQLIST: unexpected shape".into()),
            }
        }
        _ => Ok(left),
    }
}

// ATOM -> NUCLEUS ATOMMOD
fn build_atom(children: &[ParseTree]) -> Result<AstNode, String> {
    match children {
        [nucleus, atommod] => {
            let inner = build_nucleus(match nucleus {
                ParseTree::Interior { children, .. } => children,
                _ => return Err("ATOM: expected NUCLEUS".into()),
            })?;
            build_atommod(inner, atommod)
        }
        _ => Err("ATOM: unexpected shape".into()),
    }
}

// ATOMMOD -> kleene | plus | lambda
fn build_atommod(inner: AstNode, tree: &ParseTree) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(inner),
        ParseTree::Interior { symbol, children } if symbol == "ATOMMOD" => {
            match children.as_slice() {
                [ParseTree::Leaf(t)] if t == "kleene" => Ok(AstNode::Star(Box::new(inner))),
                [ParseTree::Leaf(t)] if t == "plus"   => Ok(AstNode::Plus(Box::new(inner))),
                _ => Ok(inner), // lambda
            }
        }
        _ => Ok(inner),
    }
}

// NUCLEUS -> open ALT close | char CHARRNG | dot
fn build_nucleus(children: &[ParseTree]) -> Result<AstNode, String> {
    match children {
        // dot
        [ParseTree::Leaf(t)] if t == "dot" => Ok(AstNode::Dot),

        // open ALT close
        [ParseTree::Leaf(o), alt, ParseTree::Leaf(c)]
            if o == "open" && c == "close" =>
        {
            build_ast(alt)
        }

        // char CHARRNG
        [ParseTree::Leaf(ch), charrng] if ch == "char" => {
            // TODO: we need the actual char value here, not just "char"
            // this will need updating when we thread char values through
            build_charrng('?', charrng)
        }

        _ => Err("NUCLEUS: unexpected shape".into()),
    }
}

// CHARRNG -> dash char | lambda
fn build_charrng(left_char: char, tree: &ParseTree) -> Result<AstNode, String> {
    match tree {
        ParseTree::Epsilon => Ok(AstNode::Ch(left_char)),
        ParseTree::Interior { symbol, children } if symbol == "CHARRNG" => {
            match children.as_slice() {
                [ParseTree::Leaf(d), ParseTree::Leaf(r)]
                    if d == "dash" && r == "char" =>
                {
                    // TODO: need actual char value for range end too
                    Ok(AstNode::Range(left_char, '?'))
                }
                _ => Ok(AstNode::Ch(left_char)),
            }
        }
        _ => Ok(AstNode::Ch(left_char)),
    }
}