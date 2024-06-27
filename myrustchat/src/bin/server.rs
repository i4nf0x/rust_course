use anyhow::{Result,Context};
use tokio::net::tcp::OwnedWriteHalf;
use std::collections::HashMap;

use tokio::net::{TcpStream,TcpListener};
use std::net::SocketAddr;

use std::process::exit;

use clap::Parser;
use log;

use chat::ChatMessage;
use chat::EmptyResult;
use tokio;
use tokio::sync::Mutex;
use std::sync::Arc;



#[derive(Debug,thiserror::Error)]
pub enum ServerError {
    #[error("Server error: Mutex poisoned.")]
    MutexPoisoned,
    #[error("Client error: Stream is broken")]
    BrokenStream
}

#[derive(Clone)]
struct ClientsTable {
    pub table: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>
}

/// Represents a hash table that's shared among client threads
impl ClientsTable {
    pub fn new() -> ClientsTable {
        ClientsTable {
            table: Arc::new(Mutex::new(HashMap::<SocketAddr, OwnedWriteHalf>::new()))
        }
    }

    pub async fn add_client(&mut self, addr: SocketAddr, write_half: OwnedWriteHalf) {
        let mut clients = self.table.lock().await;
        clients.insert(addr, write_half); 
        log::info!("Client {addr} connected.");
    }

    pub async fn remove_client(&mut self, addr: SocketAddr) {
        let mut clients = self.table.lock().await;
        
        clients.remove(&addr); 
        log::info!("Client {addr} disconnected.");
    }

    /// Broadcasts a chat message to all connected clients except the author.
    pub async fn broadcast_message(&mut self, author: SocketAddr, message: &ChatMessage) -> EmptyResult {
        let mut clients = self.table.lock().await;
        let mut to_remove = vec![];

        log::debug!("Broadcasting a message from {author}");

        for (addr, write_half) in clients.iter_mut() {
            if *addr == author {
                continue;
            }

            log::debug!("Forwarding the message to {addr}.");

            if let Err(_) = message.write_to_stream(write_half).await {
                log::warn!("Write to client {addr} failed.");
                to_remove.push(addr.clone());
            }
        }

        for addr in to_remove {
            clients.remove(&addr);
        }


        Ok(())
    }
}



/// Receives messages from a client and broadcasts them to other clients.
async fn receive_messages(mut clients: ClientsTable, stream: TcpStream, addr: SocketAddr)  -> EmptyResult {
    let (mut read_half, write_half) = stream.into_split();
    clients.add_client(addr, write_half).await;

    loop {
        match ChatMessage::read_from_stream(&mut read_half).await {
            Ok(message) => { 
                clients.broadcast_message(addr, &message).await?; 
            }
            Err(chat::MessageError::IOError) => { 
                clients.remove_client(addr).await;
                Err(ServerError::BrokenStream)?
            },
            Err(chat::MessageError::MalformedMessage) => { 
                log::warn!("Received a malformed message from {addr}."); 
            }
        }
    }
}

/// Handle incoming connection errors and pass control to receive_messages
async fn handle_client(clients: ClientsTable, client_info: Result<(TcpStream, SocketAddr), std::io::Error>) -> EmptyResult {
    log::info!("Client task started.");

    let (stream, address) = client_info
        .context("Failed to establish communication with a client.")?;

    if let Err(e) = receive_messages(clients, stream, address).await {
        if let Some(ServerError::BrokenStream) = e.downcast_ref::<ServerError>() {
            log::warn!("Connection with client terminated.");
            log::warn!("{e}");
        } else {
            return Err(e);
        }
    }
    
    log::info!("Client task terminated.");
    Ok(())
}

/// Main server function. Listens for incoming connections and spawns a new thread to handle each connection.
async fn start_server(address: &str, port: u16) -> EmptyResult {
    let listener = TcpListener::bind((address, port)).await
        .with_context(|| format!("Could not bind {address}:{port}."))?;

    let clients = ClientsTable::new();

    log::info!("Ok: listening for connections on {address}:{port}");
    loop {
        let stream = listener.accept().await;
        let clients  = clients.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(clients, stream).await {
                log::error!("Client error: {e}");
            }
        });
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

#[tokio::main]
async fn main() {
    simple_logger::init().unwrap();
    let args = Args::parse(); 
    if let Err(e) = start_server(&args.address, args.port).await {
        log::error!("{e}");
        exit(1);
    }
}