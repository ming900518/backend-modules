use crate::{EMAIL_FUNCTIONALITY, SMTP_PASSWORD, SMTP_SERVER, SMTP_USERNAME};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use base_library::{default_fallback, err_json_gen, CustomJsonRequest};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use serde::Deserialize;
use serde_json::Value;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use tokio::task;

pub fn router() -> Router {
    Router::new().nest(
        "/notification",
        Router::new()
            .route("/sentRegMail", post(sent_reg_mail))
            .route("/sentResetPwMail", post(sent_reset_pw_mail))
            .route("/sentAppleNotification", post(sent_apple_notification))
            .fallback(default_fallback),
    )
}

#[derive(Deserialize)]
struct RegistrationMailRequest {
    from_address: String,
    to_address: String,
    user_info: RegistrationUserInfo,
}

#[derive(Deserialize)]
struct RegistrationUserInfo {
    registration_link: String,
    user_name: String,
}

async fn sent_reg_mail(
    CustomJsonRequest(request): CustomJsonRequest<RegistrationMailRequest>,
) -> impl IntoResponse {
    match (
        (Address::from_str(&request.from_address)),
        (Address::from_str(&request.to_address)),
    ) {
        (Ok(from_address), Ok(to_address)) => sent_email(
            Message::builder()
                .from(Mailbox::new(None, from_address))
                .to(Mailbox::new(None, to_address))
                .subject("XXX平臺 - 註冊驗證信")
                .body(format!(
                    "{}您好，感謝您註冊XXX平臺帳戶。\n請點選以下連結進行帳號驗證：\n{}",
                    request.user_info.user_name, request.user_info.registration_link
                ))
                .unwrap(),
        ),
        (Ok(_), Err(err)) => err_json_gen(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(format!("to_address could not be parsed: {}.", err)),
        ),
        (Err(err), Ok(_)) => err_json_gen(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(format!("from_address could not be parsed: {}.", err)),
        ),
        (Err(err1), Err(err2)) => err_json_gen(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(format!(
                "from_address could not be parsed: {}. to_address could not be parsed: {}.",
                err1, err2
            )),
        ),
    }
}

async fn sent_reset_pw_mail() -> impl IntoResponse {
    (StatusCode::OK, Json::from(Value::default()))
}

async fn sent_apple_notification() -> impl IntoResponse {
    (StatusCode::OK, Json::from(Value::default()))
}

fn sent_email(email: Message) -> (StatusCode, Json<Value>) {
    match EMAIL_FUNCTIONALITY.load(Ordering::Relaxed) {
        true => {
            let creds = Credentials::new(
                SMTP_USERNAME.get(0..).unwrap().to_string(),
                SMTP_PASSWORD.get(0..).unwrap().to_string(),
            );
            let mailer = SmtpTransport::relay(SMTP_SERVER.get(0..).unwrap())
                .unwrap()
                .port(465)
                .credentials(creds)
                .build();
            task::spawn(async move { mailer.send(&email) });
            (StatusCode::OK, Json(Value::default()))
        }
        false => err_json_gen(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("Email functionality has been disabled.".to_string()),
        ),
    }
}
