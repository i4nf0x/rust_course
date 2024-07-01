use anyhow::{Result, Context};
use chat::{Datagram, ServerResponse};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::try_join;
use std::collections::HashMap;

use tokio::net::{TcpStream, TcpListener};
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

/// Enum representing various server-related errors.
#[derive(Debug, thiserror::Error)]
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

/// Struct representing the server context, holding shared data among asynchronous tasks.
#[derive(Clone)]
struct ServerContext {
    socket_table: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>,
    username_table: Arc<RwLock<HashMap<SocketAddr, String>>>,
    database: Arc<Mutex<ServerDatabase>>
}

impl ServerContext {
    /// Creates a new instance of `ServerContext`.
    ///
    /// # Arguments
    ///
    /// * `file` - A string slice that holds the path to the database file.
    ///
    /// # Returns
    ///
    /// * `Result<ServerContext>` - Returns a result containing a `ServerContext` instance if successful.
    pub async fn new(file: &str) -> Result<ServerContext> {
        Ok(ServerContext {
            socket_table: Arc::new(Mutex::new(HashMap::<SocketAddr, OwnedWriteHalf>::new())),
            username_table: Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new())),
            database: Arc::new(Mutex::new(ServerDatabase::new(file).await?))
        })
    }

    /// Adds a new client to the server context.
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address of the client.
    /// * `username` - The username of the client.
    /// * `write_half` - The writable half of the TCP stream.
    pub async fn add_client(&self, addr: SocketAddr, username: &str, write_half: OwnedWriteHalf) {
        let mut clients = self.socket_table.lock().await;
        let mut usernames = self.username_table.write().await;
        clients.insert(addr, write_half); 
        usernames.insert(addr, username.to_string());

        log::info!("Client {addr} connected.");
    }

    /// Removes a client from the server context.
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address of the client.
    pub async fn remove_client(&self, addr: SocketAddr) {
        let mut clients = self.socket_table.lock().await;
        let mut usernames = self.username_table.write().await;

        clients.remove(&addr); 
        usernames.remove(&addr);
        log::info!("Client {addr} disconnected.");
    }

    /// Stores a chat message in the database.
    ///
    /// # Arguments
    ///
    /// * `message` - A reference to a `ChatMessage` containing the message details.
    ///
    /// # Returns
    ///
    /// * `EmptyResult` - Returns an empty result if successful.
    pub async fn store_message(&self, message: &ChatMessage) -> EmptyResult {
        let mut db = self.database.lock().await;
        db.store_message(message).await
    }

    /// Verifies that the sender of a message is the authenticated user.
    ///
    /// # Arguments
    ///
    /// * `verified_username` - The username of the authenticated user.
    /// * `message` - A reference to the `ChatMessage` to be verified.
    ///
    /// # Returns
    ///
    /// * `EmptyResult` - Returns an empty result if successful.
    pub fn verify_message_sender(&self, verified_username: &str, message: &ChatMessage) -> EmptyResult {
        if message.sender == verified_username {
            Ok(())
        } else {
            Err(ServerError::SpoofingError)?
        }
    }

    /// Broadcasts a chat message to all connected clients except the author.
    ///
    /// # Arguments
    ///
    /// * `author` - The socket address of the author of the message.
    /// * `message` - A reference to the `ChatMessage` to be broadcasted.
    ///
    /// # Returns
    ///
    /// * `EmptyResult` - Returns an empty result if successful.
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

    /// Checks user authentication by verifying the password.
    ///
    /// # Arguments
    ///
    /// * `username` - A string slice that holds the username.
    /// * `password` - A string slice that holds the password.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Returns a result containing a boolean indicating if authentication was successful.
    pub async fn check_auth(&self, username: &str, password: &str) -> Result<bool> {
        let mut db = self.database.lock().await;
        Ok(db.check_auth(username, password).await?)
    }
}

/// Sends a server response to the client.
///
/// # Arguments
///
/// * `write_half` - The writable half of the TCP stream.
/// * `response` - The server response to be sent.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
pub async fn send_response(write_half: &mut OwnedWriteHalf, response: ServerResponse) -> EmptyResult {
    let datagram = Datagram::ServerResponse(response);
    datagram.write_to_stream(write_half).await?;
    Ok(())
}

/// Receives messages from a client and broadcasts them to other clients.
///
/// # Arguments
///
/// * `context` - The server context.
/// * `stream` - The TCP stream of the client.
/// * `addr` - The socket address of the client.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn receive_datagrams(context: ServerContext, stream: TcpStream, addr: SocketAddr) -> EmptyResult {
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
                log::warn!("Received a malformed datagram from {addr}."); 
            }
        }
    }
}

/// Handles incoming connection errors and passes control to `receive_datagrams`.
///
/// # Arguments
///
/// * `context` - The server context.
/// * `client_info` - The result containing the TCP stream and socket address of the client.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn handle_client(context: ServerContext, client_info: Result<(TcpStream, SocketAddr), std::io::Error>) -> EmptyResult {
    log::info!("Client task started.");

    let (stream, address) = client_info
        .context("Failed to establish communication with a client.")?;

    if let Err(e) = receive_datagrams(context, stream, address).await {
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

/// Main server function. Listens for incoming connections and spawns a new task to handle each connection.
///
/// # Arguments
///
/// * `address` - The address to bind to.
/// * `port` - The port to bind to.
/// * `db_file` - The path to the SQLite database file.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
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

/// Registers a new user in the database.
///
/// # Arguments
///
/// * `db_file` - The path to the SQLite database file.
/// * `username` - The username to register.
/// * `password` - The password to register.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn register_user(db_file: &str, username: &str, password: &str) -> EmptyResult {
    let mut db = ServerDatabase::new(db_file).await?;
    db.register_user(username, password).await?;
    log::info!("User {username} registered successfully.");
    Ok(())
}

/// Simple chat server
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// SQLite database file
    #[arg(short, long, default_value = "server.db")]
    db_file: String,
    #[command(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false)]
    Run {
        /// address to bind
        #[arg(short, long, default_value = "127.0.0.1")]
        address: String,
        /// port to bind
        #[arg(short, long, default_value_t = 11111)]
        port: u16,
    },
    #[command(arg_required_else_help = true)]
    Register {
        /// username to register
        #[arg(short, long)]
        username: String,
        /// password to register
        #[arg(short, long)]
        password: String
    }
}

#[tokio::main]
async fn main() {
    simple_logger::init().unwrap();
    let args = Args::parse();
    match args.command {
        Commands::Run { address, port } => {
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


#[cfg(test)]
mod tests {
    use chat::*;
    use tempfile;

    use crate::ServerContext;

    #[tokio::test]
    async fn test_verify_message_sender() {
        let dbfile = tempfile::tempdir().unwrap().into_path().join("test.db");
        let dbfile = dbfile.as_os_str().to_str().unwrap();

        let context = ServerContext::new(dbfile).await;
        assert!(context.is_ok());
        let context = context.unwrap();

        let verified_username = "Bob";
        let message = ChatMessage{sender: "Bob".to_string(), content: ChatMessageContent::Text("test message".to_string())};
        assert!(context.verify_message_sender(verified_username, &message).is_ok());

        let verified_username = "Alice";
        assert!(context.verify_message_sender(verified_username, &message).is_err());
    }
    
}