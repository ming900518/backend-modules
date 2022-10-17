use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use jsonwebtoken::{encode, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;
use uuid::Uuid;

use base_library::{
    default_fallback, err_json_gen, get_db_err, get_jwt_exp_timestamp, now_local_time, Claims, CustomJsonRequest, UserToken, USER_KEY
};

pub fn router() -> Router {
    Router::new().nest(
        "/user",
        Router::new()
            .route("/query", get(query))
            .route("/save", put(save))
            .route("/login", post(login))
            .fallback(default_fallback),
    )
}

#[derive(Deserialize)]
struct LoginReq {
    account: Option<String>,
    password: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, sqlx::FromRow)]
struct UserInfo {
    #[serde(skip_deserializing)]
    uuid: Uuid,
    #[serde(skip_deserializing)]
    login_account: String,
    login_password: String,
    #[serde(skip_deserializing)]
    account_rule: i32,
    #[serde(skip_deserializing)]
    account_status: bool,
    user_name: String,
    user_email: Option<String>,
    note: Option<String>,
    #[serde(
        skip_deserializing,
        default = "now_local_time",
        with = "time::serde::iso8601"
    )]
    creation_timestamp: OffsetDateTime,
    #[serde(
        skip_deserializing,
        default = "now_local_time",
        with = "time::serde::iso8601"
    )]
    update_timestamp: OffsetDateTime,
}

/// 查詢用戶資訊（用戶）
async fn query(
    UserToken(user_token): UserToken,
    Extension(ref db): Extension<Pool<Postgres>>
) -> impl IntoResponse {
    match sqlx::query_as!(
        UserInfo,
        "select * from backendmodulesdb.user_info where uuid = $1",
        user_token.uuid
    )
    .fetch_one(db)
    .await
    {
        Ok(user_info) => Ok(Json::from(json!(user_info))),
        Err(error) => Err(get_db_err(error)),
    }
}

/// 儲存用戶資訊（更新）
async fn save(
    UserToken(user_token): UserToken,
    Extension(ref db): Extension<Pool<Postgres>>,
    CustomJsonRequest(params): CustomJsonRequest<UserInfo>,
) -> impl IntoResponse {
        let query = sqlx::query_as!(
            UserInfo,
            r#"
        update backendmodulesdb.user_info
        set login_password = $2,
            user_name = $3,
            user_email = $4,
            note = $5,
            update_timestamp = $6
        where uuid = $1 returning *;
        "#,
            user_token.uuid,
            params.login_password,
            params.user_name,
            params.user_email,
            params.note,
            params.update_timestamp
        )
        .fetch_one(db)
        .await;
        match query {
            Ok(result) => Ok(Json::from(json!(result))),
            Err(error) => Err(get_db_err(error)),
        }
}

async fn login(
    Extension(ref db): Extension<Pool<Postgres>>,
    CustomJsonRequest(request): CustomJsonRequest<LoginReq>,
) -> impl IntoResponse {
    let account_empty = request.account.is_none();
    let password_empty = request.password.is_none();
    if account_empty || password_empty {
        return Err(err_json_gen(
            StatusCode::UNPROCESSABLE_ENTITY,
            if account_empty && password_empty {
                Some("Both account and password is empty.".to_string())
            } else if account_empty {
                Some("Account is empty.".to_string())
            } else {
                Some("Password is empty.".to_string())
            },
        ));
    }
    match sqlx::query_as!(
        UserInfo,
        "select * from backendmodulesdb.user_info where login_account = $1",
        request.account
    )
    .fetch_all(db)
    .await
    {
        Ok(admin_vec) => {
            let successful_count = admin_vec
                .iter()
                .filter(|user_info| user_info.login_password == request.password.clone().unwrap())
                .filter(|user_info| user_info.account_status)
                .count();
            if successful_count > 1 {
                Err(err_json_gen(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Some(
                        "More than one record passed authorization, which is probably a bug."
                            .to_string(),
                    ),
                ))
            } else if successful_count == 0 {
                Err(err_json_gen(
                    StatusCode::UNAUTHORIZED,
                    Some("Couldn't found your account.".to_string()),
                ))
            } else {
                let claims = Claims {
                    uuid: admin_vec.first().unwrap().uuid.to_owned(),
                    exp: get_jwt_exp_timestamp(),
                };
                match encode(&Header::default(), &claims, &USER_KEY.encoding) {
                    Ok(token) => Ok((StatusCode::OK, token)),
                    Err(error) => Err(err_json_gen(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some(error.to_string()),
                    )),
                }
            }
        }
        Err(error) => Err(get_db_err(error)),
    }
}
