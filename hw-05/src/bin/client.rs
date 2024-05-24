use chat::{ChatMessage, ChatMessageContent};
use std::io::{stdin, Read};
use std::net::TcpStream;
use std::process::exit;
use std::thread;

fn incoming_loop(mut stream: TcpStream) {
    loop {
        match ChatMessage::read_from(&mut stream) {
            Ok(message) => {
                match message.content {
                    ChatMessageContent::Text(text) => {
                        let sender = message.sender;
                        println!("[{sender}] {text}");
                    },
                    ChatMessageContent::Image(data) => {
                        println!("Received a file...");
                    },
                    ChatMessageContent::File(filename, data) => {
                        println!("Received a file...");
                    }
                }
            },
            Err(_) => {
                eprintln!("Error: Malformed message received."); 
            }
        };
    }
}

fn keyboard_loop(mut stream: TcpStream) {
    println!("Ok, connected to server.");
    loop {
        let mut buf = String::new();
        match std::io::stdin().read_line(&mut buf) {
            Err(_) => {
                eprintln!("Error reading from stdin.");
                exit(1);
            }

            Ok(0) => {
                println!("Ok, bye.");
                exit(0);
            }
            
            Ok(_) => {
                let buf = buf.trim().to_string();
                let message = ChatMessage {
                    sender: "test".to_string(),
                    content: ChatMessageContent::Text(buf)
                };

                if let Err(_) = message.write_to(&mut stream) {
                    eprintln!("Failed to send a message.");
                    exit(1);
                }
            }
        }

    }
    

}

fn start_client(address: &str) {
    match TcpStream::connect(address) {
        Ok(stream) => {
            let stream2 = stream.try_clone().expect("Could not clone the stream.");
            thread::spawn(move || incoming_loop(stream2));
            keyboard_loop(stream);
        },
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }

}

fn main() {
    start_client("127.0.0.1:11111");
    
}