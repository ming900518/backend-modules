#![allow(dead_code)]
#![forbid(unsafe_code)]
extern crate core;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, FromRequestParts};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::request::Parts;
use axum::http::{Request, StatusCode};
use axum::{async_trait, Json, TypedHeader};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Validation};
use once_cell::sync::Lazy;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Uuid;
use sqlx::Error;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;
use uuid::{Context, Timestamp};

pub const DEFAULT_FALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html>
<title>404 Not Found</title>
<h1>404 Not Found</h1>
<p>The requested resources was not found on this server.</p>
</html>
"#;

pub const UNAUTHORIZED_MSG: &str = r#"401 Unauthorized
This server could not verify that you are authorized to access the document requested.
Either you supplied the wrong credentials (e.g., bad password), or server doesn't understand how to supply the credentials required.
"#;

pub const BAD_REQUEST_MSG: &str = r#"400 Bad Request
You sent a request that this server could not understand.
"#;

pub const NOT_FOUND_MSG: &str = r#"404 Not Found
The requested resources was not found on this server.
"#;

pub const UNSUPPORTED_MEDIA_TYPE_MSG: &str = r#"415 Unsupported Media Type
The server refused this request because the request entity is in a format not supported by the requested resource for the requested method.
"#;

pub const UNPROCESSABLE_ENTITY_MSG: &str = r#"422 Unprocessable Entity
The server understands the content type of the request entity, but it was unable to process the contained instructions.
"#;

pub const SERVICE_UNAVAILABLE_MSG: &str = r#"503 Service Unavailable
The server is temporarily unable to service your request due to maintenance downtime or capacity problems.
Please try again later.
Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.
"#;

pub const GATEWAY_TIMEOUT_MSG: &str = r#"504 Gateway Timeout
The server didn't respond in time.
Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.
"#;

pub const INTERNAL_SERVER_ERROR_MSG: &str = r#"500 Internal Server Error
The server encountered an internal error or misconfiguration and was unable to complete your request.
Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.
"#;

pub static KEYS: Lazy<Keys> = Lazy::new(|| {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "7W0nwzmIeFBj6gd-tZpwHw7tTZ8KJ9vp0fvhU9chRqcD74dPJy_KV_cqxFyjpmvEC0AJSENbMC5Pq03BfIA4mLR3pd_h1vKoB4mestDn0cx6gKULZXBVSTa3fUdvxGzDxY_IDRUUlpGRWp6loprqyvliO8aw0BzkN2BPD8qRN8M".to_owned());
    Keys::new(secret.as_bytes())
});

pub struct Keys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl Keys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

pub struct CustomJsonRequest<T>(pub T);

#[async_trait]
impl<S, B, T> FromRequest<S, B> for CustomJsonRequest<T>
where
    Json<T>: FromRequest<S, B, Rejection = JsonRejection>,
    S: Send + Sync,
    B: Send + 'static,
    T: Send,
{
    type Rejection = (StatusCode, Json<String>);

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        let req = Request::from_parts(parts, body);

        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let error = match &rejection {
                    JsonRejection::JsonDataError(_) => {
                        (StatusCode::UNPROCESSABLE_ENTITY, UNPROCESSABLE_ENTITY_MSG)
                    }
                    JsonRejection::JsonSyntaxError(_) => (StatusCode::BAD_REQUEST, BAD_REQUEST_MSG),
                    JsonRejection::MissingJsonContentType(_) => (
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        UNSUPPORTED_MEDIA_TYPE_MSG,
                    ),
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_SERVER_ERROR_MSG),
                };
                Err((error.0, Json::from(error.1.to_string())))
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Claims {
    pub uuid: Uuid,
    pub exp: u64,
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| create_authorization_err())?;
        let token_data = decode::<Claims>(bearer.token(), &KEYS.decoding, &Validation::default())
            .map_err(|_| create_authorization_err())?;

        Ok(token_data.claims)
    }
}

pub fn get_db_err(error: Error) -> (StatusCode, Json<Value>) {
    match error {
        Error::RowNotFound | Error::ColumnNotFound(_) => {
            (StatusCode::NOT_FOUND, Json::from(json!(NOT_FOUND_MSG)))
        }
        Error::TypeNotFound { .. } | Error::ColumnIndexOutOfBounds { .. } => {
            (StatusCode::BAD_REQUEST, Json::from(json!(BAD_REQUEST_MSG)))
        }
        Error::PoolTimedOut => (
            StatusCode::GATEWAY_TIMEOUT,
            Json::from(json!(GATEWAY_TIMEOUT_MSG)),
        ),
        Error::PoolClosed => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json::from(json!(SERVICE_UNAVAILABLE_MSG)),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json::from(json!(INTERNAL_SERVER_ERROR_MSG)),
        ),
    }
}

pub fn create_authorization_err() -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json::from(json!(UNAUTHORIZED_MSG)),
    )
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub page: Option<u64>,
    pub count: Option<u64>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct PaginationResp {
    total_pages: u64,
    current_page: u64,
    count: u64,
    value: Value,
}

impl PaginationResp {
    pub fn new(total: u64, count: u64, page: u64, value: Value) -> PaginationResp {
        PaginationResp {
            total_pages: (total as f32 / count as f32).ceil() as u64,
            current_page: page,
            count,
            value,
        }
    }
}

/// 計算分頁查詢的偏移
pub fn pagination_offset(page: u64, count: u64) -> u64 {
    if page.eq(&1) {
        0
    } else {
        (page - 1) * count
    }
}

/// 產生第一版UUID（本專案用於資料庫的主鍵）
pub fn new_uuid_v1() -> Uuid {
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let ctx = Context::new(rand::thread_rng().gen());
    let ts = Timestamp::from_unix(ctx, duration.as_secs(), duration.subsec_nanos());
    let array: [u8; 6] = rand::random();
    Uuid::new_v1(ts, &array)
}

/// 產生現在時間（如果沒辦法確認就改用UTC）
pub fn now_local_time() -> OffsetDateTime {
    OffsetDateTime::now_local()
        .ok()
        .unwrap_or_else(OffsetDateTime::now_utc)
}

/// 產生JWT過期時間（目前設定一天）
pub fn get_jwt_exp_timestamp() -> u64 {
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    (since_the_epoch + Duration::from_secs(86400)).as_secs()
}
