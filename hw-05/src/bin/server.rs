use std::error::Error;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::net::{TcpStream,SocketAddr,TcpListener};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::thread;
use std::rc::Rc;

use chat::ChatMessage;




#[derive(Debug,thiserror::Error)]
pub enum ServerError {
    #[error("Mutex poisoned.")]
    MutexPoisoned,
    #[error("Stream is broken")]
    BrokenStream
}

type EmptyResult = Result<(), Box<dyn Error>>;
type ClientsTable = Arc<Mutex<HashMap<SocketAddr, TcpStream>>>;

fn broadcast_message(author: SocketAddr, clients: ClientsTable, message: &ChatMessage) -> EmptyResult {
    match clients.lock() {
        Ok(mut clients)  =>{
            let mut failed = Vec::<SocketAddr>::new();
            let data: Vec<u8>  = serde_json::to_vec(message)?;

            clients.retain(|addr, stream| {  
                if *addr == author {
                    return true;
                }  
                match stream.write_all(&data) {
                    Ok(_) => true,
                    Err(e) => {
                        eprintln!("Write to client {addr} failed, disconnecting.");
                        false
                    },
                }
            });
        }
        Err(_) => {
            return Err(ServerError::MutexPoisoned)?;
        }
    }
    Ok(())
}


fn receive_messages(clients: ClientsTable, mut stream: TcpStream)  -> EmptyResult {
    let addr = stream.peer_addr()?;

    match clients.lock() {
        Ok(mut clients) => {
            clients.insert(addr, stream.try_clone()?);
        }
        Err(_) => {
            return Err(ServerError::MutexPoisoned)?;
        }
    }

    loop {
        match ChatMessage::read(&mut stream) {
            Ok(message) => {
                broadcast_message(addr, Arc::clone(&clients), &message)?;
            }
            Err(chat::MessageError::IOError) => {
                eprintln!("Error reading form socket {addr}, dropping connection.");
                break;
            },
            Err(chat::MessageError::MalformedMessage) => {
                eprintln!("Received a malformed message from {addr}.");
            }
        }
    }

    return Ok(());
}

fn handle_client(clients: ClientsTable, stream: Result<TcpStream, std::io::Error>) {
    match stream {
        Ok(stream) => {
            if let Err(e) = receive_messages(clients, stream) {
                eprintln!("Client connection error: {e}");
            }
        },

        Err(e) => {
            eprintln!("Failed to handle a connection: {e}");
        }
    };
}


fn listen(address: &str) {
    match  TcpListener::bind(address) {
        Ok(listener) => {
            let clients: HashMap<SocketAddr, TcpStream> = HashMap::new();
            let clients = Arc::new(Mutex::new(clients));
        
            eprintln!("Ok: listening for connections on {address}");
            for stream in listener.incoming() {
                let clients = Arc::clone(&clients);
                thread::spawn(move || handle_client(clients, stream)) ;
            }
        },
        Err(e) => {
            eprintln!("Error while listening: {e}");
        }
    } 
}

fn main() { 
    listen("127.0.0.1:11111")
     
}