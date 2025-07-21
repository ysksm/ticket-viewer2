pub mod changelog_parser;
pub mod client;
pub mod config_store;
pub mod duckdb_store;
pub mod error;
pub mod history;
pub mod json_store;
pub mod models;
pub mod persistence;
pub mod sync;
pub mod time_filter;

pub use client::{Auth, JiraClient, JiraConfig};
pub use error::Error;
pub use models::*;

// Sync module re-exports
pub use sync::{
    SyncConfig, SyncResult, SyncService, SyncState, SyncServiceStats, 
    ProjectSyncStats
};

// Time filter module re-exports  
pub use time_filter::{
    TimeBasedFilter, TimeChunk, parse_jira_datetime
};

// Persistence module re-exports
pub use persistence::{
    PersistenceStore, IssueFilter, DateRange, SortOrder, 
    StorageStats, FilterConfig
};

// JSON store re-export
pub use json_store::JsonStore;

// DuckDB store re-export
pub use duckdb_store::DuckDBStore;

// Config store re-exports
pub use config_store::{ConfigStore, FileConfigStore, AppConfig};

// History re-exports
pub use history::{
    IssueHistory, HistoryAuthor, HistoryFilter, HistoryStats,
    ChangeType, HistorySortOrder
};

// Changelog parser re-export
pub use changelog_parser::ChangelogParser;
