use std::{
    env,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};
use tracing::{Level, error, info, trace, warn};
use utils::{receive_data, send_data};

const PORT: u16 = 6969;

#[async_std::main]
async fn main() {
    let mut verbose_level = 0u8;
    let args: Vec<String> = env::args().collect();
    let mut portstr = PORT.to_string();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                "port" => portstr = args[i + 1].clone(),
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    'p' => {
                        portstr = args[argindex].clone();
                        argindex += 1;
                    }
                    _ => panic!("Pre-init failure; unknown short-name argument: {arg}"),
                }
            }
        }
    }
    let log_level = match verbose_level {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap_or_else(|_| {
        tracing_subscriber::fmt().init();
    });
    let port = match portstr.parse() {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to parse port: {}. Defaulting to {}", e, PORT);
            PORT
        }
    };
    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to port {}: {}", port, e);
            std::process::exit(1);
        }
    };
    info!("Listening on port {}", port);
    let server = Server::new();
    for stream in listener.incoming() {
        match stream {
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
            }
            Ok(stream) => {
                trace!(
                    "New connection from {}:{}",
                    stream.peer_addr().unwrap().ip(),
                    stream.peer_addr().unwrap().port(),
                );
                let serverclone = server.clone();
                async_std::task::spawn(async move {
                    serverclone.handle_client(stream).await;
                });
            }
        }
    }
}

#[derive(Clone)]
struct Server {
    clients: Arc<Mutex<Vec<(TcpStream, String)>>>,
}

impl Server {
    fn new() -> Server {
        Server {
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }
    async fn handle_client(&self, stream: TcpStream) {
        println!("New connection from {}", stream.peer_addr().unwrap());
        self.clients
            .lock()
            .unwrap()
            .push((stream.try_clone().unwrap(), String::new()));
        loop {
            let message = receive_data(&stream);
            if message.is_empty() {
                break;
            }
            self.broadcast(&message, &stream);
        }
        self.clients
            .lock()
            .unwrap()
            .retain(|c| c.0.peer_addr().unwrap() != stream.peer_addr().unwrap());
    }
    fn broadcast(&self, message: &[u8], stream: &TcpStream) {
        let idx = message.iter().position(|&c| c == 0).unwrap();
        let (username, messagedata) = message.split_at(idx);
        if String::from_utf8_lossy(messagedata).trim().is_empty() {
            return;
        }
        info!(
            "Received message from {} ({}): {}",
            String::from_utf8_lossy(username),
            stream.peer_addr().unwrap(),
            String::from_utf8_lossy(messagedata)
        );
        let clients = self.clients.lock().unwrap();
        let current = stream.peer_addr().unwrap();
        let current = current.ip().to_string() + ":" + &current.port().to_string();
        for client in clients.iter() {
            let clientid = client.0.peer_addr().unwrap().ip().to_string()
                + ":"
                + &client.0.peer_addr().unwrap().port().to_string();
            if clientid != current {
                send_data(message, &client.0);
            }
        }
    }
}
