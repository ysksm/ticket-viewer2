use async_trait::async_trait;
use chrono::{DateTime, Utc};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::fs::{File, create_dir_all};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{Error, FilterConfig, Issue, IssueFilter, PersistenceStore, SortOrder, StorageStats};

/// JSON形式のファイルストア（gzip圧縮対応）
pub struct JsonStore {
    /// データディレクトリのパス
    data_dir: PathBuf,
    /// gzip圧縮を使用するかどうか
    use_compression: bool,
    /// メタデータキャッシュ
    metadata_cache: Option<StorageStats>,
    /// 最後の更新時刻
    last_sync_time: Option<DateTime<Utc>>,
}

impl JsonStore {
    /// 新しいJSONストアを作成
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            use_compression: true,
            metadata_cache: None,
            last_sync_time: None,
        }
    }

    /// 圧縮設定を変更
    pub fn with_compression(mut self, use_compression: bool) -> Self {
        self.use_compression = use_compression;
        self
    }

    /// データディレクトリを初期化
    pub async fn initialize(&mut self) -> Result<(), Error> {
        create_dir_all(&self.data_dir)
            .await
            .map_err(|e| Error::IoError(e))?;

        // サブディレクトリの作成
        create_dir_all(self.data_dir.join("issues"))
            .await
            .map_err(|e| Error::IoError(e))?;
        create_dir_all(self.data_dir.join("filters"))
            .await
            .map_err(|e| Error::IoError(e))?;
        create_dir_all(self.data_dir.join("history"))
            .await
            .map_err(|e| Error::IoError(e))?;
        create_dir_all(self.data_dir.join("metadata"))
            .await
            .map_err(|e| Error::IoError(e))?;

        Ok(())
    }

    /// Issuesファイルのパスを取得
    fn get_issues_file_path(&self) -> PathBuf {
        let filename = if self.use_compression {
            "issues.json.gz"
        } else {
            "issues.json"
        };
        self.data_dir.join("issues").join(filename)
    }

    /// フィルター設定ファイルのパスを取得
    fn get_filter_config_file_path(&self) -> PathBuf {
        let filename = if self.use_compression {
            "filter_config.json.gz"
        } else {
            "filter_config.json"
        };
        self.data_dir.join("filters").join(filename)
    }

    /// 履歴ファイルのパスを取得
    fn get_history_file_path(&self) -> PathBuf {
        let filename = if self.use_compression {
            "history.json.gz"
        } else {
            "history.json"
        };
        self.data_dir.join("history").join(filename)
    }

    /// 履歴データにフィルターを適用
    fn apply_history_filter(
        &self,
        histories: &[crate::IssueHistory],
        filter: &crate::HistoryFilter,
    ) -> Vec<crate::IssueHistory> {
        let mut filtered: Vec<crate::IssueHistory> = histories
            .iter()
            .filter(|h| {
                // 課題キーフィルター
                if let Some(ref issue_keys) = filter.issue_keys {
                    if !issue_keys.is_empty() && !issue_keys.contains(&h.issue_key) {
                        return false;
                    }
                }

                // フィールド名フィルター
                if let Some(ref field_names) = filter.field_names {
                    if !field_names.is_empty() && !field_names.contains(&h.field_name) {
                        return false;
                    }
                }

                // 変更者フィルター
                if let Some(ref authors) = filter.authors {
                    if !authors.is_empty() {
                        if let Some(ref author) = h.author {
                            if !authors.contains(&author.account_id) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }

                // 日付範囲フィルター
                if let Some(ref date_range) = filter.date_range {
                    if !date_range.contains(&h.change_timestamp) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // ソート適用
        match filter.sort_order {
            crate::HistorySortOrder::TimestampAsc => {
                filtered.sort_by(|a, b| a.change_timestamp.cmp(&b.change_timestamp));
            }
            crate::HistorySortOrder::TimestampDesc => {
                filtered.sort_by(|a, b| b.change_timestamp.cmp(&a.change_timestamp));
            }
            crate::HistorySortOrder::IssueKey => {
                filtered.sort_by(|a, b| a.issue_key.cmp(&b.issue_key));
            }
            crate::HistorySortOrder::FieldName => {
                filtered.sort_by(|a, b| a.field_name.cmp(&b.field_name));
            }
        }

        // 件数制限適用
        if let Some(limit) = filter.limit {
            filtered.truncate(limit);
        }

        filtered
    }

    /// メタデータファイルのパスを取得
    fn get_metadata_file_path(&self) -> PathBuf {
        let filename = if self.use_compression {
            "metadata.json.gz"
        } else {
            "metadata.json"
        };
        self.data_dir.join("metadata").join(filename)
    }

    /// データをJSONファイルに書き込み（圧縮対応）
    async fn write_json_file<T>(&self, path: &Path, data: &T) -> Result<(), Error>
    where
        T: Serialize + ?Sized,
    {
        let json_data = serde_json::to_vec_pretty(data)
            .map_err(|e| Error::SerializationError(format!("JSON serialization failed: {}", e)))?;

        let final_data = if self.use_compression {
            // gzip圧縮
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&json_data)
                .map_err(|e| Error::IoError(e))?;
            encoder.finish().map_err(|e| Error::IoError(e))?
        } else {
            json_data
        };

        let mut file = File::create(path).await.map_err(|e| Error::IoError(e))?;
        file.write_all(&final_data)
            .await
            .map_err(|e| Error::IoError(e))?;
        file.sync_all().await.map_err(|e| Error::IoError(e))?;

        Ok(())
    }

    /// JSONファイルからデータを読み込み（圧縮対応）
    async fn read_json_file<T>(&self, path: &Path) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut file = File::open(path).await.map_err(|e| Error::IoError(e))?;

        let mut raw_data = Vec::new();
        file.read_to_end(&mut raw_data)
            .await
            .map_err(|e| Error::IoError(e))?;

        let json_data = if self.use_compression {
            // gzip解凍
            let mut decoder = GzDecoder::new(&raw_data[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| Error::IoError(e))?;
            decompressed
        } else {
            raw_data
        };

        serde_json::from_slice(&json_data)
            .map_err(|e| Error::SerializationError(format!("JSON deserialization failed: {}", e)))
    }

    /// Issueをフィルタリング
    fn filter_issues(&self, issues: &[Issue], filter: &IssueFilter) -> Vec<Issue> {
        let mut filtered: Vec<Issue> = issues
            .iter()
            .filter(|issue| filter.matches(issue))
            .cloned()
            .collect();

        // ソート
        match filter.sort_order {
            SortOrder::CreatedAsc => {
                filtered.sort_by(|a, b| a.fields.created.cmp(&b.fields.created))
            }
            SortOrder::CreatedDesc => {
                filtered.sort_by(|a, b| b.fields.created.cmp(&a.fields.created))
            }
            SortOrder::UpdatedAsc => {
                filtered.sort_by(|a, b| a.fields.updated.cmp(&b.fields.updated))
            }
            SortOrder::UpdatedDesc => {
                filtered.sort_by(|a, b| b.fields.updated.cmp(&a.fields.updated))
            }
            SortOrder::KeyAsc => filtered.sort_by(|a, b| a.key.cmp(&b.key)),
            SortOrder::KeyDesc => filtered.sort_by(|a, b| b.key.cmp(&a.key)),
            SortOrder::PriorityAsc => filtered.sort_by(|a, b| {
                let priority_a = a
                    .fields
                    .priority
                    .as_ref()
                    .map(|p| &p.name)
                    .map_or("None", |v| v);
                let priority_b = b
                    .fields
                    .priority
                    .as_ref()
                    .map(|p| &p.name)
                    .map_or("None", |v| v);
                priority_a.cmp(priority_b)
            }),
            SortOrder::PriorityDesc => filtered.sort_by(|a, b| {
                let priority_a = a
                    .fields
                    .priority
                    .as_ref()
                    .map(|p| &p.name)
                    .map_or("None", |v| v);
                let priority_b = b
                    .fields
                    .priority
                    .as_ref()
                    .map(|p| &p.name)
                    .map_or("None", |v| v);
                priority_b.cmp(priority_a)
            }),
        }

        // オフセットと制限の適用
        let start = filter.offset.unwrap_or(0);
        let end = if let Some(limit) = filter.limit {
            (start + limit).min(filtered.len())
        } else {
            filtered.len()
        };

        if start >= filtered.len() {
            Vec::new()
        } else {
            filtered[start..end].to_vec()
        }
    }

    /// ストレージ統計を計算
    fn calculate_stats(&self, issues: &[Issue]) -> StorageStats {
        let mut stats = StorageStats::new();
        stats.total_issues = issues.len();

        for issue in issues {
            // プロジェクト別統計
            if let Some(ref project) = issue.fields.project {
                *stats
                    .issues_by_project
                    .entry(project.key.clone())
                    .or_insert(0) += 1;
            }

            // ステータス別統計
            *stats
                .issues_by_status
                .entry(issue.fields.status.name.clone())
                .or_insert(0) += 1;

            // Issue種別別統計
            *stats
                .issues_by_type
                .entry(issue.fields.issue_type.name.clone())
                .or_insert(0) += 1;
        }

        stats.last_updated = Utc::now();
        stats.compression_ratio = if self.use_compression { 0.7 } else { 1.0 }; // 推定値

        stats
    }

    /// メタデータキャッシュを更新
    async fn update_metadata_cache(&mut self) -> Result<(), Error> {
        let issues_path = self.get_issues_file_path();
        if issues_path.exists() {
            let issues: Vec<Issue> = self.read_json_file(&issues_path).await.unwrap_or_default();
            self.metadata_cache = Some(self.calculate_stats(&issues));

            // メタデータをファイルにも保存
            let metadata_path = self.get_metadata_file_path();
            if let Some(ref stats) = self.metadata_cache {
                self.write_json_file(&metadata_path, stats).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl PersistenceStore for JsonStore {
    async fn save_issues(&mut self, issues: &[Issue]) -> Result<usize, Error> {
        self.initialize().await?;

        let issues_path = self.get_issues_file_path();
        self.write_json_file(&issues_path, issues).await?;

        // メタデータキャッシュを更新
        self.metadata_cache = Some(self.calculate_stats(issues));
        self.last_sync_time = Some(Utc::now());

        // メタデータファイルも更新
        let metadata_path = self.get_metadata_file_path();
        if let Some(ref stats) = self.metadata_cache {
            self.write_json_file(&metadata_path, stats).await?;
        }

        Ok(issues.len())
    }

    async fn load_issues(&self, filter: &IssueFilter) -> Result<Vec<Issue>, Error> {
        let issues_path = self.get_issues_file_path();

        if !issues_path.exists() {
            return Ok(Vec::new());
        }

        let all_issues: Vec<Issue> = self.read_json_file(&issues_path).await?;
        Ok(self.filter_issues(&all_issues, filter))
    }

    async fn load_all_issues(&self) -> Result<Vec<Issue>, Error> {
        let filter = IssueFilter::new();
        self.load_issues(&filter).await
    }

    async fn count_issues(&self, filter: &IssueFilter) -> Result<usize, Error> {
        let issues = self.load_issues(filter).await?;
        Ok(issues.len())
    }

    async fn delete_issues(&mut self, issue_keys: &[String]) -> Result<usize, Error> {
        let issues_path = self.get_issues_file_path();

        if !issues_path.exists() {
            return Ok(0);
        }

        let mut all_issues: Vec<Issue> = self.read_json_file(&issues_path).await?;
        let original_count = all_issues.len();

        // 指定されたキーのIssueを削除
        all_issues.retain(|issue| !issue_keys.contains(&issue.key));
        let deleted_count = original_count - all_issues.len();

        if deleted_count > 0 {
            // 更新されたデータを保存
            self.write_json_file(&issues_path, &all_issues).await?;

            // メタデータキャッシュを更新
            self.metadata_cache = Some(self.calculate_stats(&all_issues));
            self.last_sync_time = Some(Utc::now());
        }

        Ok(deleted_count)
    }

    async fn optimize(&mut self) -> Result<(), Error> {
        // JSONストアの場合、最適化は主にメタデータの再計算
        self.update_metadata_cache().await?;
        Ok(())
    }

    async fn get_stats(&self) -> Result<StorageStats, Error> {
        if let Some(ref cache) = self.metadata_cache {
            return Ok(cache.clone());
        }

        // キャッシュがない場合は計算
        let issues = self.load_all_issues().await?;
        Ok(self.calculate_stats(&issues))
    }

    async fn save_filter_config(&mut self, config: &FilterConfig) -> Result<(), Error> {
        self.initialize().await?;

        let config_path = self.get_filter_config_file_path();
        self.write_json_file(&config_path, config).await?;

        Ok(())
    }

    async fn load_filter_config(&self) -> Result<Option<FilterConfig>, Error> {
        let config_path = self.get_filter_config_file_path();

        if !config_path.exists() {
            return Ok(None);
        }

        let config: FilterConfig = self.read_json_file(&config_path).await?;
        Ok(Some(config))
    }

    async fn save_issue_history(
        &mut self,
        history: &[crate::IssueHistory],
    ) -> Result<usize, Error> {
        self.initialize().await?;

        let history_path = self.get_history_file_path();
        self.write_json_file(&history_path, history).await?;
        Ok(history.len())
    }

    async fn load_issue_history(
        &self,
        filter: &crate::HistoryFilter,
    ) -> Result<Vec<crate::IssueHistory>, Error> {
        let history_path = self.get_history_file_path();

        if !history_path.exists() {
            return Ok(Vec::new());
        }

        let all_history: Vec<crate::IssueHistory> = self.read_json_file(&history_path).await?;
        let filtered = self.apply_history_filter(&all_history, filter);
        Ok(filtered)
    }

    async fn get_history_stats(&self) -> Result<crate::HistoryStats, Error> {
        let history_path = self.get_history_file_path();

        if !history_path.exists() {
            return Ok(crate::HistoryStats::new());
        }

        let all_history: Vec<crate::IssueHistory> = self.read_json_file(&history_path).await?;
        let mut stats = crate::HistoryStats::new();
        stats.update(&all_history);
        Ok(stats)
    }

    async fn delete_issue_history(&mut self, issue_keys: &[String]) -> Result<usize, Error> {
        let history_path = self.get_history_file_path();

        if !history_path.exists() {
            return Ok(0);
        }

        let all_history: Vec<crate::IssueHistory> = self.read_json_file(&history_path).await?;
        let original_len = all_history.len();

        let filtered_history: Vec<crate::IssueHistory> = all_history
            .into_iter()
            .filter(|h| !issue_keys.contains(&h.issue_key))
            .collect();

        let deleted_count = original_len - filtered_history.len();
        self.write_json_file(&history_path, &filtered_history)
            .await?;
        Ok(deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HistoryFilter;
    use crate::models::{IssueFields, IssueType, Priority, Project, Status, StatusCategory, User};
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
    async fn test_json_store_new() {
        // JsonStore::new()で正しく作成されることをテスト
        let temp_dir = TempDir::new().unwrap();
        let store = JsonStore::new(temp_dir.path());

        assert_eq!(store.data_dir, temp_dir.path());
        assert_eq!(store.use_compression, true);
        assert!(store.metadata_cache.is_none());
    }

    #[tokio::test]
    async fn test_json_store_with_compression() {
        // JsonStore::with_compression()が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let store = JsonStore::new(temp_dir.path()).with_compression(false);

        assert_eq!(store.use_compression, false);
    }

    #[tokio::test]
    async fn test_json_store_initialize() {
        // JsonStore::initialize()でディレクトリが作成されることをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        store.initialize().await.unwrap();

        assert!(temp_dir.path().join("issues").exists());
        assert!(temp_dir.path().join("filters").exists());
        assert!(temp_dir.path().join("metadata").exists());
    }

    #[tokio::test]
    async fn test_json_store_save_and_load_issues() {
        // JsonStoreでIssueの保存と読み込みが正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

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

        // 作成日時降順でソートされるため、後から作成された順
        let mut issue_keys: Vec<String> = loaded_issues.iter().map(|i| i.key.clone()).collect();
        issue_keys.sort();
        assert_eq!(issue_keys, vec!["DEMO-1", "TEST-1", "TEST-2"]);
    }

    #[tokio::test]
    async fn test_json_store_filter_issues() {
        // JsonStoreでIssueのフィルタリングが正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];

        store.save_issues(&issues).await.unwrap();

        // プロジェクトでフィルタ
        let filter = IssueFilter::new().project_keys(vec!["TEST".to_string()]);
        let filtered_issues = store.load_issues(&filter).await.unwrap();

        assert_eq!(filtered_issues.len(), 2);
        assert!(
            filtered_issues
                .iter()
                .all(|i| i.fields.project.as_ref().unwrap().key == "TEST")
        );

        // ステータスでフィルタ
        let filter = IssueFilter::new().statuses(vec!["Done".to_string()]);
        let filtered_issues = store.load_issues(&filter).await.unwrap();

        assert_eq!(filtered_issues.len(), 1);
        assert_eq!(filtered_issues[0].fields.status.name, "Done");
    }

    #[tokio::test]
    async fn test_json_store_count_issues() {
        // JsonStore::count_issues()が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

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
    async fn test_json_store_delete_issues() {
        // JsonStore::delete_issues()が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        let issues = vec![
            create_test_issue("TEST-1", "TEST", "Open"),
            create_test_issue("TEST-2", "TEST", "In Progress"),
            create_test_issue("DEMO-1", "DEMO", "Done"),
        ];

        store.save_issues(&issues).await.unwrap();

        // 削除
        let deleted_count = store
            .delete_issues(&["TEST-1".to_string(), "DEMO-1".to_string()])
            .await
            .unwrap();
        assert_eq!(deleted_count, 2);

        // 残りを確認
        let remaining_issues = store.load_all_issues().await.unwrap();
        assert_eq!(remaining_issues.len(), 1);
        assert_eq!(remaining_issues[0].key, "TEST-2");
    }

    #[tokio::test]
    async fn test_json_store_get_stats() {
        // JsonStore::get_stats()が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

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
    }

    #[tokio::test]
    async fn test_json_store_filter_config() {
        // JsonStoreでFilterConfigの保存と読み込みが正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        let filter = IssueFilter::new()
            .project_keys(vec!["TEST".to_string()])
            .statuses(vec!["Open".to_string()]);

        let config =
            FilterConfig::new("test_filter".to_string(), "Test Filter".to_string(), filter)
                .description("Test description".to_string());

        // 保存
        store.save_filter_config(&config).await.unwrap();

        // 読み込み
        let loaded_config = store.load_filter_config().await.unwrap();
        assert!(loaded_config.is_some());

        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.id, "test_filter");
        assert_eq!(loaded_config.name, "Test Filter");
        assert_eq!(
            loaded_config.description,
            Some("Test description".to_string())
        );
        assert_eq!(loaded_config.filter.project_keys, vec!["TEST"]);
        assert_eq!(loaded_config.filter.statuses, vec!["Open"]);
    }

    #[tokio::test]
    async fn test_json_store_save_and_load_history() {
        // JsonStoreで履歴データの保存と読み込みが正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path()).with_compression(false);

        use crate::{HistoryAuthor, HistoryFilter, IssueHistory};
        use chrono::Utc;

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
            )
            .with_author(author.clone())
            .with_field_change(
                Some("Open".to_string()),
                Some("In Progress".to_string()),
                Some("Open".to_string()),
                Some("In Progress".to_string()),
            ),
            IssueHistory::new(
                "10001".to_string(),
                "TEST-124".to_string(),
                "change_2".to_string(),
                Utc::now(),
                "assignee".to_string(),
            )
            .with_author(author)
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
        let filter = HistoryFilter::new().issue_keys(vec!["TEST-123".to_string()]);
        let filtered_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(filtered_histories.len(), 1);
        assert_eq!(filtered_histories[0].issue_key, "TEST-123");

        // ステータス変更のみ読み込み
        let filter = HistoryFilter::new().field_names(vec!["status".to_string()]);
        let status_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(status_histories.len(), 1);
        assert_eq!(status_histories[0].field_name, "status");
    }

    #[tokio::test]
    async fn test_json_store_history_stats() {
        // JsonStoreで履歴統計が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        use crate::IssueHistory;
        use chrono::Utc;

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
    async fn test_json_store_delete_history() {
        // JsonStoreで履歴削除が正しく動作することをテスト
        let temp_dir = TempDir::new().unwrap();
        let mut store = JsonStore::new(temp_dir.path());

        use crate::IssueHistory;
        use chrono::Utc;

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
        let deleted_count = store
            .delete_issue_history(&["TEST-123".to_string()])
            .await
            .unwrap();
        assert_eq!(deleted_count, 1);

        // 残りの履歴を確認
        let filter = HistoryFilter::new();
        let remaining_histories = store.load_issue_history(&filter).await.unwrap();
        assert_eq!(remaining_histories.len(), 1);
        assert_eq!(remaining_histories[0].issue_key, "TEST-124");
    }
}
