use std::error::Error;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::net::{TcpStream,SocketAddr,TcpListener};
use std::ops::DerefMut;
use std::process::exit;
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

#[derive(Clone)]
struct ClientsTable {
    pub table: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>
}

impl ClientsTable {
    pub fn new() -> ClientsTable {
        ClientsTable {
            table: Arc::new(Mutex::new(HashMap::<SocketAddr, TcpStream>::new()))
        }
    }
    pub fn add_client(&mut self, addr: SocketAddr, stream: &TcpStream) -> EmptyResult {
        match self.table.lock() {
            Ok(mut clients) => { 
                clients.insert(addr, stream.try_clone()?); 
                eprintln!("Client {addr} connected.");
            },
            Err(_) => { return Err(ServerError::MutexPoisoned)?; }
        }
        Ok(())
    }

    pub fn remove_client(&mut self, addr: SocketAddr) -> EmptyResult {
        match self.table.lock() {
            Ok(mut clients) => { 
                clients.remove(&addr); 
                eprintln!("Client {addr} disconnected.");
            },
            Err(_) => { return Err(ServerError::MutexPoisoned)?; }
        }
        Ok(())
    }

    pub fn for_each<F: FnMut(&SocketAddr, &mut TcpStream) -> bool>(&mut self, callback: F) -> EmptyResult {
        match self.table.lock() {
            Ok(mut clients) => { 
                clients.retain(callback);
            },
            Err(_) => { return Err(ServerError::MutexPoisoned)?; }
        }
        Ok(())
    }
}



fn broadcast_message(author: SocketAddr, clients: &mut ClientsTable, message: &ChatMessage) -> EmptyResult {
    clients.for_each(|addr, stream| {  
        if *addr == author {
            return true;
        }
        match message.write_to(stream) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Write to client {addr} failed, disconnecting.");
                false
            },
        }
    })?;

    Ok(())
}

fn receive_messages(mut clients: ClientsTable, mut stream: TcpStream)  -> EmptyResult {
    let addr = stream.peer_addr()?;
    clients.add_client(addr, &stream)?;

    loop {
        match ChatMessage::read_from(&mut stream) {
            Ok(message) => { 
                broadcast_message(addr, &mut clients, &message)?; 
            }
            Err(chat::MessageError::IOError) => { 
                clients.remove_client(addr)?;
                return Err(ServerError::BrokenStream)?; 
            },
            Err(chat::MessageError::MalformedMessage) => { eprintln!("Received a malformed message from {addr}."); }
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
            let clients = ClientsTable::new();
            
            eprintln!("Ok: listening for connections on {address}");
            for stream in listener.incoming() {
                let clients = clients.clone();
                thread::spawn(move || handle_client(clients, stream)) ;
            }
        },
        Err(e) => {
            eprintln!("Error while listening: {e}");
            exit(1);
        }
    } 
}

fn main() { 
    listen("127.0.0.1:11111")
     
}