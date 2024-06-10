use anyhow::{Result,Context};
use std::collections::{HashMap};

use std::net::{TcpStream,SocketAddr,TcpListener};

use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

use clap::Parser;
use log;

use chat::ChatMessage;
use chat::EmptyResult;


#[derive(Debug,thiserror::Error)]
pub enum ServerError {
    #[error("Server error: Mutex poisoned.")]
    MutexPoisoned,
    #[error("Client error: Stream is broken")]
    BrokenStream
}

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
        let mut clients = self.table.lock()
            .map_err(|_| ServerError::MutexPoisoned)?;
        clients.insert(addr, stream.try_clone()?); 
        log::info!("Client {addr} connected.");
        Ok(())
    }

    pub fn remove_client(&mut self, addr: SocketAddr) -> EmptyResult {
        let mut clients = self.table.lock()
            .map_err(|_| ServerError::MutexPoisoned)?;
        
        clients.remove(&addr); 
        log::info!("Client {addr} disconnected.");
        Ok(())
    }

    pub fn for_each<F: FnMut(&SocketAddr, &mut TcpStream) -> bool>(&mut self, callback: F) -> EmptyResult {
        let mut clients = self.table.lock()
            .map_err(|_| ServerError::MutexPoisoned)?;
        clients.retain(callback);    
        Ok(())
    }
}


/// Broadcasts a chat message to all connected clients except the author.
fn broadcast_message(author: SocketAddr, clients: &mut ClientsTable, message: &ChatMessage) -> EmptyResult {
    log::debug!("Broadcasting a message from {author}");
    clients.for_each(|addr, stream| {  
        if *addr == author {
            return true;
        }

        log::debug!("Forwarding the message to {addr}.");

        if let Ok(_) = message.write_to(stream) {
            true
        } else {
            log::warn!("Write to client {addr} failed.");
            false
        }
    })?;

    Ok(())
}

/// Receives messages from a client and broadcasts them to other clients.
fn receive_messages(mut clients: ClientsTable, stream: Result<TcpStream, std::io::Error>)  -> EmptyResult {
    let mut stream = stream.context("Failed to establish communication with a client.")?;
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
            Err(chat::MessageError::MalformedMessage) => { 
                log::warn!("Received a malformed message from {addr}."); 
            }
        }
    }
}

/// Handle incoming connection errors and pass control to receive_messages
fn handle_client(clients: ClientsTable, stream: Result<TcpStream, std::io::Error>) {
    log::info!("Client thread started.");

    if let Err(e) = receive_messages(clients, stream) {
        if let Some(ServerError::BrokenStream) = e.downcast_ref::<ServerError>() {
            log::warn!("Connection with client terminated.");
            log::warn!("{e}");
        } else {
            log::error!("{e}");
        }
        
    }
    log::info!("Client thread terminated.");
}

/// Main server function. Listens for incoming connections and spawns a new thread to handle each connection.
fn start_server(address: &str, port: u16) -> EmptyResult {
    let listener = TcpListener::bind((address, port))
        .with_context(|| format!("Could not bind {address}:{port}."))?;

    let clients = ClientsTable::new();

    log::info!("Ok: listening for connections on {address}:{port}");
    for stream in listener.incoming() {
        let clients = clients.clone();
        thread::spawn(move || handle_client(clients, stream)) ;
    }

    unreachable!();
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
    if let Err(e) = start_server(&args.address, args.port) {
        log::error!("{e}");
        exit(1);
    }
}