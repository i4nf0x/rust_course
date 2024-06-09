use std::error::Error;
use std::collections::{HashMap};

use std::net::{TcpStream,SocketAddr,TcpListener};

use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

use clap::Parser;
use log;

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

/// Represents a hash table that's shared among client threads
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
                log::info!("Client {addr} connected.");
            },
            Err(_) => { return Err(ServerError::MutexPoisoned)?; }
        }
        Ok(())
    }

    pub fn remove_client(&mut self, addr: SocketAddr) -> EmptyResult {
        match self.table.lock() {
            Ok(mut clients) => { 
                clients.remove(&addr); 
                log::info!("Client {addr} disconnected.");
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


/// Broadcasts a chat message to all connected clients except the author.
fn broadcast_message(author: SocketAddr, clients: &mut ClientsTable, message: &ChatMessage) -> EmptyResult {
    clients.for_each(|addr, stream| {  
        if *addr == author {
            return true;
        }
        match message.write_to(stream) {
            Ok(_) => true,
            Err(_e) => {
                log::warn!("Write to client {addr} failed, disconnecting.");
                false
            },
        }
    })?;

    Ok(())
}

/// Receives messages from a client and broadcasts them to other clients.
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
                Err(ServerError::BrokenStream)?; 
            },
            Err(chat::MessageError::MalformedMessage) => { log::warn!("Received a malformed message from {addr}."); }
        }
    }
}

/// Handle incoming connection errors and pass control to receive_messages
fn handle_client(clients: ClientsTable, stream: Result<TcpStream, std::io::Error>) {
    match stream {
        Ok(stream) => {
            if let Err(e) = receive_messages(clients, stream) {
                log::warn!("Client connection error: {e}");
            }
        },

        Err(e) => {
            log::warn!("Failed to handle a connection: {e}");
        }
    };
}

/// Main server function. Listens for incoming connections and spawns a new thread to handle each connection.
fn start_server(address: &str, port: u16) {
    match  TcpListener::bind((address, port) ) {
        Ok(listener) => {
            let clients = ClientsTable::new();
            
            log::info!("Ok: listening for connections on {address}");
            for stream in listener.incoming() {
                let clients = clients.clone();
                thread::spawn(move || handle_client(clients, stream)) ;
            }
        },
        Err(e) => {
            log::error!("Couldn't bind to {e}");
            exit(1);
        }
    } 
}

/// Simple chat server
#[derive(Parser,Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// address to bind
    #[arg(short,long,default_value = "127.0.0.1")]
    address: String,
    /// port to bind
    #[arg(short,long, default_value_t = 11111)]
    port: u16,
}

fn main() {
    simple_logger::init().unwrap();
    let args = Args::parse(); 
    start_server(&args.address, args.port);
}