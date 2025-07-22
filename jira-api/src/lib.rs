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
