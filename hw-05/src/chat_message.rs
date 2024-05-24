use std::net::TcpStream;
use std::io::Read;
use std::error::Error;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatMessage {
    Text(String),
    Image(Vec<u8>),
    File(String, Vec<u8>),  // Filename and its content as bytes
}

#[derive(Debug,thiserror::Error)]
pub enum MessageError {
    #[error("Socked closed")]
    IOError,
    #[error("Malformed message")]
    MalformedMessage
}

impl ChatMessage {
    pub fn read(stream: &mut dyn Read) -> Result<ChatMessage, MessageError> {
        let mut msg_len = [0u8; 4];
        
        if let Err(_) = stream.read_exact(&mut msg_len) {
            return Err(MessageError::IOError);
        }

        let msg_len = u32::from_le_bytes(msg_len);

        let mut buf: Vec<u8> = Vec::with_capacity(msg_len as usize);
        if let Err(_) = stream.read_exact(&mut buf) {
            return Err(MessageError::IOError);
        }

        match serde_json::from_slice::<ChatMessage>(&buf) {
            Ok(message) => Ok(message),
            Err(e) => {
                if e.is_io() || e.is_eof() {
                    Err(MessageError::IOError)
                } else {
                    Err(MessageError::MalformedMessage)
                }
            }
        }

    }
}