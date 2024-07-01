use serde::{Serialize, Deserialize};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::tcp::{OwnedReadHalf, OwnedWriteHalf}};

/// Enum representing different types of datagrams exchanged in the chat protocol.
#[derive(Serialize, Deserialize, Debug)]
pub enum Datagram {
    /// Represents a login datagram containing a username and password.
    Login { username: String, password: String },
    /// Represents a server response datagram.
    ServerResponse(ServerResponse),
    /// Represents a chat message datagram.
    Message(ChatMessage),
}

/// Enum representing different types of server responses.
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerResponse {
    /// Indicates a successful login.
    LoginOk,
    /// Indicates a failed login.
    LoginFailed,
}

/// Represents a chat message which consists of a sender nickname and content.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub sender: String,
    pub content: ChatMessageContent,
}

/// Represents the content of a chat message which can be plaintext, image (encoded as PNG), or a file (with a filename).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChatMessageContent {
    /// Plaintext message content.
    Text(String),
    /// Image message content encoded as PNG.
    Image(Vec<u8>),
    /// File message content with a filename and its content as bytes.
    File(String, Vec<u8>),
}

/// Enum representing errors that can occur in the chat protocol.
#[derive(Debug, thiserror::Error)]
pub enum ChatProtocolError {
    #[error("Socket error")]
    IOError,
    #[error("Malformed message")]
    MalformedMessage,
}

impl Datagram {
    /// Reads a `Datagram` from the provided stream.
    ///
    /// # Arguments
    ///
    /// * `read_half` - The readable half of the TCP stream.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<Datagram, ChatProtocolError>` - Returns a result containing the `Datagram` if successful.
    pub async fn read_from_stream(read_half: &mut OwnedReadHalf) -> anyhow::Result<Datagram, ChatProtocolError> {
        let mut msg_len = [0u8; 4];
        
        if read_half.read_exact(&mut msg_len).await.is_err() {
            return Err(ChatProtocolError::IOError);
        }

        let msg_len = u32::from_le_bytes(msg_len);

        let mut buf: Vec<u8> = vec![0u8; msg_len as usize];
        if read_half.read_exact(&mut buf).await.is_err() {
            return Err(ChatProtocolError::IOError);
        }

        match serde_cbor::from_slice::<Datagram>(&buf) {
            Ok(datagram) => Ok(datagram),
            Err(e) => {
                if e.is_io() || e.is_eof() {
                    Err(ChatProtocolError::IOError)
                } else {
                    Err(ChatProtocolError::MalformedMessage)
                }
            }
        }
    }

    /// Writes a `Datagram` to the provided stream.
    ///
    /// # Arguments
    ///
    /// * `stream` - The writable half of the TCP stream.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<(), ChatProtocolError>` - Returns an empty result if successful.
    pub async fn write_to_stream(&self, stream: &mut OwnedWriteHalf) -> anyhow::Result<(), ChatProtocolError> {
        match serde_cbor::to_vec(&self) {
            Ok(data) => {
                let len = (data.len() as u32).to_le_bytes();
                if stream.write(&len).await.is_err() {
                    return Err(ChatProtocolError::IOError);
                }

                if stream.write_all(&data).await.is_err() {
                    return Err(ChatProtocolError::IOError);
                }

                Ok(())
            },
            Err(_) => {
                Err(ChatProtocolError::MalformedMessage)
            }
        }
    }
}
