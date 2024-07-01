# rust-chat

myrustchat is a simple command-line application that allows people to chat in real-time. It consists of two binaries: a server and a client. The server binary handles message distribution, while the client binary allows users to connect to the server and send text messages, files, and images to a group chat.

## Features

- Demonstrates Rust's async networking and database capabilities
- Real-time group chat from the command line
- Support for sending text messages, files, and images
- Uses SQLite (via sqlx) to store user credentials and message history

## Security considerations
- Users are authenticated with a username and password. Credentials are currently passed as command-line parameters. It would be more secure to read them directly from stdin, store them in a config file or implement some more secure workflow similar to OAuth to avoid storing them altogether.
- Server will detect attempts to spoof messages from other users and will discard such messages.
- All communication is currently unencrypted. It's assumed that TLS would be used in a real-world scenario.
- All passwords are stored in a hashed form, however, they are transported in plaintext over the network. This would be solved by TLS as stated in the previous point. 


## Prerequisites

All dependencies are specified in `Cargo.toml` and will be automatically downloaded from crates.io.

## Installation

To install and compile the application, run the following command:

```sh
cargo build
```

## Dependencies
- `serde` and `serde_cbor` for message marshalling
- `thiserror` for creating custom errors
- `anyhow` error handling
- `chrono` for timestamp generation
- `image` for image conversion
- `log` and `simple_logger` for pretty logging
- `clap` for commandline argument parsing
- `tokio` for async networking
- `sqlx` for database
- `argon2` for secure password hashing

## Changelog
- 0.1.0 - the initial version with basic functionality
- 0.1.1 - added the `log` library in the server
- 0.1.2 - improved error handling
- 0.1.3 - ported to async, added database functionality

## Usage
To quickly test the whole project you can run the "test.sh" script.
If no database is present, this script will register two users, Alice and Bob.

Then it  will spawn a server and two clients. It requires bash and Xterm to be present in your system.

### Server

Before any users can connect, they need to be registered on the server. Registration is accomplished with the server command.

To register a user run the 'server' binary with a command 'register' and username and password:

```sh
server register -u Bob -p bbb
```

Where `-u` specifies the username and `-p` the password to be registered.


There are optional arguments

 - -a, --address <ADDRESS>: Address to bind [default: 127.0.0.1]
 - -p, --port <PORT>: Port to bind [default: 11111]
 - -d, --db-file: SQLite


To run the server, simply run the 'server' binary:

```sh
server run
```

There are optional arguments:

 - -a, --address <ADDRESS>: Address to bind [default: 127.0.0.1]
 - -p, --port <PORT>: Port to bind [default: 11111]
 - -d, --db-file: SQLite

### Client
 
Mandatory arguments:

 - -u <USERNAME>: username for authentication
 - -p <PASSWORD>: password for authentication

Optional arguments:
 - -a, --address <ADDRESS>: Address of the server [default: 127.0.0.1]
 - -p, --port <PORT>: Port of the server [default: 11111]


Sending messages:

- To send a text message, simply type your message and press Enter.

- To send an image, type `.image filename.png` where filename.png is the name of the image file. The image will be always automatically converted to .png on the client.

- To send a file, type `.file filename.txt` where filename.txt is the name of the file.

## Known issues
- When a user receives a message while typing, the input message will be interrupted by the incoming message text.
- History is currently logged but there is no way to view the messages.