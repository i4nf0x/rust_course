use chat::{ChatMessage, ChatMessageContent};
use std::io::{stdin, Read};
use std::net::TcpStream;
use std::process::exit;
use std::thread;
use clap::Parser;
use std::fs::File;
use std::error::Error;


struct ChatContext<'a> {
    stream: &'a mut TcpStream,
    nickname: &'a str
}

fn incoming_loop(mut stream: TcpStream) {
    loop {
        match ChatMessage::read_from(&mut stream) {
            Ok(message) => {
                let sender = message.sender;
                match message.content {
                    ChatMessageContent::Text(text) => {
                        println!("[{sender}] {text}");
                    },
                    ChatMessageContent::Image(data) => {
                        println!("[{sender}] sent an image");
                    },
                    ChatMessageContent::File(filename, data) => {
                        println!("[{sender}] sent an file: ");
                    }
                }
            },
            Err(chat::MessageError::MalformedMessage) => {
                eprintln!("Error: Malformed message received."); 
            },
            Err(chat::MessageError::IOError) => {
                eprintln!("Error: Connection with server broken.");
                exit(1);
            }
        };
    }
}

fn cmd_quit() {
    println!("Ok, bye.");
    exit(0);
}

fn read_file_to_vec(filename: &str) -> Result<Vec<u8>,Box<dyn Error>> {
    let mut file = File::open(filename)?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn send_message(context: &mut ChatContext, message: ChatMessage) {
    if let Err(_) = message.write_to(&mut context.stream) {
        eprintln!("Failed to send a message.");
        exit(1);
    }
}

fn cmd_send_file(context: &mut ChatContext, filename: &str) {
    match read_file_to_vec(filename) {
        Ok(data) => {
            let message = ChatMessage {
                sender: context.nickname.to_string(),
                content: ChatMessageContent::File(filename.to_string(), data)
            };
            send_message(context, message);
        },
        Err(e) => {
            println!("Could not read file \"{filename}\"\n{e}");
        }
    }
}

fn cmd_send_image(context: &mut ChatContext, filename: &str) {
    match read_file_to_vec(filename) {
        Ok(data) => {
            let message = ChatMessage {
                sender: context.nickname.to_string(),
                content: ChatMessageContent::Image(data)
            };
            send_message(context, message);
        },
        Err(e) => {
            println!("Could not read image \"{filename}\"\n{e}");
        }
    }
}

fn cmd_send_text(context: &mut ChatContext, text: String) {
    let message = ChatMessage {
        sender: context.nickname.to_string(),
        content: ChatMessageContent::Text(text)
    };

    send_message(context, message);
}

fn process_command(context: &mut ChatContext, line: String) {
    let mut line_sep = line.clone()+" ";
    let command = line_sep.split_once(' ');
    println!("{:?}",command);
    match command {
        Some(("","")) => {},
        Some((".quit","")) => cmd_quit(),
        Some((".file", filename)) => cmd_send_file(context, filename.trim()),
        Some((".image", filename)) => cmd_send_image(context, filename.trim()),
        _ => cmd_send_text(context, line)
    }

}

fn keyboard_loop(context: &mut ChatContext) {
    println!("Ok, connected to server.");
    loop {
        let mut buf = String::new();
        match std::io::stdin().read_line(&mut buf) {
            Err(_) => {
                eprintln!("Error reading from stdin.");
                exit(1);
            },
            Ok(0) => cmd_quit(),
            Ok(_) => {
                process_command(context, buf.trim().to_string());
            }
        }
    }
}

fn start_client(address: &str, port: u16, nickname: &str) {
    match TcpStream::connect((address, port) ) {
        Ok(mut stream) => {
            let stream2 = stream.try_clone().expect("Could not clone the stream.");
            thread::spawn(move || incoming_loop(stream2));
            let mut context = ChatContext{stream: &mut stream, nickname};
            keyboard_loop(&mut context);
        },
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }

}

/// Simple chat client
#[derive(Parser,Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// address of the server
    #[arg(short,long,default_value = "127.0.0.1")]
    address: String,
    /// port of the server
    #[arg(short,long, default_value_t = 11111)]
    port: u16,
    /// nickname for your messages
    #[arg(short,default_value = "anonymous")]
    nickname: String
}

fn main() {
    let args = Args::parse();

    start_client(&args.address, args.port, &args.nickname);
}