use std::env;
use std::fs;
use std::process;
mod cfg;
mod ast;
mod nfa;
mod silly_lexer;
mod grammar;

use cfg::CFG;
use ast::build_ast;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} REC <llre.cfg> <scan.lut>", args[0]);
        process::exit(1);
    }

    let cfg_file = &args[2];
    let scan_file = &args[3];

    let scan_content = fs::read_to_string(scan_file).unwrap_or_else(|e| {
        eprintln!("Cannot read scan file '{}': {}", scan_file, e);
        process::exit(1);
    });

    let mut lines = scan_content.lines().filter(|l| !l.trim().is_empty());

    // First line is the alphabet
    let alphabet_line = lines.next().unwrap_or_else(|| {
        eprintln!("scan file is empty");
        process::exit(1);
    });

    let alphabet = silly_lexer::decode_alphabet_line(alphabet_line).unwrap_or_else(|_| {
        eprintln!("Failed to parse alphabet");
        process::exit(1);
    });

    let cfg = CFG::from_file("llre.cfg").unwrap_or_else(|e| {
        eprintln!("CFG error: {}", e);
        process::exit(1);
    });

    // Remaining lines: first column is regex, second is token id, optional third is data
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();

        let regex_string = parts[0];
        let token_id = parts[1];
        let data = parts.get(2).copied(); // optional

        let re_tokens = silly_lexer::silly_lex(regex_string, &alphabet).unwrap_or_else(|bad_char| {
            eprintln!("Lexical error: character '{}' not in alphabet", bad_char);
            process::exit(4);
        });

        // feed just categories to the LL(1) parser
        let categories: Vec<String> = re_tokens.iter().map(|t| t.category.clone()).collect();
        let tree = cfg.parse(&categories).unwrap_or_else(|e| {
            eprintln!("Syntax error in regex '{}': {}", regex_string, e);
            process::exit(2);
        });

        let ast = build_ast(&tree).unwrap_or_else(|e| {
            eprintln!("AST error: {}", e);
            process::exit(1);
        });

        println!("AST: {:#?}", ast);

        // let nfa  = nfa::NFA::from_ast(&ast);
        // nfa.write_to_file(...);
    }
}