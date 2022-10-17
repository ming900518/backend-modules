use axum::{Extension, Json, Router};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put, delete};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;
use uuid::Uuid;
use base_library::{AdminToken, CustomJsonRequest, default_fallback, err_json_gen, get_db_err, now_local_time, pagination_offset, PaginationParams, PaginationResp, new_uuid_v1};

pub fn router() -> Router {
    Router::new().nest(
        "/user/management",
        Router::new()
            .route("/list", get(list))
            .route("/query/:uuid", get(query))
            .route("/save", put(save))
            .route("/delete/:uuid", delete(remove))
            .fallback(default_fallback),
    )
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, sqlx::FromRow)]
struct UserInfo {
    uuid: Uuid,
    login_account: String,
    login_password: String,
    account_rule: i32,
    account_status: bool,
    user_name: String,
    user_email: String,
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

async fn query(
    AdminToken(_): AdminToken,
    Extension(ref db): Extension<Pool<Postgres>>,
    Path(search_uuid): Path<Uuid>,
) -> impl IntoResponse {
    match sqlx::query_as!(
        UserInfo,
        "select * from backendmodulesdb.user_info where uuid = $1",
        search_uuid
    )
        .fetch_one(db)
        .await
    {
        Ok(user_info) => Ok(Json::from(json!(user_info))),
        Err(error) => Err(get_db_err(error)),
    }
}

/// 查詢管理員列表（分頁查詢，無提供參數則使用預設值）
async fn list(
    AdminToken(_): AdminToken,
    Extension(ref db): Extension<Pool<Postgres>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let count = params.count.unwrap_or(5);
    let total = sqlx::query!("select count(*) from backendmodulesdb.user_info;")
        .fetch_one(db)
        .await
        .unwrap()
        .count
        .unwrap_or(0) as u64;
    let offset = pagination_offset(page, count);
    match sqlx::query_as!(
        UserInfo,
        r#"select * from backendmodulesdb.user_info order by creation_timestamp limit $1 offset $2;"#,
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
    AdminToken(_): AdminToken,
    Extension(ref db): Extension<Pool<Postgres>>,
    CustomJsonRequest(params): CustomJsonRequest<UserInfo>,
) -> impl IntoResponse {
    if params.uuid == Uuid::default() {
        match sqlx::query!(
            "select count(*) from backendmodulesdb.user_info where login_account = $1",
            params.login_account
        )
            .fetch_one(db)
            .await
        {
            Ok(record) => {
                if record.count.unwrap() == 0 {
                    let query = sqlx::query_as!(
                        UserInfo,
                        r#"
            insert into backendmodulesdb.user_info (
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
            UserInfo,
            r#"
        update backendmodulesdb.user_info
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
    AdminToken(_): AdminToken,
    Extension(ref db): Extension<Pool<Postgres>>,
    Path(uuid): Path<Uuid>,
) -> impl IntoResponse {
    match sqlx::query_as!(
        UserInfo,
        "delete from backendmodulesdb.user_info where uuid = $1 returning *;",
        uuid
    )
        .fetch_one(db)
        .await
    {
        Ok(user_info) => Ok((StatusCode::OK, user_info.uuid.to_string())),
        Err(error) => Err(get_db_err(error)),
    }
}


