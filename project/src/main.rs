use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;

fn main() {
  let args : Vec<String> = env::args().collect();
  if args.len() < 2{
    eprintln!("Usage: ALPHABETENCODING <encode|decode>");
    std::process::exit(1);
  }

  let code = &args[1];
  match code.as_str() {
    "encode" => {
      if args.len() != 3 {
        eprintln!("Usage: ALPHABETENCODING encode <text_to_encode>");
        std::process::exit(1);
      }

      encode(&args[2]);
    },
  
    "decode" => {
      if args.len() != 4 {
        eprintln!("Usage: ALPHABETENCODING decode <text_to_decode> <output_file>");
        std::process::exit(1);
      }

      match decode(&args[2], &args[3]) {
        Ok(decoded) => println!("{}", decoded),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
      }
    },    

    _ => {
      eprintln!("Invalid mode. First argument must be 'encode' or 'decode'.");
      std::process::exit(1);
    }
  }
}

fn encode(input_file: &str) {  
  let input = fs::read_to_string(input_file)
    .expect("Failed to read input file");

  print!("OUTPUT ");

  for c in input.chars() {
    if must_encode(c) {
      print!("x{:02x}", c as u8);
    } else {
      print!("{}", c);
    }
  }
}

fn must_encode(c: char) -> bool {
  c == 'x' || c == '\\' || c == ':' || c.is_whitespace() // || !c.is_printable()
}

fn decode(encoded_text: &str, output_file: &str) -> Result<String, &'static str> {
  print!("OUTPUT ");

  let mut output_file = File::create(output_file).expect("Cannot create output file");

  let bytes = encoded_text.as_bytes();
  let mut i = 0;
  let mut output = String::new();

  while i < bytes.len() {
    // Check for xHH
    if bytes[i] == b'x' && i + 2 < bytes.len() && bytes[i+1].is_ascii_hexdigit() && bytes[i+2].is_ascii_hexdigit() {
      let hex = &encoded_text[i+1 .. i+3];
      let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| "Invalid hex sequence")?;      
      output.push(byte as char);
      i += 3;
    } else {
      output.push(bytes[i] as char);
      i += 1; 
    }
  }
  output_file.write_all(output.as_bytes()).expect("Failed to write output");

  Ok(output)
}