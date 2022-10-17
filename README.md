# 後端模組
| 項目    | 框架/語言      |
|-------|------------|
| 程式語言  | Rust       |
| 資料庫   | PostgreSQL |
| 後端框架  | Axum       |
| 資料庫框架 | SQLx       |

採用[Axum框架](https://github.com/tokio-rs/axum)與[SQLx](https://github.com/launchbadge/sqlx)製作的後端模組。

預設使用[Musl standard library](https://musl.libc.org/)與[mimalloc](https://github.com/microsoft/mimalloc)，方便利用Alpine Linux或scratch base image製作小型Docker Image，並[兼顧多執行緒性能](https://www.linkedin.com/pulse/testing-alternative-c-memory-allocators-pt-2-musl-mystery-gomes)。

## 模組介紹
- base_library：共用Library，以下的每一個模組都依賴此crate。
- admin_info：提供管理員帳號的CRUD與登入功能（JWT）。
- user_info：提供用戶帳號的CRUD與登入功能（JWT）。

（持續更新）
