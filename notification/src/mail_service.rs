use axum::http::StatusCode;
use axum::response::IntoResponse;

use axum::{Extension, Json};
use base_library::{err_json_gen, CustomJsonRequest};
use lettre::message::Mailbox;
use lettre::{Address, Message, SmtpTransport, Transport};
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;
use tokio::task;

#[derive(Deserialize)]
pub struct MailRequest {
    from_address: String,
    to_address: String,
    title: String,
    content: String,
}

pub async fn sent_mail(
    Extension(mailer): Extension<SmtpTransport>,
    CustomJsonRequest(request): CustomJsonRequest<MailRequest>,
) -> impl IntoResponse {
    match (
        (Address::from_str(&request.from_address)),
        (Address::from_str(&request.to_address)),
    ) {
        (Ok(from_address), Ok(to_address)) => {
            task::spawn(async move {
                mailer.send(
                    &Message::builder()
                        .from(Mailbox::new(None, from_address))
                        .to(Mailbox::new(None, to_address))
                        .subject(&request.title)
                        .body(request.content)
                        .unwrap(),
                )
            });
            (
                StatusCode::OK,
                Json(json!("{'description', 'Mail has been sent.'}")),
            )
        }
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
        )
    }
}
