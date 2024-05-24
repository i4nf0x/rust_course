use std::io::{Read,Write};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub sender: String,
    pub content: ChatMessageContent
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatMessageContent {
    Text(String),
    Image(Vec<u8>),
    File(String, Vec<u8>),  // Filename and its content as bytes
}

#[derive(Debug,thiserror::Error)]
pub enum MessageError {
    #[error("Socked error")]
    IOError,
    #[error("Malformed message")]
    MalformedMessage,
}

impl ChatMessage {

    pub fn read_from(stream: &mut dyn Read) -> Result<ChatMessage, MessageError> {
        let mut msg_len = [0u8; 4];
        
        if let Err(_) = stream.read_exact(&mut msg_len) {
            return Err(MessageError::IOError);
        }

        let msg_len = u32::from_le_bytes(msg_len);

        let mut buf: Vec<u8> = vec![0u8;msg_len as usize];
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

    pub fn write_to(&self, stream: &mut dyn Write) -> Result<(), MessageError> {
        match serde_json::to_vec(&self) {
            Ok(data) => {
                let len = (data.len() as u32 ).to_le_bytes();
                if let Err(_) = stream.write(&len) {
                    return Err(MessageError::IOError);
                }

                if let Err(_) = stream.write_all(&data) {
                    return Err(MessageError::IOError);
                }

                return Ok(())
            },
            Err(_) => {
                return Err(MessageError::MalformedMessage)
            }
        }
    }
}