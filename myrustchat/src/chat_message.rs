use serde::{Serialize, Deserialize};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}}};


/// Represents a chat message which consists of a sender nickname and content
#[derive(Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub sender: String,
    pub content: ChatMessageContent
}

/// Represents a chat message content which can be a plaintext, Image (encoded as PNG)
/// or a file (with a filename)
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

    pub async fn read_from_stream(read_half: &mut OwnedReadHalf) -> Result<ChatMessage, MessageError> {
        let mut msg_len = [0u8; 4];
        
        if read_half.read_exact(&mut msg_len).await.is_err() {
            return Err(MessageError::IOError);
        }

        let msg_len = u32::from_le_bytes(msg_len);

        let mut buf: Vec<u8> = vec![0u8;msg_len as usize];
        if read_half.read_exact(&mut buf).await.is_err() {
            return Err(MessageError::IOError);
        }

        match serde_cbor::from_slice::<ChatMessage>(&buf) {
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

    pub async fn write_to_stream(&self, stream: &mut OwnedWriteHalf) -> Result<(), MessageError> {
        match serde_cbor::to_vec(&self) {
            Ok(data) => {
                let len = (data.len() as u32 ).to_le_bytes();
                if stream.write(&len).await.is_err() {
                    return Err(MessageError::IOError);
                }

                if stream.write_all(&data).await.is_err() {
                    return Err(MessageError::IOError);
                }

                Ok(())
            },
            Err(_) => {
                Err(MessageError::MalformedMessage)
            }
        }
    }
}