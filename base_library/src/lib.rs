#![allow(dead_code)]
#![forbid(unsafe_code)]
extern crate core;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, FromRequestParts};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::request::Parts;
use axum::http::{Request, StatusCode};
use axum::response::{Html, IntoResponse};
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

pub const BAD_REQUEST_MSG: &str = "You sent a request that this server could not understand.";

pub const UNAUTHORIZED_MSG: &str = "This server could not verify that you are authorized to access the document requested. Either you supplied the wrong credentials (e.g., bad password), or server doesn't understand how to supply the credentials required.";

pub const NOT_FOUND_MSG: &str = "The requested resources was not found on this server.";

pub const UNSUPPORTED_MEDIA_TYPE_MSG: &str = "The server refused this request because the request entity is in a format not supported by the requested resource for the requested method.";

pub const UNPROCESSABLE_ENTITY_MSG: &str = "The server understands the content type of the request entity, but it was unable to process the contained instructions.";

pub const SERVICE_UNAVAILABLE_MSG: &str = "The server is temporarily unable to service your request due to maintenance downtime or capacity problems. Please try again later. Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.";

pub const GATEWAY_TIMEOUT_MSG: &str = "The server didn't respond in time. Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.";

pub const INTERNAL_SERVER_ERROR_MSG: &str = "The server encountered an internal error or misconfiguration and was unable to complete your request. Please contact the server administrator, mail@mingchang.tw and inform them of the time the error occurred, and anything you might have done that may have caused the error.";

pub static ADMIN_KEY: Lazy<Keys> = Lazy::new(|| {
    let secret = env::var("ADMIN_JWT_SECRET").unwrap_or_else(|_| "7W0nwzmIeFBj6gd-tZpwHw7tTZ8KJ9vp0fvhU9chRqcD74dPJy_KV_cqxFyjpmvEC0AJSENbMC5Pq03BfIA4mLR3pd_h1vKoB4mestDn0cx6gKULZXBVSTa3fUdvxGzDxY_IDRUUlpGRWp6loprqyvliO8aw0BzkN2BPD8qRN8M".to_owned());
    Keys::new(secret.as_bytes())
});

pub static USER_KEY: Lazy<Keys> = Lazy::new(|| {
    let secret = env::var("USER_JWT_SECRET").unwrap_or_else(|_| "8W0nwzmIeFBj6gd-tZpwHw7tTZ8KJ9vp0fvhU9chRqcD74dPJy_KV_cqxFyjpmvEC0AJSENbMC5Pq03BfIA4mLR3pd_h1vKoB4mestDn0cx6gKULZXBVSTa3fUdvxGzDxY_IDRUUlpGRWp6loprqyvliO8aw0BzkN2BPD8qRN9N".to_owned());
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
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        let req = Request::from_parts(parts, body);

        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => match &rejection {
                JsonRejection::JsonDataError(_) => {
                    Err(err_json_gen(StatusCode::UNPROCESSABLE_ENTITY, None))
                }
                JsonRejection::JsonSyntaxError(_) => {
                    Err(err_json_gen(StatusCode::BAD_REQUEST, None))
                }
                JsonRejection::MissingJsonContentType(_) => {
                    Err(err_json_gen(StatusCode::UNSUPPORTED_MEDIA_TYPE, None))
                }
                _ => Err(err_json_gen(StatusCode::INTERNAL_SERVER_ERROR, None)),
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Claims {
    pub uuid: Uuid,
    pub exp: u64,
}

#[derive(Deserialize)]
pub struct AdminToken(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AdminToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| {
                    err_json_gen(
                        StatusCode::UNAUTHORIZED,
                        Some("Unable to extract token from request. Please log in.".to_string()),
                    )
                })?;
        let token_data = decode::<AdminToken>(bearer.token(), &ADMIN_KEY.decoding, &Validation::default())
            .map_err(|_| {
                err_json_gen(
                    StatusCode::UNAUTHORIZED,
                    Some("Unable to parse token.".to_string()),
                )
            })?;

        if token_data.claims.0.exp
            < SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        {
            Err(err_json_gen(
                StatusCode::UNAUTHORIZED,
                Some("Token expired, please log in again.".to_string()),
            ))
        } else {
            Ok(token_data.claims)
        }
    }
}

#[derive(Deserialize)]
pub struct UserToken(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for UserToken
    where
        S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| {
                    err_json_gen(
                        StatusCode::UNAUTHORIZED,
                        Some("Unable to extract token from request. Please log in.".to_string()),
                    )
                })?;
        let token_data = decode::<UserToken>(bearer.token(), &USER_KEY.decoding, &Validation::default())
            .map_err(|_| {
                err_json_gen(
                    StatusCode::UNAUTHORIZED,
                    Some("Unable to parse token.".to_string()),
                )
            })?;

        if token_data.claims.0.exp
            < SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
        {
            Err(err_json_gen(
                StatusCode::UNAUTHORIZED,
                Some("Token expired, please log in again.".to_string()),
            ))
        } else {
            Ok(token_data.claims)
        }
    }
}

pub async fn default_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html::from(DEFAULT_FALLBACK_HTML))
}

pub fn err_json_gen(status_code: StatusCode, reason: Option<String>) -> (StatusCode, Json<Value>) {
    #[derive(Serialize)]
    struct ErrJson {
        code: u16,
        description: String,
        message: String,
        reason: Option<String>,
    }

    impl ErrJson {
        fn new(status_code: StatusCode, message: String, reason: Option<String>) -> ErrJson {
            ErrJson {
                code: status_code.as_u16(),
                description: status_code.canonical_reason().unwrap().to_string(),
                message,
                reason,
            }
        }
    }

    match status_code {
        StatusCode::UNAUTHORIZED => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                UNAUTHORIZED_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::UNPROCESSABLE_ENTITY => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                UNPROCESSABLE_ENTITY_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                UNSUPPORTED_MEDIA_TYPE_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::NOT_FOUND => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                NOT_FOUND_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::BAD_REQUEST => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                BAD_REQUEST_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::GATEWAY_TIMEOUT => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                GATEWAY_TIMEOUT_MSG.to_string(),
                reason
            ))),
        ),
        StatusCode::SERVICE_UNAVAILABLE => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                SERVICE_UNAVAILABLE_MSG.to_string(),
                reason
            ))),
        ),
        _ => (
            status_code,
            Json::from(json!(ErrJson::new(
                status_code,
                INTERNAL_SERVER_ERROR_MSG.to_string(),
                reason
            ))),
        ),
    }
}

pub fn get_db_err(error: Error) -> (StatusCode, Json<Value>) {
    match error {
        Error::RowNotFound | Error::ColumnNotFound(_) => {
            err_json_gen(StatusCode::NOT_FOUND, Some(error.to_string()))
        }
        Error::TypeNotFound { .. } | Error::ColumnIndexOutOfBounds { .. } => {
            err_json_gen(StatusCode::BAD_REQUEST, Some(error.to_string()))
        }
        Error::PoolTimedOut => err_json_gen(StatusCode::GATEWAY_TIMEOUT, Some(error.to_string())),
        Error::PoolClosed => err_json_gen(StatusCode::SERVICE_UNAVAILABLE, Some(error.to_string())),
        _ => err_json_gen(StatusCode::INTERNAL_SERVER_ERROR, Some(error.to_string())),
    }
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
