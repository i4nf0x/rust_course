use std::ffi::OsStr;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::process::exit;
use std::fs::File;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use clap::Parser;
use image::io::Reader as ImageReader;
use anyhow::{Context, Error, Result};

use chat::{ChatMessage, ChatMessageContent, Datagram, EmptyResult, ServerResponse};

/// Enum representing different types of client errors.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("File operation failed.")]
    FileOperationFailed(#[from] Error),
    #[error("Stream is broken")]
    BrokenStream,
    #[error("Login failed")]
    LoginFailed
}

/// Listens to the TCP socket and processes incoming messages.
///
/// # Arguments
///
/// * `read_half` - The readable half of the TCP stream.
async fn incoming_loop(mut read_half: OwnedReadHalf) {
    loop {
        match Datagram::read_from_stream(&mut read_half).await {
            Ok(Datagram::Message(message)) => {
                let sender = message.sender;
                match message.content {
                    ChatMessageContent::Text(text) => {
                        println!("[{sender}] {text}");
                    },
                    ChatMessageContent::Image(data) => {
                        println!("[{sender}] sending an image");
                        if let Some(file) = handle_incoming_file("images", data, None) {
                            println!("Image saved to {}", file);
                        }
                    },
                    ChatMessageContent::File(filename, data) => {
                        println!("[{sender}] sending a file");
                        if let Some(file) = handle_incoming_file("files", data, Some(filename)) {
                            println!("File saved to {}", file);
                        }
                    }
                }
            },
            Ok(Datagram::ServerResponse(_)) => {
                // We don't handle any server responses here
            },
            Ok(_) => {
                eprintln!("Error: unexpected datagram");
            }
            Err(chat::ChatProtocolError::MalformedMessage) => {
                eprintln!("Error: Malformed message received."); 
            },
            Err(chat::ChatProtocolError::IOError) => {
                eprintln!("Error: Connection with server broken.");
                exit(1);
            }
        };
    }
}

/// Handles incoming file and saves it to the specified directory.
///
/// # Arguments
///
/// * `dir_path` - The directory path where the file will be saved.
/// * `data` - The file data.
/// * `filename` - The optional filename.
///
/// # Returns
///
/// * `Option<String>` - Returns the saved filename if successful.
fn handle_incoming_file(dir_path: &str, data: Vec<u8>, filename: Option<String>) -> Option<String> {
    match save_received_file(dir_path, data, filename) {
        Ok(filename) => {
            Some(filename)
        },
        Err(e) => {
            eprintln!("Failed to save an incoming file.");
            eprintln!("{e}");
            None
        }
    }
}

/// Saves the received file to the specified directory.
///
/// # Arguments
///
/// * `dir_path` - The directory path where the file will be saved.
/// * `data` - The file data.
/// * `filename` - The optional filename.
///
/// # Returns
///
/// * `Result<String>` - Returns the saved filename if successful.
fn save_received_file(dir_path: &str, data: Vec<u8>, filename: Option<String>) -> Result<String> {
    let dir_path = Path::new(dir_path);
    if !dir_path.exists() {
        std::fs::create_dir_all(dir_path)
            .with_context(|| format!("Error: Failed to create directory: {:?}", dir_path))?;       
    }
    
    let filename = basename(filename.unwrap_or(generate_timestamp("png")).as_str());
    let filepath = Path::join(dir_path, filename);

    let mut file = File::create(&filepath)
        .with_context(|| format!("Error: Could not create {:?}", &filepath))?;
    file.write_all(&data)
        .with_context(|| format!("Error: Could not write to {:?}", &filepath))?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Generates a timestamped filename with the specified extension.
///
/// # Arguments
///
/// * `file_ext` - The file extension.
///
/// # Returns
///
/// * `String` - Returns the generated filename.
fn generate_timestamp(file_ext: &str) -> String {
    let time = chrono::Local::now();
    time.format("%Y-%m-%d-%H:%M:%S.").to_string() + file_ext
}

/// Extracts the basename from the given filename.
///
/// # Arguments
///
/// * `filename` - The full filename.
///
/// # Returns
///
/// * `String` - Returns the basename of the filename.
fn basename(filename: &str) -> String {
    let default_fn = "unknown.bin";
    Path::new(filename).file_name()
                .unwrap_or(OsStr::new(default_fn))
                .to_str().unwrap_or(default_fn).to_string()
}

/// Represents the chat context holding the writable half of the TCP stream and the username.
struct ChatContext {
    write_half: OwnedWriteHalf,
    username: String,
}

/// Enum representing different user commands.
#[derive(PartialEq)]
enum UserCommand {
    Text(String),
    File(String),
    Image(String),
    Quit,
}

impl UserCommand {
    /// Parses a command string into a `UserCommand`.
    ///
    /// # Arguments
    ///
    /// * `line` - The command string.
    ///
    /// # Returns
    ///
    /// * `UserCommand` - Returns the parsed user command.
    fn from_str(line: &str) -> UserCommand {
        let line_sep = line.to_string() + " ";
        let command = line_sep.split_once(' ');
        match command {
            Some((".quit", "")) => Self::Quit,
            Some((".file", filename)) => Self::File(filename.trim().to_string()),
            Some((".image", filename)) => Self::Image(filename.trim().to_string()),
            _ => Self::Text(line.to_string())
        }
    }

    /// Performs a user command.
    ///
    /// # Arguments
    ///
    /// * `context` - The chat context.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Returns `true` if the command indicates to quit, otherwise `false`.
    async fn perform(&self, context: &mut ChatContext) -> Result<bool> {
        match &self {
            Self::Text(text) => {
                send_message(context, ChatMessageContent::Text(text.clone())).await?;
                Ok(false)
            },
            Self::Image(filename) => {
                let data = read_image_data(filename)
                    .map_err(|e| ClientError::FileOperationFailed(e))?;
                let content = ChatMessageContent::Image(data);
                send_message(context, content).await?;
                println!("Image sent.");
                Ok(false)
            },
            Self::File(filename) => {
                let data = read_file_data(filename)
                    .map_err(|e| ClientError::FileOperationFailed(e))?;
                let content = ChatMessageContent::File(basename(filename), data);
                send_message(context, content).await?;
                println!("File {} sent.", basename(filename));
                Ok(false)
            },
            Self::Quit => {
                println!("Ok, bye.");
                Ok(true)
            }
        }
    }
}

/// Continuously reads from stdin and processes user commands.
///
/// # Arguments
///
/// * `context` - The chat context.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn keyboard_loop(context: &mut ChatContext) -> EmptyResult {
    println!("Ok, connected to server.");
    println!("Your name is {}", context.username);
    loop {
        let mut buf = String::new();
        let len = std::io::stdin().read_line(&mut buf)
            .context("Can't read from stdin.")?;
        
        if len > 0 {
            let cmd = UserCommand::from_str(buf.trim());

            match cmd.perform(context).await {
                Err(e) => {
                    // If there was a problem with file handling, print it, otherwise terminate the loop
                    if matches!(e.downcast_ref::<ClientError>(), Some(ClientError::FileOperationFailed(_))) {
                        eprintln!("Error: {e}"); 
                        eprint!("{}", e.root_cause());
                    } else {
                        return Err(e); 
                    }
                },
                Ok(true) => return Ok(()), // exit
                _ => (),
            }
        } else {
            return Ok(()); // end of input, exit
        }
    }
}

/// Sends a chat message.
///
/// # Arguments
///
/// * `context` - The chat context.
/// * `content` - The content of the chat message.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn send_message(context: &mut ChatContext, content: ChatMessageContent) -> EmptyResult {
    let nickname = context.username.to_string();
    let message = ChatMessage {
        sender: nickname,
        content,
    };

    Datagram::Message(message).write_to_stream(&mut context.write_half).await
        .context("Failed to send a message.")?;
    Ok(())
}

/// Reads image data from a file. The file is converted to PNG if needed.
///
/// # Arguments
///
/// * `filename` - The name of the file.
///
/// # Returns
///
/// * `Result<Vec<u8>>` - Returns the image data if successful.
fn read_image_data(filename: &str) -> Result<Vec<u8>> {
    let extension = Path::new(filename).extension().and_then(OsStr::to_str);
    match extension {
        Some("png") | Some("PNG") => {
            read_file_data(filename)
        },
        _ => {
            let img = ImageReader::open(filename)
                .with_context(|| format!("Could not open {filename} for image conversion."))
                ?.decode()
                .with_context(|| format!("Could not decode {filename}."))?;
            let mut data = Vec::<u8>::new();
            img.write_to(&mut Cursor::new(&mut data), image::ImageFormat::Png)
                .with_context(|| format!("Could not encode {filename}"))?;
            Ok(data)
        }
    }
}

/// Reads file data to a vector.
///
/// # Arguments
///
/// * `filename` - The name of the file.
///
/// # Returns
///
/// * `Result<Vec<u8>>` - Returns the file data if successful.
fn read_file_data(filename: &str) -> Result<Vec<u8>> {
    let mut file = File::open(filename)
        .with_context(|| format!("Could not open file {filename}."))?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)
        .with_context(|| format!("Could not read file {filename}."))?;
    Ok(buf)
}

/// Main function of the client. Connects to the server and starts the keyboard loop
/// which reads text commands.
///
/// # Arguments
///
/// * `address` - The address of the server.
/// * `port` - The port of the server.
/// * `username` - The username of the client.
/// * `password` - The password of the client.
///
/// # Returns
///
/// * `EmptyResult` - Returns an empty result if successful.
async fn start_client(address: &str, port: u16, username: String, password: String) -> EmptyResult {
    let stream = TcpStream::connect((address, port)).await
        .with_context(|| format!("Could not connect to {address}:{port}"))?;
    let (mut read_half, mut write_half) = stream.into_split();

    // Authenticate
    println!("Waiting for login...");
    let login_datagram = Datagram::Login { username: username.clone(), password };
    login_datagram.write_to_stream(&mut write_half).await?;

    if let Datagram::ServerResponse(ServerResponse::LoginOk) = Datagram::read_from_stream(&mut read_half).await? {
        println!("Login successful.");
        tokio::spawn(async move {
            incoming_loop(read_half).await
        });
    
        let mut context = ChatContext { write_half, username };
        return keyboard_loop(&mut context).await;
    } else {
        Err(ClientError::LoginFailed)?
    }
}

/// Simple chat client.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Address of the server
    #[arg(short, long, default_value = "127.0.0.1")]
    address: String,
    /// Port of the server
    #[arg(short = 'P', long, default_value_t = 11111)]
    port: u16,
    /// Your username
    #[arg(short)]
    username: String,
    /// Your password
    #[arg(short = 'p')]
    password: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = start_client(&args.address, args.port, args.username, args.password).await {
        eprintln!("Error: {e}");
        exit(1);
    } else {
        exit(0);
    }
}

#[cfg(test)]
mod tests {

    use crate::{basename, UserCommand};

    #[test]
    fn test_basename() {
        let z = basename("a/b/c.txt");
        assert_eq!(z, "c.txt");
    }

    #[test]
    fn test_user_command_from_str() {
        assert!(matches!(UserCommand::from_str("this is a text"), UserCommand::Text(_)));
        
        let file_command = UserCommand::File("test.txt".to_string());
        assert!(UserCommand::from_str(".file test.txt")==file_command);

        let image_command = UserCommand::Image("test.jpg".to_string());
        assert!(UserCommand::from_str(".image test.jpg")==image_command);

        assert!(matches!(UserCommand::from_str(".quit  "), UserCommand::Text(_)));
        
        assert!(matches!(UserCommand::from_str(".quit"), UserCommand::Quit));
    }
}

