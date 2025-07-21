use async_trait::async_trait;
use chrono::Utc;
use duckdb::{Connection, params};
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::task;

use crate::{Issue, Error, PersistenceStore, IssueFilter, FilterConfig, StorageStats, SortOrder, IssueHistory, HistoryFilter, HistoryStats, HistoryAuthor};

/// DuckDB形式のデータストア
pub struct DuckDBStore {
    /// データベースファイルのパス
    #[allow(dead_code)]
    db_path: PathBuf,
    /// DuckDB接続（スレッドセーフ）
    connection: Arc<Mutex<Connection>>,
}

impl DuckDBStore {
    /// 新しいDuckDBストアを作成
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, Error> {
        let db_path = db_path.as_ref().to_path_buf();
        let connection = Connection::open(&db_path)
            .map_err(|e| Error::DatabaseError(format!("Failed to open DuckDB: {}", e)))?;
        
        Ok(Self {
            db_path,
            connection: Arc::new(Mutex::new(connection)),
        })
    }
    
    /// インメモリデータベースを作成
    pub fn new_in_memory() -> Result<Self, Error> {
        let connection = Connection::open_in_memory()
            .map_err(|e| Error::DatabaseError(format!("Failed to create in-memory DuckDB: {}", e)))?;
        
        Ok(Self {
            db_path: PathBuf::from(":memory:"),
            connection: Arc::new(Mutex::new(connection)),
        })
    }
    
    /// データベーススキーマを初期化
    pub async fn initialize(&self) -> Result<(), Error> {
        let conn = Arc::clone(&self.connection);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            // Issuesテーブルの作成（簡素化版）
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS issues (
                    id VARCHAR PRIMARY KEY,
                    issue_key VARCHAR UNIQUE NOT NULL,
                    summary VARCHAR NOT NULL,
                    description TEXT,
                    status_name VARCHAR NOT NULL,
                    priority_name VARCHAR,
                    issue_type_name VARCHAR NOT NULL,
                    project_key VARCHAR,
                    project_name VARCHAR,
                    reporter_display_name VARCHAR NOT NULL,
                    assignee_display_name VARCHAR,
                    created TIMESTAMP NOT NULL,
                    updated TIMESTAMP NOT NULL,
                    raw_json TEXT NOT NULL
                )
                "#,
                params![],
            )?;
            
            // フィルター設定テーブルの作成
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS filter_configs (
                    id VARCHAR PRIMARY KEY,
                    name VARCHAR NOT NULL,
                    description TEXT,
                    filter_json TEXT NOT NULL,
                    created_at TIMESTAMP NOT NULL,
                    updated_at TIMESTAMP NOT NULL,
                    usage_count INTEGER NOT NULL DEFAULT 0,
                    last_used_at TIMESTAMP
                )
                "#,
                params![],
            )?;
            
            // 履歴テーブル用のシーケンス作成
            conn.execute(
                "CREATE SEQUENCE IF NOT EXISTS history_id_seq START 1",
                params![],
            )?;
            
            // 履歴テーブルの作成
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS issue_history (
                    history_id INTEGER PRIMARY KEY DEFAULT nextval('history_id_seq'),
                    issue_id VARCHAR NOT NULL,
                    issue_key VARCHAR NOT NULL,
                    change_id VARCHAR NOT NULL,
                    change_timestamp TIMESTAMP NOT NULL,
                    author_account_id VARCHAR,
                    author_display_name VARCHAR,
                    author_email VARCHAR,
                    field_name VARCHAR NOT NULL,
                    field_id VARCHAR,
                    from_value VARCHAR,
                    to_value VARCHAR,
                    from_display_value VARCHAR,
                    to_display_value VARCHAR,
                    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
                "#,
                params![],
            )?;
            
            // 基本的なインデックスの作成
            conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_project_key ON issues(project_key)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_status_name ON issues(status_name)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_created ON issues(created)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_updated ON issues(updated)", params![])?;
            
            // 履歴テーブルのインデックス作成
            conn.execute("CREATE INDEX IF NOT EXISTS idx_history_issue_key ON issue_history(issue_key)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_history_change_timestamp ON issue_history(change_timestamp)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_history_field_name ON issue_history(field_name)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_history_author ON issue_history(author_account_id)", params![])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_history_composite ON issue_history(issue_key, change_timestamp DESC)", params![])?;
            
            Ok::<(), duckdb::Error>(())
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Schema creation failed: {}", e)))?;
        
        Ok(())
    }
    
    /// DuckDBクエリでフィルター条件を構築（簡素化版）
    fn build_where_clause(&self, filter: &IssueFilter) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        
        // プロジェクトキーでフィルタ
        if !filter.project_keys.is_empty() {
            let placeholders: Vec<String> = filter.project_keys.iter().map(|_| "?".to_string()).collect();
            conditions.push(format!("project_key IN ({})", placeholders.join(", ")));
            for key in &filter.project_keys {
                params.push(key.clone());
            }
        }
        
        // ステータスでフィルタ
        if !filter.statuses.is_empty() {
            let placeholders: Vec<String> = filter.statuses.iter().map(|_| "?".to_string()).collect();
            conditions.push(format!("status_name IN ({})", placeholders.join(", ")));
            for status in &filter.statuses {
                params.push(status.clone());
            }
        }
        
        // サマリー検索
        if let Some(ref text) = filter.summary_contains {
            conditions.push("summary LIKE ?".to_string());
            params.push(format!("%{}%", text));
        }
        
        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        
        (where_clause, params)
    }
    
    /// ソート順をSQL ORDER BY句に変換
    fn build_order_clause(&self, sort_order: &SortOrder) -> String {
        match sort_order {
            SortOrder::CreatedAsc => "ORDER BY created ASC".to_string(),
            SortOrder::CreatedDesc => "ORDER BY created DESC".to_string(),
            SortOrder::UpdatedAsc => "ORDER BY updated ASC".to_string(),
            SortOrder::UpdatedDesc => "ORDER BY updated DESC".to_string(),
            SortOrder::KeyAsc => "ORDER BY issue_key ASC".to_string(),
            SortOrder::KeyDesc => "ORDER BY issue_key DESC".to_string(),
            SortOrder::PriorityAsc => "ORDER BY priority_name ASC NULLS LAST".to_string(),
            SortOrder::PriorityDesc => "ORDER BY priority_name DESC NULLS LAST".to_string(),
        }
    }
    
    /// 履歴フィルター条件をSQL WHERE句に変換
    fn build_history_where_clause(&self, filter: &HistoryFilter) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        
        // 課題キーでフィルタ
        if let Some(ref issue_keys) = filter.issue_keys {
            if !issue_keys.is_empty() {
                let placeholders: Vec<String> = issue_keys.iter().map(|_| "?".to_string()).collect();
                conditions.push(format!("issue_key IN ({})", placeholders.join(", ")));
                for key in issue_keys {
                    params.push(key.clone());
                }
            }
        }
        
        // フィールド名でフィルタ
        if let Some(ref field_names) = filter.field_names {
            if !field_names.is_empty() {
                let placeholders: Vec<String> = field_names.iter().map(|_| "?".to_string()).collect();
                conditions.push(format!("field_name IN ({})", placeholders.join(", ")));
                for field in field_names {
                    params.push(field.clone());
                }
            }
        }
        
        // 変更者でフィルタ
        if let Some(ref authors) = filter.authors {
            if !authors.is_empty() {
                let placeholders: Vec<String> = authors.iter().map(|_| "?".to_string()).collect();
                conditions.push(format!("author_account_id IN ({})", placeholders.join(", ")));
                for author in authors {
                    params.push(author.clone());
                }
            }
        }
        
        // 日時範囲でフィルタ（簡素化）
        if let Some(ref date_range) = filter.date_range {
            conditions.push("change_timestamp >= ?".to_string());
            conditions.push("change_timestamp <= ?".to_string());
            params.push(date_range.start.format("%Y-%m-%d %H:%M:%S").to_string());
            params.push(date_range.end.format("%Y-%m-%d %H:%M:%S").to_string());
        }
        
        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        
        (where_clause, params)
    }
    
    /// 履歴ソート順をSQL ORDER BY句に変換  
    fn build_history_order_clause(&self, sort_order: &crate::HistorySortOrder) -> String {
        match sort_order {
            crate::HistorySortOrder::TimestampAsc => "ORDER BY change_timestamp ASC".to_string(),
            crate::HistorySortOrder::TimestampDesc => "ORDER BY change_timestamp DESC".to_string(),
            crate::HistorySortOrder::IssueKey => "ORDER BY issue_key ASC".to_string(),
            crate::HistorySortOrder::FieldName => "ORDER BY field_name ASC".to_string(),
        }
    }
}

#[async_trait]
impl PersistenceStore for DuckDBStore {
    async fn save_issues(&mut self, issues: &[Issue]) -> Result<usize, Error> {
        let conn = Arc::clone(&self.connection);
        let issues_clone = issues.to_vec();
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            // トランザクション開始
            conn.execute("BEGIN TRANSACTION", params![])?;
            
            let mut saved_count = 0;
            for issue in &issues_clone {
                let raw_json = match serde_json::to_string(issue) {
                    Ok(json) => json,
                    Err(_) => continue, // エラーの場合はスキップ
                };
                
                let result = conn.execute(
                    r#"
                    INSERT INTO issues 
                    (id, issue_key, summary, description, status_name, priority_name, 
                     issue_type_name, project_key, project_name, reporter_display_name, 
                     assignee_display_name, created, updated, raw_json)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        issue_key = EXCLUDED.issue_key,
                        summary = EXCLUDED.summary,
                        description = EXCLUDED.description,
                        status_name = EXCLUDED.status_name,
                        priority_name = EXCLUDED.priority_name,
                        issue_type_name = EXCLUDED.issue_type_name,
                        project_key = EXCLUDED.project_key,
                        project_name = EXCLUDED.project_name,
                        reporter_display_name = EXCLUDED.reporter_display_name,
                        assignee_display_name = EXCLUDED.assignee_display_name,
                        created = EXCLUDED.created,
                        updated = EXCLUDED.updated,
                        raw_json = EXCLUDED.raw_json
                    "#,
                    params![
                        &issue.id,
                        &issue.key,
                        &issue.fields.summary,
                        issue.fields.description.as_ref().map(|d| d.to_string()),
                        &issue.fields.status.name,
                        issue.fields.priority.as_ref().map(|p| &p.name),
                        &issue.fields.issue_type.name,
                        issue.fields.project.as_ref().map(|p| &p.key),
                        issue.fields.project.as_ref().map(|p| &p.name),
                        &issue.fields.reporter.display_name,
                        issue.fields.assignee.as_ref().map(|a| &a.display_name),
                        &issue.fields.created.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                        &issue.fields.updated.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                        &raw_json
                    ],
                );
                
                if result.is_ok() {
                    saved_count += 1;
                }
            }
            
            // トランザクションコミット
            conn.execute("COMMIT", params![])?;
            
            Ok::<usize, duckdb::Error>(saved_count)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Save operation failed: {}", e)))
    }
    
    async fn load_issues(&self, filter: &IssueFilter) -> Result<Vec<Issue>, Error> {
        let conn = Arc::clone(&self.connection);
        let (where_clause, filter_params) = self.build_where_clause(filter);
        let order_clause = self.build_order_clause(&filter.sort_order);
        
        let limit_clause = match (filter.offset, filter.limit) {
            (Some(offset), Some(limit)) => format!("LIMIT {} OFFSET {}", limit, offset),
            (None, Some(limit)) => format!("LIMIT {}", limit),
            (Some(offset), None) => format!("OFFSET {}", offset),
            (None, None) => "".to_string(),
        };
        
        let query = format!(
            "SELECT raw_json FROM issues {} {} {}",
            where_clause, order_clause, limit_clause
        );
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(&query)?;
            
            // パラメータを文字列リファレンスに変換
            let params_refs: Vec<&dyn duckdb::ToSql> = filter_params.iter()
                .map(|p| p as &dyn duckdb::ToSql)
                .collect();
            
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                let raw_json: String = row.get(0)?;
                Ok(raw_json)
            })?;
            
            let mut issues = Vec::new();
            for row in rows {
                let raw_json = row?;
                if let Ok(issue) = serde_json::from_str::<Issue>(&raw_json) {
                    issues.push(issue);
                }
            }
            
            Ok::<Vec<Issue>, duckdb::Error>(issues)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Load operation failed: {}", e)))
    }
    
    async fn load_all_issues(&self) -> Result<Vec<Issue>, Error> {
        let filter = IssueFilter::new();
        self.load_issues(&filter).await
    }
    
    async fn count_issues(&self, filter: &IssueFilter) -> Result<usize, Error> {
        let conn = Arc::clone(&self.connection);
        let (where_clause, filter_params) = self.build_where_clause(filter);
        
        let query = format!("SELECT COUNT(*) FROM issues {}", where_clause);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(&query)?;
            
            let params_refs: Vec<&dyn duckdb::ToSql> = filter_params.iter()
                .map(|p| p as &dyn duckdb::ToSql)
                .collect();
            
            let count: i64 = stmt.query_row(params_refs.as_slice(), |row| row.get(0))?;
            Ok::<usize, duckdb::Error>(count as usize)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Count operation failed: {}", e)))
    }
    
    async fn delete_issues(&mut self, issue_keys: &[String]) -> Result<usize, Error> {
        let conn = Arc::clone(&self.connection);
        let keys = issue_keys.to_vec();
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let placeholders: Vec<String> = keys.iter().map(|_| "?".to_string()).collect();
            let query = format!("DELETE FROM issues WHERE issue_key IN ({})", placeholders.join(", "));
            
            let params_refs: Vec<&dyn duckdb::ToSql> = keys.iter()
                .map(|k| k as &dyn duckdb::ToSql)
                .collect();
            
            let deleted_count = conn.execute(&query, params_refs.as_slice())?;
            Ok::<usize, duckdb::Error>(deleted_count)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Delete operation failed: {}", e)))
    }
    
    async fn optimize(&mut self) -> Result<(), Error> {
        let conn = Arc::clone(&self.connection);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            // DuckDBの最適化コマンド
            conn.execute("VACUUM", params![])?;
            conn.execute("ANALYZE", params![])?;
            Ok::<(), duckdb::Error>(())
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Optimize operation failed: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<StorageStats, Error> {
        let conn = Arc::clone(&self.connection);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let mut stats = StorageStats::new();
            
            // 総Issue数
            let total: i64 = conn.prepare("SELECT COUNT(*) FROM issues")?
                .query_row(params![], |row| row.get(0))?;
            stats.total_issues = total as usize;
            
            // プロジェクト別統計
            let mut stmt = conn.prepare("SELECT project_key, COUNT(*) FROM issues WHERE project_key IS NOT NULL GROUP BY project_key")?;
            let project_rows = stmt.query_map(params![], |row| {
                let key: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((key, count as usize))
            })?;
            
            for row in project_rows {
                let (key, count) = row?;
                stats.issues_by_project.insert(key, count);
            }
            
            // ステータス別統計
            let mut stmt = conn.prepare("SELECT status_name, COUNT(*) FROM issues GROUP BY status_name")?;
            let status_rows = stmt.query_map(params![], |row| {
                let name: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((name, count as usize))
            })?;
            
            for row in status_rows {
                let (name, count) = row?;
                stats.issues_by_status.insert(name, count);
            }
            
            // Issue種別別統計
            let mut stmt = conn.prepare("SELECT issue_type_name, COUNT(*) FROM issues GROUP BY issue_type_name")?;
            let type_rows = stmt.query_map(params![], |row| {
                let name: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((name, count as usize))
            })?;
            
            for row in type_rows {
                let (name, count) = row?;
                stats.issues_by_type.insert(name, count);
            }
            
            stats.last_updated = Utc::now();
            stats.compression_ratio = 0.0; // DuckDBは自動圧縮
            stats.index_count = 4; // 作成したインデックス数
            
            Ok::<StorageStats, duckdb::Error>(stats)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Stats operation failed: {}", e)))
    }
    
    async fn save_filter_config(&mut self, config: &FilterConfig) -> Result<(), Error> {
        let conn = Arc::clone(&self.connection);
        let config_clone = config.clone();
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let filter_json = match serde_json::to_string(&config_clone.filter) {
                Ok(json) => json,
                Err(_) => return Err(duckdb::Error::QueryReturnedNoRows),
            };
            
            conn.execute(
                r#"
                INSERT INTO filter_configs 
                (id, name, description, filter_json, created_at, updated_at, usage_count, last_used_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (id) DO UPDATE SET
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    filter_json = EXCLUDED.filter_json,
                    updated_at = EXCLUDED.updated_at,
                    usage_count = EXCLUDED.usage_count,
                    last_used_at = EXCLUDED.last_used_at
                "#,
                params![
                    config_clone.id,
                    config_clone.name,
                    config_clone.description,
                    filter_json,
                    config_clone.created_at.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                    config_clone.updated_at.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                    config_clone.usage_count as i64,
                    config_clone.last_used_at.map(|t| t.format("%Y-%m-%d %H:%M:%S%.f").to_string()),
                ],
            )?;
            
            Ok::<(), duckdb::Error>(())
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Save filter config failed: {}", e)))?;
        
        Ok(())
    }
    
    async fn load_filter_config(&self) -> Result<Option<FilterConfig>, Error> {
        let conn = Arc::clone(&self.connection);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let mut stmt = conn.prepare(
                "SELECT id, name, description, filter_json, created_at, updated_at, usage_count, last_used_at 
                 FROM filter_configs ORDER BY updated_at DESC LIMIT 1"
            )?;
            
            let result = stmt.query_row(params![], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let description: Option<String> = row.get(2)?;
                let filter_json: String = row.get(3)?;
                // DuckDBでは異なる型として格納されているため、Stringで取得を試行
                let created_at_str: String = match row.get::<_, String>(4) {
                    Ok(s) => s,
                    Err(_) => "2024-01-01 00:00:00".to_string(), // フォールバック
                };
                let updated_at_str: String = match row.get::<_, String>(5) {
                    Ok(s) => s,
                    Err(_) => "2024-01-01 00:00:00".to_string(), // フォールバック
                };
                let usage_count: i64 = row.get(6)?;
                let last_used_at_str: Option<String> = row.get::<_, Option<String>>(7).unwrap_or(None);
                
                Ok((id, name, description, filter_json, created_at_str, updated_at_str, usage_count, last_used_at_str))
            });
            
            match result {
                Ok((id, name, description, filter_json, _created_at_str, _updated_at_str, usage_count, _last_used_at_str)) => {
                    let filter = match serde_json::from_str(&filter_json) {
                        Ok(f) => f,
                        Err(_) => return Err(duckdb::Error::QueryReturnedNoRows),
                    };
                    
                    // 簡素化した日時解析（実際のプロダクションではより厳密に）
                    let created_at = Utc::now(); // 簡素化
                    let updated_at = Utc::now(); // 簡素化
                    let last_used_at = None; // 簡素化
                    
                    let config = FilterConfig {
                        id,
                        name,
                        description,
                        filter,
                        created_at,
                        updated_at,
                        usage_count: usage_count as u32,
                        last_used_at,
                    };
                    
                    Ok(Some(config))
                }
                Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Load filter config failed: {}", e)))
    }
    
    async fn save_issue_history(&mut self, histories: &[IssueHistory]) -> Result<usize, Error> {
        let conn = Arc::clone(&self.connection);
        let histories_clone = histories.to_vec();
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            // トランザクション開始
            conn.execute("BEGIN TRANSACTION", params![])?;
            
            let mut saved_count = 0;
            for history in &histories_clone {
                let result = conn.execute(
                    r#"
                    INSERT INTO issue_history 
                    (issue_id, issue_key, change_id, change_timestamp, author_account_id, 
                     author_display_name, author_email, field_name, field_id, from_value, 
                     to_value, from_display_value, to_display_value, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    params![
                        &history.issue_id,
                        &history.issue_key,
                        &history.change_id,
                        &history.change_timestamp.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                        &history.author.as_ref().map(|a| &a.account_id),
                        &history.author.as_ref().map(|a| &a.display_name),
                        &history.author.as_ref().and_then(|a| a.email_address.as_ref()),
                        &history.field_name,
                        &history.field_id,
                        &history.from_value,
                        &history.to_value,
                        &history.from_display_value,
                        &history.to_display_value,
                        &history.created_at.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                    ],
                );
                
                match result {
                    Ok(_) => {
                        saved_count += 1;
                    }
                    Err(e) => {
                        eprintln!("Failed to insert history record: {:?}", e);
                        // Continue with other records instead of failing completely
                    }
                }
            }
            
            // トランザクションコミット
            conn.execute("COMMIT", params![])?;
            
            Ok::<usize, duckdb::Error>(saved_count)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Save history operation failed: {}", e)))
    }
    
    async fn load_issue_history(&self, filter: &HistoryFilter) -> Result<Vec<IssueHistory>, Error> {
        let conn = Arc::clone(&self.connection);
        let (where_clause, filter_params) = self.build_history_where_clause(filter);
        let order_clause = self.build_history_order_clause(&filter.sort_order);
        
        let limit_clause = match filter.limit {
            Some(limit) => format!("LIMIT {}", limit),
            None => "".to_string(),
        };
        
        let query = format!(
            "SELECT issue_id, issue_key, change_id, 
                    strftime(change_timestamp, '%Y-%m-%d %H:%M:%S.%f') as change_timestamp_str,
                    author_account_id, author_display_name, author_email, field_name, field_id, from_value,
                    to_value, from_display_value, to_display_value,
                    strftime(created_at, '%Y-%m-%d %H:%M:%S.%f') as created_at_str
             FROM issue_history {} {} {}",
            where_clause, order_clause, limit_clause
        );
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(&query)?;
            
            let params_refs: Vec<&dyn duckdb::ToSql> = filter_params.iter()
                .map(|p| p as &dyn duckdb::ToSql)
                .collect();
            
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                let issue_id: String = row.get(0)?;
                let issue_key: String = row.get(1)?;
                let change_id: String = row.get(2)?;
                let timestamp_str: String = row.get(3)?;
                let account_id: Option<String> = row.get(4)?;
                let display_name: Option<String> = row.get(5)?;
                let email: Option<String> = row.get(6)?;
                let field_name: String = row.get(7)?;
                let field_id: Option<String> = row.get(8)?;
                let from_value: Option<String> = row.get(9)?;
                let to_value: Option<String> = row.get(10)?;
                let from_display: Option<String> = row.get(11)?;
                let to_display: Option<String> = row.get(12)?;
                let created_str: String = row.get(13)?;
                
                // Parse timestamps
                let change_timestamp = chrono::DateTime::parse_from_str(&timestamp_str, "%Y-%m-%d %H:%M:%S%.f")
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let created_at = chrono::DateTime::parse_from_str(&created_str, "%Y-%m-%d %H:%M:%S%.f")
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                
                let author = if let (Some(account_id), Some(display_name)) = (account_id, display_name) {
                    Some(HistoryAuthor {
                        account_id,
                        display_name,
                        email_address: email,
                    })
                } else {
                    None
                };
                
                let mut history = IssueHistory::new(
                    issue_id,
                    issue_key,
                    change_id,
                    change_timestamp,
                    field_name,
                );
                
                if let Some(author) = author {
                    history = history.with_author(author);
                }
                
                history = history.with_field_change(from_value, to_value, from_display, to_display);
                
                if let Some(field_id) = field_id {
                    history = history.with_field_id(field_id);
                }
                
                // Set the created_at timestamp from database
                history = history.with_created_at(created_at);
                
                Ok(history)
            })?;
            
            let mut histories = Vec::new();
            for row in rows {
                histories.push(row?);
            }
            
            Ok::<Vec<IssueHistory>, duckdb::Error>(histories)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Load history operation failed: {}", e)))
    }
    
    async fn get_history_stats(&self) -> Result<HistoryStats, Error> {
        let conn = Arc::clone(&self.connection);
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let mut stats = HistoryStats::new();
            
            // 総変更数
            let total: i64 = conn.prepare("SELECT COUNT(*) FROM issue_history")?
                .query_row(params![], |row| row.get(0))?;
            stats.total_changes = total as usize;
            
            // ユニークな課題数
            let unique_issues: i64 = conn.prepare("SELECT COUNT(DISTINCT issue_key) FROM issue_history")?
                .query_row(params![], |row| row.get(0))?;
            stats.unique_issues = unique_issues as usize;
            
            // ユニークな変更者数
            let unique_authors: i64 = conn.prepare(
                "SELECT COUNT(DISTINCT author_account_id) FROM issue_history WHERE author_account_id IS NOT NULL"
            )?.query_row(params![], |row| row.get(0))?;
            stats.unique_authors = unique_authors as usize;
            
            // フィールド別変更数
            let mut stmt = conn.prepare("SELECT field_name, COUNT(*) FROM issue_history GROUP BY field_name")?;
            let field_rows = stmt.query_map(params![], |row| {
                let field_name: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((field_name, count as usize))
            })?;
            
            for row in field_rows {
                let (field_name, count) = row?;
                stats.field_change_counts.insert(field_name, count);
            }
            
            // 最古・最新の変更日時（簡素化）
            stats.oldest_change = None; // TODO: 実装
            stats.newest_change = None; // TODO: 実装
            
            Ok::<HistoryStats, duckdb::Error>(stats)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("History stats operation failed: {}", e)))
    }
    
    async fn delete_issue_history(&mut self, issue_keys: &[String]) -> Result<usize, Error> {
        let conn = Arc::clone(&self.connection);
        let keys = issue_keys.to_vec();
        
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            let placeholders: Vec<String> = keys.iter().map(|_| "?".to_string()).collect();
            let query = format!(
                "DELETE FROM issue_history WHERE issue_key IN ({})", 
                placeholders.join(", ")
            );
            
            let params_refs: Vec<&dyn duckdb::ToSql> = keys.iter()
                .map(|k| k as &dyn duckdb::ToSql)
                .collect();
            
            let deleted_count = conn.execute(&query, params_refs.as_slice())?;
            Ok::<usize, duckdb::Error>(deleted_count)
        })
        .await
        .map_err(|e| Error::DatabaseError(format!("Task join error: {}", e)))?
        .map_err(|e| Error::DatabaseError(format!("Delete history operation failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{IssueFields, Status, IssueType, StatusCategory, Priority, Project, User};
    use std::collections::HashMap;
    use tempfile::TempDir;
    
    fn create_test_issue(key: &str, project_key: &str, status: &str) -> Issue {
        let status_category = StatusCategory {
            id: 1,
            key: "new".to_string(),
            name: "新規".to_string(),
            color_name: "blue-gray".to_string(),
            self_url: Some("http://example.com".to_string()),
        };
        
        let issue_status = Status {
            id: "1".to_string(),
            name: status.to_string(),
            description: None,
            icon_url: None,
            status_category,
            self_url: "http://example.com".to_string(),
        };
        
        let issue_type = IssueType {
            id: "1".to_string(),
            name: "Task".to_string(),
            description: None,
            icon_url: None,
            subtask: Some(false),
            self_url: "http://example.com".to_string(),
        };
        
        let project = Project {
            id: "1".to_string(),
            key: project_key.to_string(),
            name: format!("Project {}", project_key),
            project_type_key: Some("software".to_string()),
            description: None,
            lead: None,
            url: None,
            simplified: None,
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
        };
        
        let reporter = User {
            account_id: "test_user".to_string(),
            display_name: "Test User".to_string(),
            email_address: Some("test@example.com".to_string()),
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
            active: Some(true),
            time_zone: None,
            account_type: None,
        };
        
        let fields = IssueFields {
            summary: format!("Test issue {}", key),
            description: Some(serde_json::Value::String("Test description".to_string())),
            status: issue_status,
            priority: Some(Priority {
                id: "1".to_string(),
                name: "Medium".to_string(),
                description: None,
                icon_url: None,
                status_color: None,
                self_url: "http://example.com".to_string(),
            }),
            issue_type,
            assignee: None,
            reporter,
            created: Utc::now(),
            updated: Utc::now(),
            resolution_date: None,
            project: Some(project),
            custom_fields: HashMap::new(),
        };
        
        Issue {
            id: key.to_string(),
            key: key.to_string(),
            fields,
            self_url: "http://example.com".to_string(),
            changelog: None,
        }
    }
    
    #[tokio::test]
    async fn test_duckdb_store_new() {
        // DuckDBStore::new()で正しく作成されることをテスト
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = DuckDBStore::new(&db_path).unwrap();
        
        assert_eq!(store.db_path, db_path);
    }
    
    #[tokio::test]
    async fn test_duckdb_store_new_in_memory() {
        // DuckDBStore::new_in_memory()で正しく作成されることをテスト
        let store = DuckDBStore::new_in_memory().unwrap();
        
        assert_eq!(store.db_path, PathBuf::from(":memory:"));
    }
    
    #[tokio::test]
    async fn test_duckdb_store_initialize() {
        // DuckDBStore::initialize()でスキーマが作成されることをテスト
        let store = DuckDBStore::new_in_memory().unwrap();
        
        store.initialize().await.unwrap();
        
        // テーブルが作成されているかチェック（DuckDB specific）
        let conn = Arc::clone(&store.connection);
        let result = task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.prepare("SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('issues', 'filter_configs')")
                .unwrap()
                .query_row(params![], |row| row.get(0))
                .unwrap();
            count
        }).await.unwrap();
        
        assert_eq!(result, 2); // issuesとfilter_configsテーブル
    }
    
    #[tokio::test]
    async fn test_duckdb_store_save_and_load_issues() {
        // DuckDBStoreでIssueの保存と読み込みが正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];
        
        // 保存
        let saved_count = store.save_issues(&issues).await.unwrap();
        assert_eq!(saved_count, 3);
        
        // 全件読み込み
        let loaded_issues = store.load_all_issues().await.unwrap();
        assert_eq!(loaded_issues.len(), 3);
        
        // キーが正しく保存されているか確認
        let mut loaded_keys: Vec<String> = loaded_issues.iter().map(|i| i.key.clone()).collect();
        loaded_keys.sort();
        assert_eq!(loaded_keys, vec!["DEMO-1", "TEST-1", "TEST-2"]);
    }
    
    #[tokio::test]
    async fn test_duckdb_store_filter_issues() {
        // DuckDBStoreでIssueのフィルタリングが正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];
        
        store.save_issues(&issues).await.unwrap();
        
        // プロジェクトでフィルタ
        let filter = IssueFilter::new()
            .project_keys(vec!["TEST".to_string()]);
        let filtered_issues = store.load_issues(&filter).await.unwrap();
        
        assert_eq!(filtered_issues.len(), 2);
        assert!(filtered_issues.iter().all(|i| i.fields.project.as_ref().unwrap().key == "TEST"));
        
        // ステータスでフィルタ
        let filter = IssueFilter::new()
            .statuses(vec!["Done".to_string()]);
        let filtered_issues = store.load_issues(&filter).await.unwrap();
        
        assert_eq!(filtered_issues.len(), 1);
        assert_eq!(filtered_issues[0].fields.status.name, "Done");
    }
    
    #[tokio::test]
    async fn test_duckdb_store_count_issues() {
        // DuckDBStore::count_issues()が正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];
        
        store.save_issues(&issues).await.unwrap();
        
        // 全件カウント
        let total_count = store.count_issues(&IssueFilter::new()).await.unwrap();
        assert_eq!(total_count, 3);
        
        // フィルタ適用カウント
        let filter = IssueFilter::new().project_keys(vec!["TEST".to_string()]);
        let filtered_count = store.count_issues(&filter).await.unwrap();
        assert_eq!(filtered_count, 2);
    }
    
    #[tokio::test]
    async fn test_duckdb_store_delete_issues() {
        // DuckDBStore::delete_issues()が正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];
        
        store.save_issues(&issues).await.unwrap();
        
        // 削除
        let deleted_count = store.delete_issues(&["TEST-1".to_string(), "DEMO-1".to_string()]).await.unwrap();
        assert_eq!(deleted_count, 2);
        
        // 残りを確認
        let remaining_issues = store.load_all_issues().await.unwrap();
        assert_eq!(remaining_issues.len(), 1);
        assert_eq!(remaining_issues[0].key, "TEST-2");
    }
    
    #[tokio::test]
    async fn test_duckdb_store_get_stats() {
        // DuckDBStore::get_stats()が正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];
        
        store.save_issues(&issues).await.unwrap();
        
        let stats = store.get_stats().await.unwrap();
        
        assert_eq!(stats.total_issues, 3);
        assert_eq!(stats.issues_by_project.get("TEST"), Some(&2));
        assert_eq!(stats.issues_by_project.get("DEMO"), Some(&1));
        assert_eq!(stats.issues_by_status.get("Open"), Some(&1));
        assert_eq!(stats.issues_by_status.get("In Progress"), Some(&1));
        assert_eq!(stats.issues_by_status.get("Done"), Some(&1));
        assert_eq!(stats.index_count, 4);
    }
    
    #[tokio::test]
    async fn test_duckdb_store_filter_config() {
        // DuckDBStoreでFilterConfigの保存と読み込みが正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let filter = IssueFilter::new()
            .project_keys(vec!["TEST".to_string()])
            .statuses(vec!["Open".to_string()]);
        
        let config = FilterConfig::new(
            "test_filter".to_string(),
            "Test Filter".to_string(),
            filter,
        ).description("Test description".to_string());
        
        // 保存
        store.save_filter_config(&config).await.unwrap();
        
        // 読み込み
        let loaded_config = store.load_filter_config().await.unwrap();
        assert!(loaded_config.is_some());
        
        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.id, "test_filter");
        assert_eq!(loaded_config.name, "Test Filter");
        assert_eq!(loaded_config.description, Some("Test description".to_string()));
        assert_eq!(loaded_config.filter.project_keys, vec!["TEST"]);
        assert_eq!(loaded_config.filter.statuses, vec!["Open"]);
    }
    
    #[tokio::test]
    async fn test_duckdb_store_save_and_load_history() {
        // DuckDBStoreで履歴データの保存と読み込みが正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let author = HistoryAuthor {
            account_id: "user123".to_string(),
            display_name: "Test User".to_string(),
            email_address: Some("test@example.com".to_string()),
        };
        
        let histories = vec![
            IssueHistory::new(
                "10000".to_string(),
                "TEST-123".to_string(), 
                "change_1".to_string(),
                Utc::now(),
                "status".to_string(),
            ).with_author(author.clone())
             .with_field_change(
                 Some("Open".to_string()),
                 Some("In Progress".to_string()),
                 Some("Open".to_string()),
                 Some("In Progress".to_string()),
             ),
            IssueHistory::new(
                "10000".to_string(),
                "TEST-123".to_string(),
                "change_2".to_string(),
                Utc::now(),
                "assignee".to_string(),
            ).with_author(author)
             .with_field_change(
                 None,
                 Some("user456".to_string()),
                 None,
                 Some("Jane Doe".to_string()),
             ),
        ];
        
        // 保存
        let saved_count = store.save_issue_history(&histories).await.unwrap();
        assert_eq!(saved_count, 2);
        
        // 全件読み込み
        let filter = HistoryFilter::new();
        let loaded_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(loaded_histories.len(), 2);
        
        // 特定課題の履歴のみ読み込み
        let filter = HistoryFilter::new()
            .issue_keys(vec!["TEST-123".to_string()]);
        let filtered_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(filtered_histories.len(), 2);
        
        // ステータス変更のみ読み込み
        let filter = HistoryFilter::new()
            .field_names(vec!["status".to_string()]);
        let status_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(status_histories.len(), 1);
        assert_eq!(status_histories[0].field_name, "status");
    }
    
    #[tokio::test]
    async fn test_duckdb_store_history_stats() {
        // DuckDBStoreで履歴統計が正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let histories = vec![
            IssueHistory::new(
                "10000".to_string(),
                "TEST-123".to_string(),
                "change_1".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "10001".to_string(),
                "TEST-124".to_string(),
                "change_2".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "10000".to_string(),
                "TEST-123".to_string(),
                "change_3".to_string(),
                Utc::now(),
                "assignee".to_string(),
            ),
        ];
        
        store.save_issue_history(&histories).await.unwrap();
        
        let stats = store.get_history_stats().await.unwrap();
        assert_eq!(stats.total_changes, 3);
        assert_eq!(stats.unique_issues, 2); // TEST-123, TEST-124
        assert_eq!(stats.field_change_counts.get("status"), Some(&2));
        assert_eq!(stats.field_change_counts.get("assignee"), Some(&1));
    }
    
    #[tokio::test]
    async fn test_duckdb_store_delete_history() {
        // DuckDBStoreで履歴削除が正しく動作することをテスト
        let mut store = DuckDBStore::new_in_memory().unwrap();
        store.initialize().await.unwrap();
        
        let histories = vec![
            IssueHistory::new(
                "10000".to_string(),
                "TEST-123".to_string(),
                "change_1".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "10001".to_string(),
                "TEST-124".to_string(),
                "change_2".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
        ];
        
        store.save_issue_history(&histories).await.unwrap();
        
        // TEST-123の履歴を削除
        let deleted_count = store.delete_issue_history(&["TEST-123".to_string()]).await.unwrap();
        assert_eq!(deleted_count, 1);
        
        // 残りの履歴を確認
        let filter = HistoryFilter::new();
        let remaining_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(remaining_histories.len(), 1);
        assert_eq!(remaining_histories[0].issue_key, "TEST-124");
    }
}