use std::sync::Arc;

use tracing::info;
use utils::cli::parse_config;

use crate::server::server::MyServer;

pub mod utils;
pub mod server;
mod connection_pool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (cli, _guard) = parse_config();
    info!("Starting Server on: {}:{}", "127.0.0.1", cli.port);
    let server = Arc::new(MyServer::new(&cli));
    let _ = server.run().await;
    Ok(())
}
