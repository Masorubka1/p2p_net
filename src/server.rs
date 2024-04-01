pub(crate) mod server {
    use actix::{Actor, StreamHandler};
    use actix::ActorContext;
    
    use actix_web::http::StatusCode;
    use actix_web::web::Data;
    use actix_web::{error, web, App, Error, HttpRequest, HttpResponse, HttpResponseBuilder, HttpServer};
    use flate2::bufread::GzDecoder;
    use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
    use tokio::signal;
    
    
    
    use tracing::{debug, error, info};
    use futures_util::sink::SinkExt;
    
    
    
    use std::{io::Read, net::SocketAddr, sync::Arc, time::Duration};
    use tokio_tungstenite::tungstenite::Message as WebSocketMessage;

    use actix_web_actors::ws::{self};

    use crate::{connection_pool::pool_connector::ConnectionPool, utils::cli::Cli};

    pub struct MyServer {
        address: SocketAddr,
        pool_connections: Arc<ConnectionPool>,
        secret_key: String,
        cli: Cli,
        key_file: String,
        cert_file: String,
    }

    impl MyServer {
        pub fn new(cli: &Cli) -> Self {
            MyServer {
                address: SocketAddr::new("127.0.0.1".parse().unwrap(), cli.port),
                pool_connections: Arc::new(ConnectionPool::new(cli)),
                secret_key: cli.secret_key.clone(),
                cli: cli.clone(),
                cert_file: cli.crt_path.clone(),
                key_file: cli.key_path.clone(),
            }
        }
        
        pub fn tls_cfg(cert_file: &str, key_file: &str) -> Result<SslAcceptorBuilder, Box<dyn std::error::Error>> {
            let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
            builder.set_private_key_file(key_file, SslFiletype::PEM)?;
            builder.set_certificate_chain_file(cert_file)?;
            Ok(builder)
        }

        async fn websocket_handler(req: HttpRequest, stream: web::Payload, key: web::Data<String>) -> Result<HttpResponse, Error> {
            if let Some(request_key) = req.headers().get("Secret-Key") {
                if request_key.to_str().ok() != Some(key.as_str()) {
                    return Err(error::ErrorBadRequest("Invalid header value"));
                }
            } else {
                return Err(error::ErrorBadRequest("Header missing"));
            }
            let host = req.headers().get("Host").unwrap().to_str().unwrap().to_string();
            
            ws::start(DataWebSocket::new(host), &req, stream)
        }

        async fn send_gossip_messages(&self) {
            let mut interval = tokio::time::interval(Duration::from_secs(self.cli.period));
            loop {
                interval.tick().await;
                let mut sended_urls = Vec::new();
                let message = format!("#Random message: {:?}#", rand::random::<u64>());
                for url in self.pool_connections.get_all_possible_conections().await {
                    let conn = self.pool_connections.get_connection(&url).await;
                    if let Err(_) = conn {
                        error!("Unable to connect to url: {}", url);
                        continue;
                    }
                    let conn = conn.unwrap();
                    let mut ws_stream = conn.lock().await;
                    if let Err(e) = ws_stream.send(WebSocketMessage::Text(message.clone())).await {
                        info!("Error sending message: {:?}", e);
                        continue;
                    }
                    sended_urls.push(url)
                }
                if sended_urls.is_empty() {
                    info!("Waiting connections")
                } else {
                    info!("Sended msg to urls: {}", sended_urls.join(", "))
                }
            }
        }

        async fn default_handler(_req: HttpRequest, _stream: web::Payload) -> Result<HttpResponse, Error> {
            Ok(HttpResponseBuilder::new(StatusCode::BAD_REQUEST).body("Invalid request"))
        }

        pub async fn run(self: Arc<Self>) -> std::io::Result<()> {
            let secret_key = self.secret_key.clone();
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(Data::new(secret_key.clone()))
                    .configure(Self::app_configs)
            })
            .bind_openssl(self.address, Self::tls_cfg(&self.cert_file, &self.key_file).unwrap())?
            //.bind(self.address)?
            .run();
            let server_handler = server.handle();

            let self_clone = self.clone();
            if self.cli.servers.is_some() {
                tokio::spawn(async move {
                    self_clone.send_gossip_messages().await;
                });
            }

            if let Some(servers) = &self.cli.servers {
                for address in servers.split(',') {
                    match self.pool_connections.get_connection(address).await {
                        Ok(_) => info!("Connected to {}", address),
                        Err(_) => continue
                    }
                }
            }
            
            tokio::select! {
                result = server => {
                    // Handle server result
                    info!("WTF, stopping server...");
                    let _ = result.map_err(|e| Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>);
                },
                _ = signal::ctrl_c() => {
                    // Handle Ctrl+C signal for graceful shutdown
                    info!("Signal SIGINT received, stopping server...");
                    server_handler.stop(true).await;
                }
            }

            Ok(())
        }

        fn app_configs(cfg: &mut web::ServiceConfig) {
            cfg.service(
                    web::scope("")
                        .service(web::resource("/ws").to(Self::websocket_handler))
                        .default_service(web::route().to(Self::default_handler)),
                );
        }
    }

    pub struct DataWebSocket{
        url: String,
    }

    impl DataWebSocket {
        pub fn new(url: String) -> Self {
            DataWebSocket { url }
        }
    }

    impl Actor for DataWebSocket {
        type Context = ws::WebsocketContext<Self>;
    }

    impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for DataWebSocket {
        fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
            match msg {
                Ok(ws::Message::Text(text)) => {
                    info!("Received message: {} from: {}", text, self.url);
                    //ctx.text(text);
                }
                Ok(ws::Message::Binary(bin)) => {
                    let mut gz = GzDecoder::new(&bin[..]);
                    let mut s = String::new();
                    match gz.read_to_string(&mut s) {
                        Ok(_) => {
                            debug!("Decompressed message: {}", s);
                        }
                        Err(e) => {
                            error!("Error decompressing message: {:?}", e);
                        }
                    }
                    info!("Received message: {} from : {}", s, self.url);
                }
                Ok(ws::Message::Ping(msg)) => {
                    ctx.pong(&msg);
                }
                Ok(ws::Message::Pong(_)) => {
                    info!("pong!");
                }
                Ok(ws::Message::Close(reason)) => {
                    debug!("Connection closed, reason: {:?}", reason);
                    ctx.close(reason);
                    ctx.stop();
                }
                Ok(ws::Message::Nop) => {}
                Ok(ws::Message::Continuation(_)) => {}
                Err(e) => {
                    error!("Error in WebSocket message handling: {:?}", e);
                    ctx.stop();
                }
            }
        }
    }
}