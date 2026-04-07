/// A node in the parse tree produced by the LL(1) parser.
#[derive(Debug)]
pub enum ParseTree {
    /// A matched terminal token.
    Leaf(String),
    /// A lambda-production was applied here.
    Lambda,
    /// An interior node: the non-terminal that was expanded, plus its children.
    Interior {
        symbol:   String,
        children: Vec<ParseTree>,
    },
}

impl ParseTree {
    /// Recursively pretty-print the tree with 2-space indentation per level.
    pub fn pretty(&self, indent: usize) -> String {
        let pad = " ".repeat(indent * 2);
        match self {
            ParseTree::Leaf(t) => format!("{}Leaf({})\n", pad, t),
            ParseTree::Lambda => format!("{}Lambda\n", pad),
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