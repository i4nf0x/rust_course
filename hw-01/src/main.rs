use std::{env, io::{self, Write}, path::Path};

fn get_program_name() -> Result<String, ()> {
    let prog_path = env::args().next().ok_or(())?;
    let prog_path = Path::new(&prog_path);

    let filename = prog_path.file_name().ok_or(())?;
    let filename = filename.to_str().ok_or(())?;
    Ok(filename.to_string())
}

fn main() {
    let progname = get_program_name().unwrap_or("<unknown>".to_string());

    println!("Hello! My name is {}. What's your name?", progname);
    print!("> ");
    let _ = io::stdout().flush();

    let mut username: String = String::new();
    
    let username_valid = io::stdin().read_line(&mut username)
        .is_ok_and(|_| !username.trim().is_empty());
    
    if username_valid {
        println!("Nice to meet you, {}!", username.trim());
    } else {
        println!("Hmmmpff. Then keep your name to yourself!")
    }

}
