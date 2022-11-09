use crate::ApnsClientParam;
use a2::{Client, NotificationBuilder, PlainNotificationBuilder};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use base_library::{err_json_gen, CustomJsonRequest};
use serde::Deserialize;
use serde_json::json;
use tokio::fs::File;
use tokio::task;

// Badge count must be processed in iOS app (We don't really want to store all user's badge count on backend service), see https://stackoverflow.com/a/53159748 for instructions.
#[derive(Deserialize)]
pub struct ApnsNotificationRequest {
    device_token: String,
    content: String,
}

pub async fn sent_apple_notification(
    Extension(apns_client_params): Extension<ApnsClientParam>,
    CustomJsonRequest(request): CustomJsonRequest<ApnsNotificationRequest>,
) -> impl IntoResponse {
    match File::open(apns_client_params.pkcs8_pem_dir).await {
        Ok(file) => {
            let client = Client::token(
                file.into_std().await,
                apns_client_params.key_id,
                apns_client_params.team_id,
                apns_client_params.endpoint).unwrap();
            task::spawn(async move { client.send(PlainNotificationBuilder::new(&request.content).build(&request.device_token, Default::default())).await });
            (StatusCode::OK, Json(json!("{'description', 'APN has been sent.'}")))
        }
        Err(error) => err_json_gen(StatusCode::INTERNAL_SERVER_ERROR, Some(format!("Error when accessing APNs token, please inform system administrator about this error. Error returned from library: {}", error)))
    }
}
