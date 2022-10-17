use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json, Router};
use jsonwebtoken::{encode, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;
use uuid::Uuid;

use base_library::{
    err_json_gen, get_db_err, get_jwt_exp_timestamp, new_uuid_v1, now_local_time,
    pagination_offset, Claims, CustomJsonRequest, PaginationParams, PaginationResp, Token, KEYS,
};

use crate::default_fallback;

pub fn router() -> Router {
    Router::new().nest(
        "/admin",
        Router::new()
            .route("/list", get(list))
            .route("/query/:uuid", get(query))
            .route("/save", put(save))
            .route("/delete/:uuid", delete(remove))
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
struct AdminInfo {
    #[serde(default)]
    uuid: Uuid,
    login_account: String,
    login_password: String,
    account_rule: i32,
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

/// 查詢管理員資訊
async fn query(
    Token(_): Token,
    Extension(ref db): Extension<Pool<Postgres>>,
    Path(search_uuid): Path<Uuid>,
) -> impl IntoResponse {
    match sqlx::query_as!(
        AdminInfo,
        "select * from backendmodulesdb.admin_info where uuid = $1",
        search_uuid
    )
    .fetch_one(db)
    .await
    {
        Ok(admin_info) => Ok(Json::from(json!(admin_info))),
        Err(error) => Err(get_db_err(error)),
    }
}

/// 查詢管理員列表（分頁查詢，無提供參數則使用預設值）
async fn list(
    Token(_): Token,
    Extension(ref db): Extension<Pool<Postgres>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let count = params.count.unwrap_or(5);
    let total = sqlx::query!("select count(*) from backendmodulesdb.admin_info;")
        .fetch_one(db)
        .await
        .unwrap()
        .count
        .unwrap_or(0) as u64;
    let offset = pagination_offset(page, count);
    match sqlx::query_as!(
        AdminInfo,
        r#"select * from backendmodulesdb.admin_info order by creation_timestamp limit $1 offset $2;"#,
        count as i64,
        offset as i64
    )
        .fetch_all(db)
        .await
    {
        Ok(result) => Ok(Json::from(PaginationResp::new(
            total,
            count,
            page,
            json!(result),
        ))),
        Err(error) => Err(get_db_err(error)),
    }
}

/// 儲存管理員資訊（有提供UUID的情況更新，無則新增）
async fn save(
    Token(_): Token,
    Extension(ref db): Extension<Pool<Postgres>>,
    CustomJsonRequest(params): CustomJsonRequest<AdminInfo>,
) -> impl IntoResponse {
    if params.uuid == Uuid::default() {
        match sqlx::query!(
            "select count(*) from backendmodulesdb.admin_info where login_account = $1",
            params.login_account
        )
        .fetch_one(db)
        .await
        {
            Ok(record) => {
                if record.count.unwrap() == 0 {
                    let query = sqlx::query_as!(
                        AdminInfo,
                        r#"
            insert into backendmodulesdb.admin_info (
                uuid,
                login_account,
                login_password,
                account_rule,
                account_status,
                user_name,
                user_email,
                note,
                creation_timestamp,
                update_timestamp
            )
            values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10
            ) returning *;
        "#,
                        Uuid::from(new_uuid_v1()),
                        params.login_account,
                        params.login_password,
                        params.account_rule,
                        params.account_status,
                        params.user_name,
                        params.user_email,
                        params.note,
                        params.creation_timestamp,
                        params.update_timestamp
                    )
                    .fetch_one(db)
                    .await;
                    match query {
                        Ok(result) => Ok(Json::from(json!(result))),
                        Err(error) => Err(get_db_err(error)),
                    }
                } else {
                    Err(err_json_gen(
                        StatusCode::CONFLICT,
                        Some(
                            "Account with same name existed, please specify another name."
                                .to_string(),
                        ),
                    ))
                }
            }
            Err(error) => Err(get_db_err(error)),
        }
    } else {
        let query = sqlx::query_as!(
            AdminInfo,
            r#"
        update backendmodulesdb.admin_info
        set login_account = $2,
            login_password = $3,
            account_rule = $4,
            account_status = $5,
            user_name = $6,
            user_email = $7,
            note = $8,
            update_timestamp = $9
        where uuid = $1 returning *;
        "#,
            params.uuid,
            params.login_account,
            params.login_password,
            params.account_rule,
            params.account_status,
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
}

/// 移除管理員
async fn remove(
    Token(_): Token,
    Extension(ref db): Extension<Pool<Postgres>>,
    Path(uuid): Path<Uuid>,
) -> impl IntoResponse {
    match sqlx::query_as!(
        AdminInfo,
        "delete from backendmodulesdb.admin_info where uuid = $1 returning *;",
        uuid
    )
    .fetch_one(db)
    .await
    {
        Ok(admin_info) => Ok((StatusCode::OK, admin_info.uuid.to_string())),
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
        AdminInfo,
        "select * from backendmodulesdb.admin_info where login_account = $1",
        request.account
    )
    .fetch_all(db)
    .await
    {
        Ok(admin_vec) => {
            let successful_count = admin_vec
                .iter()
                .filter(|admin_info| admin_info.login_password == request.password.clone().unwrap())
                .filter(|admin_info| admin_info.account_status)
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
                match encode(&Header::default(), &claims, &KEYS.encoding) {
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
