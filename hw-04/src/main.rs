use str_transform::{csv_pretty, StrTransformMessage, StrTransformOperation};
use std::error::Error;
use std::process::exit;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{str::FromStr, thread};

mod str_transform;


#[derive(Debug,thiserror::Error)]
pub enum CommandReadingError {
    #[error("Invalid input")]
    InvalidInput
}

fn input_loop(tx: Sender<StrTransformMessage>) -> Result<(), Box<dyn Error>> {
    loop {
        let mut buf: String = String::new();
        let cmd_args = read_command_and_args(&mut buf)?;
        match cmd_args {
            Some((cmd, args)) => {
                let operation = StrTransformOperation::from_str(cmd);
                match operation {
                    Ok(operation) => {
                        let msg = StrTransformMessage::new(operation, args.to_string());
                        tx.send(msg)?;
                    },
                    Err(e) => {
                        eprintln!("{e}");
                    }
                }

            }
            None => {
                break; // end of input
            }
        }
    }
    Ok(())
}

fn read_command_and_args(buf: &mut String) -> Result<Option<(&str,&str)>, Box<dyn Error>> {
    let len = std::io::stdin().read_line(buf)?;
    if len == 0 {
        Ok(None)
    } else {
        let (command, args) = buf.split_once(' ').ok_or(CommandReadingError::InvalidInput)?;
        Ok(Some((command, args)))
    }
}

fn processing_loop(rx: Receiver<StrTransformMessage>) ->  Result<(), Box<dyn Error>> {
    loop {
        match rx.recv() {
            Err(_) => {
                // the sender has closed our channel, not our error, break the loop
                return Ok(())
            }
            Ok(msg) => {
                match msg.operation.perform(&msg.args) {
                    Err(e) => eprintln!("{e}"),
                    Ok(output) => {
                        println!("{output}");
                    }
                }
            }
        }
    }
}

fn run_threaded() {
    let (tx, rx) = mpsc::channel::<StrTransformMessage>();

    let processing_thread = thread::spawn(move || {
        if let Err(e) = processing_loop(rx) {
            eprintln!("Processing thread: {e}");
        }
    });

    let input_thread = thread::spawn(move || {
        if let Err(e) = input_loop(tx) {
            eprintln!("Input thread: {e}");
        }
    });

    input_thread.join().unwrap();
    processing_thread.join().unwrap();
}

fn run_oneshot(command: &str) -> Result<(), Box<dyn Error>> {
    let operation = StrTransformOperation::from_str(command)?;

    match operation {
        StrTransformOperation::Csv => {
            println!("{}",csv_pretty::render_csv(std::io::stdin())?);
            Ok(())
        },
        _ => {
            let mut buf = String::new();
            let bytes_read = std::io::stdin().read_line(&mut buf)?;
            if bytes_read > 0 {
                println!("{}", operation.perform(&buf)?)
            }
            Ok(())
        }
    }
}
    

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.len() {
        1 => run_threaded(),
        2 => {
            if let Err(e) = run_oneshot(&args[1]) {
                eprintln!("Error: {e}");
                print_usage_and_exit();
            }
        },
        _ => print_usage_and_exit()
    }

}

fn print_usage_and_exit() {
    eprintln!("Missing an argument.");
    eprintln!("Usage: transform [lowercase|uppercase|no-spaces|slugify|csv]");
    eprintln!("       transform (no arguments) will run in interactive threaded mode.");

    exit(1);
}
