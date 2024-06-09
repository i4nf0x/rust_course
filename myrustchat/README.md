# rust-chat

rust-chat is a simple command-line application that allows people to chat in real-time. It consists of two binaries: a server and a client. The server binary handles message distribution, while the client binary allows users to connect to the server and send text messages, files, and images to a group chat.

## Features

- Real-time group chat from the command line
- Support for sending text messages, files, and images
- Demonstrates Rust's networking and threading capabilities

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
- `chrono` for timestamp generation
- `image` for image conversion
- `log` and `simple_logger` for pretty logging
- `clap` for commandline argument parsing

## Changelog
- 0.1.0 - the initial version with basic functionality
- 0.1.1 - added the `log` library in the server

## Usage
To quickly test the whole project you can run the "test.sh" script which will spawn a server and two clients. It requires Xterm to be present in your system.

### Server

To run the server, simply run the 'server' binary:

```sh
server
```

There are optional arguments

 - -a, --address <ADDRESS>: Address to bind [default: 127.0.0.1]
 - -p, --port <PORT>: Port to bind [default: 11111]
 - -h, --help: Print help
 - -V, --version: Print version

### Client

 - -a, --address <ADDRESS>: Address of the server [default: 127.0.0.1]
 - -p, --port <PORT>: Port of the server [default: 11111]
 - -n <NICKNAME>: Nickname for your messages [default: anonymous]
 - -h, --help: Print help
 - -V, --version: Print version

Sending messages:

- To send a text message, simply type your message and press Enter.

- To send an image, type .image filename.png where filename.png is the name of the image file. The image will be always automatically converted to .png on the client.

- To send a file, type .file filename.txt where filename.txt is the name of the file.

## Known issues
- When a user receives a message while typing, the input message will be interrupted by the incoming message text.