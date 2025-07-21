pub mod client;
pub mod error;
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
