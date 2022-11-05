#![forbid(unsafe_code)]

mod service;

use axum::Router;
use base_library::default_fallback;
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub static SMTP_SERVER: Lazy<String> =
    Lazy::new(|| env::var("SMTP_SERVER").unwrap_or_else(|_| "smtp.gmail.com".to_string()));

pub static SMTP_USERNAME: Lazy<String> =
    Lazy::new(|| env::var("SMTP_USERNAME").unwrap_or_else(|_| String::default()));

pub static SMTP_PASSWORD: Lazy<String> =
    Lazy::new(|| env::var("SMTP_PASSWORD").unwrap_or_else(|_| String::default()));

pub static EMAIL_FUNCTIONALITY: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let app = Router::new()
        .merge(service::router())
        .fallback(default_fallback);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3500));

    if SMTP_USERNAME.len() != 0 || SMTP_PASSWORD.len() != 0 {
        EMAIL_FUNCTIONALITY.store(true, Ordering::Relaxed)
    } else {
        info!("[INFO] Required SMTP settings not set, email related function will be disabled.");
    }

    println!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Server startup failed.");
}
