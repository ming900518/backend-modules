#![forbid(unsafe_code)]

mod apns_service;
mod mail_service;

use a2::Endpoint;
use axum::routing::post;
use axum::{Extension, Router};
use base_library::default_fallback;
use lettre::transport::smtp::authentication::Credentials;
use lettre::SmtpTransport;
use mimalloc::MiMalloc;
use std::env;
use std::net::SocketAddr;
use tracing::info;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Clone)]
pub struct ApnsClientParam {
    pkcs8_pem_dir: String,
    key_id: String,
    team_id: String,
    endpoint: Endpoint,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3500));

    let smtp_server = env::var("SMTP_SERVER").ok();
    let smtp_username = env::var("SMTP_USERNAME").ok();
    let smtp_password = env::var("SMTP_PASSWORD").ok();
    let apns_key_dir = env::var("APNS_KEY_DIR").ok();
    let apns_key_id = env::var("APNS_KEY_ID").ok();
    let apns_team_id = env::var("APNS_TEAM_ID").ok();

    let mailer = match (smtp_username.is_none(), smtp_password.is_none()) {
        (false, true) => {
            info!("SMTP username not set, email related function will be disabled.");
            None
        }
        (true, false) => {
            info!("SMTP password not set, email related function will be disabled.");
            None
        }
        (true, true) => {
            info!("Both SMTP username and password not set, email related function will be disabled.");
            None
        }
        (false, false) => Some(
            SmtpTransport::relay(&smtp_server.unwrap_or_else(|| "smtp.google.com".parse().unwrap()))
                .unwrap()
                .port(465)
                .credentials(Credentials::new(
                    smtp_username.unwrap(),
                    smtp_password.unwrap(),
                ))
                .build(),
        ),
    };

    let apns_client_params = match (
        apns_key_dir.is_none(),
        apns_key_id.is_none(),
        apns_team_id.is_none(),
    ) {
        (true, false, false) => {
            info!("APNs key directory not set, APNs related function will be disabled.");
            None
        }
        (true, true, false) => {
            info!("APNs key directory and KEY_ID not set, APNs related function will be disabled.");
            None
        }
        (true, false, true) => {
            info!("APNs key directory and TEAM_ID not set, APNs related function will be disabled.");
            None
        }
        (false, true, false) => {
            info!("APNs KEY_ID not set, APNs related function will be disabled.");
            None
        }
        (false, true, true) => {
            info!(
                "APNs KEY_ID and TEAM_ID not set, APNs related function will be disabled."
            );
            None
        }
        (false, false, true) => {
            info!("APNs TEAM_ID not set, APNs related function will be disabled.");
            None
        }
        (true, true, true) => {
            info!("APNs key directory, KEY_ID and TEAM_ID not set, APNs related function will be disabled.");
            None
        }
        (false, false, false) => Some(ApnsClientParam {
            pkcs8_pem_dir: apns_key_dir.unwrap(),
            key_id: apns_key_id.unwrap(),
            team_id: apns_team_id.unwrap(),
            endpoint: Endpoint::Production,
        }),
    };

    let mail_router = match mailer {
        None => Router::new(),
        Some(mailer) => Router::new()
            .route("/sentMail", post(mail_service::sent_mail))
            .layer(Extension(mailer)),
    };

    let apns_router = match apns_client_params {
        None => Router::new(),
        Some(apns_client_params) => Router::new()
            .route(
                "/sentAppleNotification",
                post(apns_service::sent_apple_notification),
            )
            .layer(Extension(apns_client_params)),
    };

    let app = Router::new().nest(
        "/notification",
        Router::new()
            .merge(mail_router)
            .merge(apns_router)
            .fallback(default_fallback),
    );

    println!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Server startup failed.");
}
