use std::env;
use std::process::exit;
use std::io::{Write};
use slug::slugify;

fn print_usage_and_exit() {
    eprintln!("Missing an argument.");
    eprintln!("Usage: transform lowercase|uppercase|no-spaces|slugify");

    exit(1);
}

fn slugify_keep_newlines(s: String) -> String {
    if s.chars().last() == Some('\n') {
        return slugify(s) + "\n"
    } else {
        return s
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        print_usage_and_exit()
    }

    let command = args[1].as_str();

    let transformation: fn(String) -> String = match command {
        "lowercase" => |s| s.to_lowercase(),
        "uppercase" => |s| s.to_uppercase(),
        "no-spaces" => |s| s.replace(" ", ""),
        "slugify" => |s| slugify_keep_newlines(s),
        _ => {
            print_usage_and_exit();
            |s| s // this will never happen because program exists
        }
    };

    
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
 
    loop {
        let mut buf = String::new();

        let result_read = stdin.read_line(&mut buf);
        match result_read {
            Ok(0) => exit(0),
            Ok(_) => {
                let transformed = (transformation)(buf);
        
                if stdout.write_all(transformed.as_bytes()).and(stdout.flush()).is_err() {
                    eprintln!("Could not write to stdout.");
                    exit(1)
                }
            },
            Err(_) => {
                eprintln!("Could not read from stdin.");
                exit(1)
            }
        }
    }
}