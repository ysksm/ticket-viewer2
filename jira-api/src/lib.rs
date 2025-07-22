//! # JIRA API クライアントライブラリ
//!
//! このライブラリは、JIRA REST API v3の主要なエンドポイントをサポートするRustクライアントです。
//!
//! ## 主な機能
//!
//! - **認証**: Basic認証とBearer認証をサポート
//! - **検索**: JQLクエリによるIssue検索
//! - **メタデータ**: プロジェクト、優先度、Issue種別、フィールド、ステータスカテゴリ
//! - **ユーザー検索**: ユーザーアカウントの検索
//! - **同期機能**: 増分データ同期、時間ベースフィルタリング
//! - **データ永続化**: JSON（圧縮対応）およびDuckDB形式での保存
//! - **変更履歴**: Issue変更履歴の取得と管理
//! - **設定管理**: 認証情報とフィルター条件の永続化
//!
//! ## サポートAPIエンドポイント
//!
//! - `/rest/api/3/search` - Issue検索
//! - `/rest/api/3/project` - プロジェクト一覧
//! - `/rest/api/3/priority` - 優先度一覧
//! - `/rest/api/3/issuetype` - Issue種別一覧
//! - `/rest/api/3/field` - フィールド一覧
//! - `/rest/api/3/statuscategory` - ステータスカテゴリ一覧
//! - `/rest/api/3/users/search` - ユーザー検索
//!
//! ## 基本的な使用例
//!
//! ```rust,no_run
//! use jira_api::{JiraConfig, JiraClient, Auth};
//!
//! # tokio_test::block_on(async {
//! // 設定を作成
//! let config = JiraConfig::new(
//!     "https://your-instance.atlassian.net".to_string(),
//!     Auth::Basic {
//!         username: "your-email@example.com".to_string(),
//!         api_token: "your-api-token".to_string(),
//!     },
//! )?;
//!
//! // クライアントを初期化
//! let client = JiraClient::new(config)?;
//!
//! // プロジェクト一覧を取得
//! let projects = client.get_projects().await?;
//! println!("Found {} projects", projects.len());
//!
//! // Issue検索
//! let search_result = client.search_issues(
//!     "project = PROJ ORDER BY created DESC",
//!     jira_api::SearchParams::new().max_results(10)
//! ).await?;
//! println!("Found {} issues", search_result.total);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! ## 環境変数による設定
//!
//! ```rust,no_run
//! # std::env::set_var("JIRA_URL", "https://example.atlassian.net");
//! # std::env::set_var("JIRA_USER", "user@example.com");
//! # std::env::set_var("JIRA_API_TOKEN", "token123");
//! use jira_api::{JiraConfig, JiraClient};
//!
//! # tokio_test::block_on(async {
//! // 環境変数から設定をロード
//! let config = JiraConfig::from_env()?;
//! let client = JiraClient::new(config)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! ## データ永続化
//!
//! ```rust,no_run
//! use jira_api::{JsonStore, DuckDBStore, PersistenceStore};
//! use tempfile::TempDir;
//!
//! # tokio_test::block_on(async {
//! let temp_dir = TempDir::new()?;
//!
//! // JSONストア（圧縮対応）
//! let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
//! json_store.initialize().await?;
//!
//! // DuckDBストア（SQL対応）
//! let mut duckdb_store = DuckDBStore::new_in_memory()?;
//! duckdb_store.initialize().await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! ## 同期機能
//!
//! ```rust,no_run
//! use jira_api::{SyncService, SyncConfig, TimeBasedFilter};
//!
//! # tokio_test::block_on(async {
//! # let client = jira_api::JiraClient::new(
//! #     jira_api::JiraConfig::new(
//! #         "https://example.com".to_string(),
//! #         jira_api::Auth::Basic {
//! #             username: "user".to_string(),
//! #             api_token: "token".to_string()
//! #         }
//! #     )?
//! # )?;
//! // 同期設定
//! let config = SyncConfig::new()
//!     .interval_minutes(30)
//!     .max_history_count(10);
//!
//! let sync_service = SyncService::new(config);
//!
//! // 全体同期
//! let result = sync_service.sync_full(&client).await?;
//! println!("Synced {} issues", result.synced_issues_count);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```

pub mod changelog_parser;
pub mod client;
pub mod config_store;
pub mod duckdb_store;
pub mod error;
pub mod history;
pub mod json_store;
pub mod memory;
pub mod models;
pub mod persistence;
pub mod sync;
pub mod time_filter;

pub use client::{Auth, JiraClient, JiraConfig};
pub use error::Error;
pub use models::*;

// Sync module re-exports
pub use sync::{
    ProjectSyncStats, SyncConfig, SyncResult, SyncService, SyncServiceStats, SyncState,
};

// Time filter module re-exports
pub use time_filter::{TimeBasedFilter, TimeChunk, parse_jira_datetime};

// Persistence module re-exports
pub use persistence::{
    DateRange, FilterConfig, IssueFilter, PersistenceStore, SortOrder, StorageStats,
};

// JSON store re-export
pub use json_store::JsonStore;

// DuckDB store re-export
pub use duckdb_store::DuckDBStore;

// Config store re-exports
pub use config_store::{AppConfig, ConfigStore, FileConfigStore};

// History re-exports
pub use history::{
    ChangeType, HistoryAuthor, HistoryFilter, HistorySortOrder, HistoryStats, IssueHistory,
};

// Changelog parser re-export
pub use changelog_parser::ChangelogParser;

// Memory management re-exports
pub use memory::{
    DetailStatus, IssueLoader, IssueStream, IssueStreamLoader, LazyIssue, MemoryConfig, MemoryGC,
    MemoryPool, MemoryPoolGC, PoolStats, PooledObject,
};
