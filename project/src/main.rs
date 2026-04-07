use std::env;
use std::fs;
use std::process;

mod ast;
mod cfg;
mod grammar;
mod nfa;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} REC <llre.cfg> <scan.lut>", args[0]);
        process::exit(1);
    }

    let cfg_file = &args[2];
    let scan_file = &args[3];

    // 1. Load regex grammar from llre.cfg
    let grammar = cfg::CFG::from_file(cfg_file).unwrap_or_else(|e| {
        eprintln!("CFG error: {}", e);
        process::exit(1);
    });

    // 2. Parse the scanner output into a parse tree 
    let tree = grammar.parse_scan_file(scan_file).unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        process::exit(1);
    });

    let tree_str = tree.pretty(0);
        fs::write("output_file", &tree_str).unwrap_or_else(|e| {
        eprintln!("Cannot write output file '{}': {}", "output_file", e);
        std::process::exit(1);
    });

    // // 3. Convert the parse tree to an NFA 
    // let nfa = nfa::NFA::from_ast(&tree).unwrap_or_else(|e| {
    //     eprintln!("NFA construction error: {}", e);
    //     process::exit(1);
    // });

    // 4. Write outputs 
    // ...
}