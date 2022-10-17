#![forbid(unsafe_code)]
mod user_service;

use axum::{Extension, Router};
use base_library::default_fallback;
use dotenvy::dotenv;
use mimalloc::MiMalloc;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();

    let db = PgPoolOptions::new()
        .max_connections(80)
        .connect(&*format!(
            "postgresql://{}:{}@localhost:5432/postgres",
            env::var("DB_USER").unwrap_or_else(|_| "postgres".parse().unwrap()),
            env::var("DB_PASS").unwrap_or_else(|_| "postgres".parse().unwrap())
        ))
        .await
        .expect("Database connection failed.");

    let app = Router::new()
        .merge(user_service::router())
        .layer(Extension(db))
        .fallback(default_fallback);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    println!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Server startup failed.");
}
