pub(crate) mod pool_connector {
    use dashmap::DashMap;
    use futures_util::{SinkExt, StreamExt};
    use tracing::{debug, error, info};
    use std::{net::SocketAddr, sync::Arc, time::{Duration, Instant}};
    use tokio::{fs::File, io::AsyncReadExt, sync::Mutex, time::timeout};
    use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
    use tokio_tungstenite::tungstenite::http::Request;
    use tokio::net::TcpStream;
    use tokio_native_tls::{native_tls::{self, Identity}, TlsConnector, TlsStream};
    
    
    use native_tls::TlsConnector as NativeTlsConnector;
    

    use crate::utils::cli::Cli;

    pub type Stream = WebSocketStream<MaybeTlsStream<TlsStream<TcpStream>>>;

    pub struct ConnectionPool {
        addres: SocketAddr,
        connections: DashMap<String, (Instant, Option<Arc<Mutex<Stream>>>)>,
        keep_alive: Duration,
        secret_key: String,
        key_file: String,
        pfx_file: String,
        password: String,
    }

    impl ConnectionPool {
        pub fn new(cli: &Cli) -> Self {
            ConnectionPool {
                addres: SocketAddr::new("127.0.0.1".parse().unwrap(), cli.port),
                connections: DashMap::new(),
                keep_alive: Duration::from_secs(cli.keep_alive),
                secret_key: cli.secret_key.clone(),
                pfx_file: cli.pfx_path.clone(),
                key_file: cli.key_path.clone(),
                password: cli.password.clone(),
            }
        }

        async fn create_tls_connector(cert_path: &str, key_path: &str, password: &str) -> Result<NativeTlsConnector, Box<dyn std::error::Error>> {
            let mut cert_file = File::open(cert_path).await?;
            let mut cert_data = vec![];
            cert_file.read_to_end(&mut cert_data).await?;
            
            let mut key_file = File::open(key_path).await?;
            let mut key_data = vec![];
            key_file.read_to_end(&mut key_data).await?;
        
            let identity = Identity::from_pkcs12(&cert_data, password)?;
        
            let builder = NativeTlsConnector::builder()
                .identity(identity)
                .build()?;
            Ok(builder)
        }
        

        pub async fn get_connection(&self, url: &str) -> Result<Arc<Mutex<Stream>>, Box<dyn std::error::Error>> {
            let mut recreate_connection = false;
            {
                let connection = self.connections.get(url);
                if connection.is_none() {
                    recreate_connection = true;
                }
                if connection.is_some() {
                    let con = connection.unwrap();
                    let (instant, stream_option) = con.value();
                    if instant.elapsed() >= self.keep_alive || stream_option.is_none() ||
                        !self.is_connection_alive(stream_option.clone().unwrap()).await {
                        recreate_connection = true;
                    } 
                }
            }
            if recreate_connection {
                return self.create_connection(url).await;
            }
            Ok(self.connections.get(url).unwrap().1.clone().unwrap())
        }

        async fn is_connection_alive(&self, stream_arc: Arc<Mutex<Stream>>) -> bool {
            let mut stream = stream_arc.lock().await;
        
            if stream.send(Message::Ping(Vec::new())).await.is_err() {
                return false;
            }
        
            let timeout_duration = Duration::from_secs(1);
        
            match timeout(timeout_duration, stream.next()).await {
                Ok(Some(Ok(Message::Pong(_)))) => {
                    debug!("Connection is alive - received pong.");
                    true
                },
                Ok(Some(Ok(Message::Ping(_)))) => {
                    debug!("Connection is alive - received ping.");
                    true
                },
                Ok(Some(Err(e))) => {
                    error!("Socket error: {}", e.to_string());
                    false
                },
                Ok(Some(Ok(message))) => {
                    debug!("Received an unhandled message type: {:?}", message);
                    false
                },
                Ok(None) => {
                    debug!("Connection closed by the client.");
                    false
                },
                Err(_) => {
                    error!("Ping timeout - the connection might be lost.");
                    false
                }
            }
        }

        async fn create_connection(&self, url: &str) -> Result<Arc<Mutex<Stream>>, Box<dyn std::error::Error>> {
            self.connections.insert(url.to_owned(), (Instant::now(), None));
            info!("Creating new connection, url: {}", url);
            let request = Request::builder()
                .uri(format!("ws://{}{}", url, "/ws"))
                .header("Secret-Key", &self.secret_key)
                .header("Host", self.addres.to_string())
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
                .body(())?;

            let addr = request.uri().host().unwrap_or("localhost");
            let port = request.uri().port_u16().unwrap_or(443);
            
            let socket = TcpStream::connect(format!("{}:{}", addr, port)).await?;
            info!("Conneting to: {}", format!("{}:{}", addr, port));
        
            let tls_connector = Self::create_tls_connector(&self.pfx_file, &self.key_file, &self.password).await?;
            let tokio_tls_connector = TlsConnector::from(tls_connector);
            let tls_stream = tokio_tls_connector.connect(addr, socket).await?;
        
            let (ws_stream, _) = tokio_tungstenite::client_async_tls(request, tls_stream).await?;
            let ws_connection = Arc::new(Mutex::new(ws_stream));
            self.connections.insert(url.to_owned(), (Instant::now(), Some(ws_connection.clone())));
            Ok(ws_connection)
        }

        pub async fn get_all_possible_conections(&self) -> Vec<String> {
            let mut active_connections = Vec::new();
            for entry in self.connections.iter() {
                let url = entry.key().clone();
                active_connections.push(url);
            }
            active_connections
        }
    }

}
