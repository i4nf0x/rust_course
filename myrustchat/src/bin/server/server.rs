use anyhow::{Result,Context};
use chat::{Datagram, ServerResponse};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::try_join;
use std::collections::HashMap;

use tokio::net::{TcpStream,TcpListener};
use std::net::SocketAddr;

use std::process::exit;

use clap::{Parser, Subcommand};
use log;

use chat::ChatMessage;
use chat::EmptyResult;
use tokio;
use tokio::sync::{Mutex, RwLock};
use std::sync::Arc;

mod server_db;
use server_db::ServerDatabase;

#[derive(Debug,thiserror::Error)]
pub enum ServerError {
    #[error("Server error: Mutex poisoned.")]
    MutexPoisoned,
    #[error("Client error: Stream is broken")]
    BrokenStream,
    #[error("Login error")]
    LoginError,
    #[error("Message spoofing detected")]
    SpoofingError,
}

#[derive(Clone)]
struct ServerContext {
    socket_table: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>,
    username_table: Arc<RwLock<HashMap<SocketAddr, String>>>,
    database: Arc<Mutex<ServerDatabase>>
}

/// Represents a server context that wraps shared data amont the async tasks
impl ServerContext {
    pub async fn new(file: &str) -> Result<ServerContext> {
        Ok(ServerContext {
            socket_table: Arc::new(Mutex::new(HashMap::<SocketAddr, OwnedWriteHalf>::new())),
            username_table: Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new())),
            database: Arc::new(Mutex::new(ServerDatabase::new(file).await?))
        })
    }

    pub async fn add_client(&self, addr: SocketAddr, username: &str, write_half: OwnedWriteHalf) {
        let mut clients = self.socket_table.lock().await;
        let mut usernames = self.username_table.write().await;
        clients.insert(addr, write_half); 
        usernames.insert( addr, username.to_string());

        log::info!("Client {addr} connected.");
    }

    pub async fn remove_client(&self, addr: SocketAddr) {
        let mut clients = self.socket_table.lock().await;
        let mut usernames = self.username_table.write().await;

        clients.remove(&addr); 
        usernames.remove(&addr);
        log::info!("Client {addr} disconnected.");
    }

    pub async fn store_message(&self, message: &ChatMessage) -> EmptyResult {
        let mut db = self.database.lock().await;
        db.store_message(message).await
    } 

    pub fn verify_message_sender(&self, verified_username: &str, message: &ChatMessage) -> EmptyResult {
        if message.sender == verified_username {
            Ok(())
        } else {
            Err(ServerError::SpoofingError)?
        }
    }

    /// Broadcasts a chat message to all connected clients except the author.
    pub async fn broadcast_message(&self, author: SocketAddr, message: &ChatMessage) -> EmptyResult {
        let mut clients = self.socket_table.lock().await;
        let mut to_remove = vec![];

        let datagram = Datagram::Message(message.clone());

        log::debug!("Broadcasting a message from {author}");

        for (addr, write_half) in clients.iter_mut() {
            if *addr == author {
                continue;
            }

            log::debug!("Forwarding the message to {addr}.");

            if let Err(_) = datagram.write_to_stream(write_half).await {
                log::warn!("Write to client {addr} failed.");
                to_remove.push(addr.clone());
            }
        }

        for addr in to_remove {
            clients.remove(&addr);
        }

        Ok(())
    }

    pub async fn check_auth(&self, username: &str, password: &str) -> Result<bool> {
        let mut db = self.database.lock().await;
        Ok(db.check_auth(username, password).await?)
    }
}

pub async fn send_response(write_half: &mut OwnedWriteHalf, response: ServerResponse) -> EmptyResult {
    let datagram = Datagram::ServerResponse(response);
    datagram.write_to_stream(write_half).await?;
    Ok(())
}

/// Receives messages from a client and broadcasts them to other clients.
async fn receive_messages(context: ServerContext, stream: TcpStream, addr: SocketAddr)  -> EmptyResult {
    let (mut read_half, mut write_half) = stream.into_split();
    
    let verified_username;
    // Expect login datagram
    match Datagram::read_from_stream(&mut read_half).await {
        Err(e) => return Err(e)?,
        Ok(Datagram::Login { username, password }) => {
            if context.check_auth(username.as_str(), password.as_str()).await? {
                log::info!("User {username} logged in from {addr}.");
                verified_username = username;
                send_response(&mut write_half, ServerResponse::LoginOk).await?;

            } else {
                log::warn!("Invalid username or password received from {addr}.");
                send_response(&mut write_half, ServerResponse::LoginFailed).await?;

                return Err(ServerError::LoginError)?; 
            }
        },
        Ok(_) => {
            log::warn!("Login datagram not present, closing connection with {addr}.");
            return Err(ServerError::LoginError)?;
        }
    }
    
    // We have authenticated the user
    context.add_client(addr, &verified_username, write_half).await;
    log::info!("User {verified_username} successfully authenticated.");

    // Read incoming datagrams in a loop
    loop {
        match Datagram::read_from_stream(&mut read_half).await {
            Ok(Datagram::Message(message)) => { 
                context.verify_message_sender(&verified_username, &message)?;
                try_join!(
                    context.store_message(&message),
                    context.broadcast_message(addr, &message)
                )?;
            }
            Ok(_) => {
                log::warn!("Received an unexpected datagram from {addr}."); 
            },
            Err(chat::ChatProtocolError::IOError) => { 
                context.remove_client(addr).await;
                Err(ServerError::BrokenStream)?
            },
            Err(chat::ChatProtocolError::MalformedMessage) => { 
                log::warn!("Received a malformed message from {addr}."); 
            }
        }
    }
}

/// Handle incoming connection errors and pass control to receive_messages
async fn handle_client(context: ServerContext, client_info: Result<(TcpStream, SocketAddr), std::io::Error>) -> EmptyResult {
    log::info!("Client task started.");

    let (stream, address) = client_info
        .context("Failed to establish communication with a client.")?;

    if let Err(e) = receive_messages(context, stream, address).await {
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
async fn start_server(address: &str, port: u16, db_file: &str) -> EmptyResult {

    let listener = TcpListener::bind((address, port)).await
        .with_context(|| format!("Could not bind {address}:{port}."))?;

    let context = ServerContext::new(db_file).await?;

    log::info!("Ok: listening for connections on {address}:{port}");
    loop {
        let stream = listener.accept().await;
        let context  = context.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(context, stream).await {
                log::error!("Client error: {e}");
            }
        });
    }
}

async fn register_user(db_file: &str, username: &str, password: &str) -> EmptyResult {
    let mut db = ServerDatabase::new(db_file).await?;
    db.register_user(username, password).await?;
    log::info!("User {username} registered succesfully.");
    Ok(())
}

/// Simple chat server
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// SQLite database file
    #[arg(short,long, default_value = "server.db")]
    db_file: String,
    #[command(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false)]
    Run {
        /// address to bind
        #[arg(short,long,default_value = "127.0.0.1")]
        address: String,
        /// port to bind
        #[arg(short,long, default_value_t = 11111)]
        port: u16,
    },
    #[command(arg_required_else_help = true)]
    Register {
        /// username to register
        #[arg(short,long)]
        username: String,
        /// password to register
        #[arg(short,long)]
        password: String
    }
}

#[tokio::main]
async fn main() {
    simple_logger::init().unwrap();
    let args = Args::parse();
    match args.command {
        Commands::Run{address, port} => {
            if let Err(e) = start_server(&address, port, &args.db_file).await {
                log::error!("{e}");
                exit(1);
            }
        },
        Commands::Register { username, password } => {
            if let Err(e) = register_user(&args.db_file, &username, &password).await {
                log::error!("{e}");
                exit(1);
            }
        }
    }

}