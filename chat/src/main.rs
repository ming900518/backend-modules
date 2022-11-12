#![forbid(unsafe_code)]
mod service;

use axum::routing::get;
use axum::Router;
use base_library::default_fallback;
use dotenvy::dotenv;
use mimalloc::MiMalloc;
use r2d2::Pool;
use redis::Client;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use service::websocket_handler;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub struct AppState {
    pub chatroom_set: Mutex<HashMap<String, HashSet<String>>>,
    pub tx: Mutex<HashMap<String, Sender<String>>>,
    pub pool: Pool<Client>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();

    let chatroom_set = Mutex::new(HashMap::new());
    let (tx, _rx) = broadcast::channel(100);

    let hash_map_tx = Mutex::new(HashMap::new());
    hash_map_tx
        .try_lock()
        .unwrap()
        .insert("root".to_string(), tx);

    let pool = Pool::builder()
        .max_size(100)
        .build(
            Client::open("redis://127.0.0.1/")
                .expect("Could not connect to Redis, is Redis ready?"),
        )
        .unwrap();

    let app_state = Arc::new(Mutex::new(AppState {
        chatroom_set,
        tx: hash_map_tx,
        pool,
    }));

    let app = Router::with_state(app_state)
        .route("/", get(websocket_handler))
        .fallback(default_fallback);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3501));

    println!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Server startup failed.");
}
