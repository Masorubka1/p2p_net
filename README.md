# P2P Message Relay

## Overview
This program is designed to operate as a peer-to-peer (P2P) message relay, forwarding random messages to a specified set of peers at regular intervals. It utilizes WebSocket connections to communicate with other nodes in the network.
For more detailed instructions and project background, refer to the task description[task description](https://hackmd.io/@r3XngjBBSumx2rU-hKU7Qg/BkbHS80cv).

## Features
- WebSocket-based message forwarding.
- Periodic transmission of random messages.
- Configurable connection settings including port, keep-alive duration, and security keys.
- Fault-tolerant system desing.

## Configuration
The program can be configured through command-line arguments. Below are the available options:

- `--period`: Interval in seconds between message transmissions.
- `--port`: Port on which the server listens for incoming connections.
- `--connect`: Semicolon-separated list of server addresses to connect to (e.g., `host1:port1;host2:port2`).
- `--file`: Path to the log file directory.
- `--keep_alive`: Duration in seconds to keep the connection alive without activity.
- `--secret_key`: Secret key for securing the connection.
- `--crt_path`: Path to the PEM certificate file.
- `--key_path`: Path to the PEM private key file.
- `--pfx_path`: Path to the PFX certificate file.
- `--password`: Password for the PFX certificate file.

## Running the Program
To run the program, use the following command, replacing the arguments with your desired values:

```bash
cargo run -- --period 10 --port 8080 --connect "127.0.0.1:8081;127.0.0.1:8082" --secret_key "SOME_KEY" --crt_path "/path/to/cert.crt" --key_path "/path/to/key.key" --pfx_path "/path/to/cert.pfx" --password "password"
```

## Logging
Logging is configured to output to either the console or a file, based on the provided arguments. The log directory and file prefix can be specified via the `--file` argument.

## Dependencies
The program depends on several Rust crates, including `tokio`, `actix-web`, `tokio-tungstenite`, and `tokio-native-tls`, among others.

## Note
Ensure that the specified certificate and key files are accessible and that the provided secret key and passwords are secure. This is important for maintaining the security and integrity of the P2P communication.