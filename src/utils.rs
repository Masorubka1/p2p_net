pub(crate) mod cli {
    use std::{ sync::Arc, time::Instant};

    use clap::Parser;
    use tracing::info;
    use tracing_subscriber::{fmt::time::FormatTime, EnvFilter};
    use tracing_appender::{non_blocking::WorkerGuard, non_blocking, rolling::daily};

    #[derive(Parser, Clone)]
    #[command(version, about, long_about = None)]
    pub(crate) struct Cli {
        /// Period of sending messages
        #[arg(long, value_name = "period")]
        pub period: u64,

        /// Port starting servier
        #[arg(long, value_name = "port")]
        pub port: u16,

        /// lits of servers that we want to connect(host + port)
        /// separate - ";"
        #[arg(long, value_name = "connect")]
        pub servers: Option<String>,

        /// path to log file
        #[arg(long, value_name = "file", default_value = "./p2p_log")]
        pub path: String,

        /// keep_alive_connection
        #[arg(long, value_name = "keep_alive", default_value = "10")]
        pub keep_alive: u64,

        /// secret_key for connection
        #[arg(long, value_name = "secret_key", default_value = "SOME_KEY")]
        pub secret_key: String,

        /// path to pem crt sertificate
        #[arg(long, value_name = "crt_path")]
        pub crt_path: String,

        /// path to pem key sertificate
        #[arg(long, value_name = "key_path")]
        pub key_path: String,

        /// path to pem pfx sertificate
        #[arg(long, value_name = "pfx_path")]
        pub pfx_path: String,

        /// secret_key for signed pem sertificate
        #[arg(long, value_name = "password", default_value = "password")]
        pub password: String,
    }
    pub type SafeConfig = Arc<Cli>;


    pub(crate) fn parse_config() -> (SafeConfig, WorkerGuard) {
        let cli = Cli::parse();
        
        // TODO: setup loging via cfg
        let flt = &format!(
            "{},mio=info,actix_server=info,actix_http=info,actix_codec=info,actix_tls=info,hyper=info",
            "info"
        );

        let log_path = match cli.path.rfind('/').map(|index| cli.path.split_at(index)) {
            Some((str1, str2)) => vec![str1, &str2[1..]],
            None => vec![".", &cli.path[1..]]
        };
        info!("log_path: {:?}", log_path);
        let _guard = init_tracing(false, flt, log_path[0], log_path[1]);
        (Arc::new(cli), _guard)
    }
    struct UptimeTimer {
        start: Instant,
    }
    
    impl UptimeTimer {
        fn new() -> Self {
            UptimeTimer { start: Instant::now() }
        }
    }
    
    impl FormatTime for UptimeTimer {
        fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
            let elapsed = Instant::now().duration_since(self.start);
            write!(
                w,
                "{:02}:{:02}:{:02}",
                elapsed.as_secs() / 3600,
                (elapsed.as_secs() / 60) % 60,
                elapsed.as_secs() % 60
            )
        }
    }

    pub fn init_tracing(
        stdout: bool,
        filter_str: &str,
        log_dir: &str,
        file_prefix: &str,
    ) -> WorkerGuard {
        let filter = EnvFilter::try_new(filter_str).unwrap();
        let (writer, guard) = if stdout {
            let (writer, guard) = non_blocking(std::io::stdout());
            (writer, guard)
        } else {
            let file_appender = daily(log_dir, file_prefix);
            let (writer, guard) = non_blocking(file_appender);
            (writer, guard)
        };

        let timer = UptimeTimer::new();

        tracing_subscriber::fmt()
            .with_writer(writer)
            .with_timer(timer)
            .with_env_filter(filter)
            .with_ansi(stdout)
            .with_target(false)
            .with_file(false)
            .with_thread_names(false)
            .with_line_number(false)
            .init();
    
        guard
    }
}