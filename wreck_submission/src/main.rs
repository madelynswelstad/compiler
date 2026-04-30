use std::env;
use std::fs;
use std::path::Path;
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

    if args.len() != 5 {
        eprintln!("Usage: {} REC <llre.cfg> <scan.lut> <output_file>", args[0]);
        process::exit(1);
    }

    let cfg_file   = &args[2];
    let scan_file  = &args[3];
    let output_file = &args[4];

    // // Ensure output directory exists
    // fs::create_dir_all(output_file).unwrap_or_else(|e| {
    //     eprintln!("Cannot create output directory '{}': {}", output_file, e);
    //     process::exit(1);
    // });

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

    let cfg = CFG::from_file(cfg_file).unwrap_or_else(|e| {
        eprintln!("CFG error: {}", e);
        process::exit(1);
    });

    // Pick lambda char — first non-printable not in alphabet
    let lambda_char = (0u8..=31u8)
        .map(|b| b as char)
        .find(|c| !alphabet.contains(c))
        .unwrap_or_else(|| {
            eprintln!("No lambda character available");
            process::exit(1);
        });

    // Remaining lines: first column is regex, second is token id, optional third is data
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }

        let regex_string = parts[0];
        let token_id     = parts[1];

        let re_tokens = silly_lexer::silly_lex(regex_string, &alphabet).unwrap_or_else(|bad_char| {
            eprintln!("Lexical error: character '{}' not in alphabet", bad_char);
            process::exit(4);
        });

        let categories: Vec<String> = re_tokens.iter().map(|t| t.0.clone()).collect();

        let tree = cfg.parse(&categories).unwrap_or_else(|e| {
            eprintln!("Syntax error in regex '{}': {}", regex_string, e);
            process::exit(2);
        });

        let mut idx = 0;
        let ast = match build_ast(&tree, &re_tokens, &mut idx, &alphabet) {
            Ok(a) => a,
            Err(e) if e.starts_with("LEX:") => {
                eprintln!("Lexical error: {}", &e[4..]);
                process::exit(4);
            }
            Err(e) if e.starts_with("SEM:") => {
                eprintln!("Semantic error: {}", &e[4..]);
                process::exit(3);
            }
            Err(e) => {
                eprintln!("Internal error: {}", e);
                process::exit(1);
            }
        };

        let nfa = nfa::NFA::from_ast(&ast, alphabet.clone());
        let l   = nfa.build_lambda_matrix();
        let t   = nfa.build_transition_table();

        nfa::print_lambda_matrix(&l);
        nfa::print_transition_table(&t);

        // let nfa_filename = format!("{}.tt", token_id);
        let nfa_filename = format!("{}.nfa", token_id);
        nfa.write_to_file(&nfa_filename, lambda_char).unwrap_or_else(|e| {
            eprintln!("Error writing NFA '{}': {}", nfa_filename, e);
            process::exit(1);
        });
    }

    // Write scan.u into the output directory
    // let scan_u_path = Path::new(output_file).join(output_file);
    // let mut scan_u = fs::File::create(&scan_u_path).unwrap_or_else(|e| {
    //     eprintln!("Error creating '{}': {}", scan_u_path.display(), e);
    //     process::exit(1);
    // });
    let scan_u_path = output_file;
    let mut scan_u = fs::File::create(scan_u_path).unwrap_or_else(|e| {
        eprintln!("Error creating '{}': {}", scan_u_path, e);
        process::exit(1);
    });
    use std::io::Write;

    // Line 1: alphabet
    let alphabet_str: Vec<String> = alphabet.iter().map(|&c| {
        if (c as u8) < 32 || c == 'x' || c == '\\' || c == ':' || c.is_whitespace() {
            format!("x{:02x}", c as u8)
        } else {
            c.to_string()
        }
    }).collect();
    writeln!(scan_u, "{}", alphabet_str.join(" ")).unwrap();

    // One line per token
    for line in scan_content.lines().filter(|l| !l.trim().is_empty()).skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let token_id = parts[1];
        let category = parts.get(2).copied();

        if let Some(cat) = category {
            writeln!(scan_u, "{}.tt\t\t\t{}\t\t{}", token_id, token_id, cat).unwrap();
        } else {
            writeln!(scan_u, "{}.tt\t\t\t{}", token_id, token_id).unwrap();
        }
    }


}