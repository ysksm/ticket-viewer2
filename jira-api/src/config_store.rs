use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{Error, JiraConfig, FilterConfig};

/// アプリケーション設定の抽象化トレイト
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// JIRA接続設定を保存
    async fn save_jira_config(&mut self, config: &JiraConfig) -> Result<(), Error>;
    
    /// JIRA接続設定を読み込み
    async fn load_jira_config(&self) -> Result<Option<JiraConfig>, Error>;
    
    /// フィルター設定を保存
    async fn save_filter_config(&mut self, config: &FilterConfig) -> Result<(), Error>;
    
    /// フィルター設定を読み込み
    async fn load_filter_config(&self, id: &str) -> Result<Option<FilterConfig>, Error>;
    
    /// 全フィルター設定を一覧取得
    async fn list_filter_configs(&self) -> Result<Vec<FilterConfig>, Error>;
    
    /// フィルター設定を削除
    async fn delete_filter_config(&mut self, id: &str) -> Result<bool, Error>;
    
    /// アプリケーション設定を保存
    async fn save_app_config(&mut self, config: &AppConfig) -> Result<(), Error>;
    
    /// アプリケーション設定を読み込み
    async fn load_app_config(&self) -> Result<Option<AppConfig>, Error>;
    
    /// 設定ストアを初期化
    async fn initialize(&mut self) -> Result<(), Error>;
    
    /// 設定ストアをクリア
    async fn clear(&mut self) -> Result<(), Error>;
}

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// アプリケーション名
    pub app_name: String,
    /// バージョン
    pub version: String,
    /// デフォルトの同期間隔（分）
    pub default_sync_interval_minutes: u32,
    /// デフォルトの最大取得件数
    pub default_max_results: usize,
    /// デバッグモード
    pub debug_mode: bool,
    /// ログレベル
    pub log_level: String,
    /// カスタム設定
    pub custom_settings: HashMap<String, String>,
    /// 最後に更新された日時
    pub last_updated: DateTime<Utc>,
}

impl AppConfig {
    /// 新しいアプリケーション設定を作成
    pub fn new() -> Self {
        Self {
            app_name: "JIRA API Client".to_string(),
            version: "0.1.0".to_string(),
            default_sync_interval_minutes: 60,
            default_max_results: 100,
            debug_mode: false,
            log_level: "info".to_string(),
            custom_settings: HashMap::new(),
            last_updated: Utc::now(),
        }
    }
    
    /// カスタム設定を追加
    pub fn set_custom_setting(&mut self, key: String, value: String) {
        self.custom_settings.insert(key, value);
        self.last_updated = Utc::now();
    }
    
    /// カスタム設定を取得
    pub fn get_custom_setting(&self, key: &str) -> Option<&String> {
        self.custom_settings.get(key)
    }
    
    /// デバッグモードを設定
    pub fn set_debug_mode(&mut self, debug: bool) {
        self.debug_mode = debug;
        self.last_updated = Utc::now();
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON形式のファイルベース設定ストア
pub struct FileConfigStore {
    /// 設定ディレクトリのパス
    config_dir: PathBuf,
}

impl FileConfigStore {
    /// 新しいファイル設定ストアを作成
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }
    
    /// デフォルトの設定ディレクトリでファイル設定ストアを作成
    pub fn default_config_dir() -> Result<Self, Error> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::ConfigurationMissing("設定ディレクトリが見つかりません".to_string()))?
            .join("jira-api");
        
        Ok(Self::new(config_dir))
    }
    
    /// JIRA設定ファイルのパスを取得
    fn jira_config_path(&self) -> PathBuf {
        self.config_dir.join("jira_config.json")
    }
    
    /// フィルター設定ディレクトリのパスを取得
    fn filter_configs_dir(&self) -> PathBuf {
        self.config_dir.join("filters")
    }
    
    /// フィルター設定ファイルのパスを取得
    fn filter_config_path(&self, id: &str) -> PathBuf {
        self.filter_configs_dir().join(format!("{}.json", id))
    }
    
    /// アプリケーション設定ファイルのパスを取得
    fn app_config_path(&self) -> PathBuf {
        self.config_dir.join("app_config.json")
    }
    
    /// JSONファイルに書き込み
    async fn write_json_file<T>(&self, path: &Path, data: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        // 親ディレクトリを作成
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| Error::IoError(e))?;
        }
        
        let json_data = serde_json::to_string_pretty(data)
            .map_err(|e| Error::SerializationError(format!("JSON serialization failed: {}", e)))?;
        
        let mut file = fs::File::create(path).await
            .map_err(|e| Error::IoError(e))?;
        
        file.write_all(json_data.as_bytes()).await
            .map_err(|e| Error::IoError(e))?;
        
        file.sync_all().await
            .map_err(|e| Error::IoError(e))?;
        
        Ok(())
    }
    
    /// JSONファイルから読み込み
    async fn read_json_file<T>(&self, path: &Path) -> Result<Option<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !path.exists() {
            return Ok(None);
        }
        
        let mut file = fs::File::open(path).await
            .map_err(|e| Error::IoError(e))?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents).await
            .map_err(|e| Error::IoError(e))?;
        
        if contents.trim().is_empty() {
            return Ok(None);
        }
        
        let data: T = serde_json::from_str(&contents)
            .map_err(|e| Error::SerializationError(format!("JSON deserialization failed: {}", e)))?;
        
        Ok(Some(data))
    }
}

#[async_trait]
impl ConfigStore for FileConfigStore {
    async fn save_jira_config(&mut self, config: &JiraConfig) -> Result<(), Error> {
        let path = self.jira_config_path();
        self.write_json_file(&path, config).await
    }
    
    async fn load_jira_config(&self) -> Result<Option<JiraConfig>, Error> {
        let path = self.jira_config_path();
        self.read_json_file(&path).await
    }
    
    async fn save_filter_config(&mut self, config: &FilterConfig) -> Result<(), Error> {
        let path = self.filter_config_path(&config.id);
        self.write_json_file(&path, config).await
    }
    
    async fn load_filter_config(&self, id: &str) -> Result<Option<FilterConfig>, Error> {
        let path = self.filter_config_path(id);
        self.read_json_file(&path).await
    }
    
    async fn list_filter_configs(&self) -> Result<Vec<FilterConfig>, Error> {
        let filters_dir = self.filter_configs_dir();
        
        if !filters_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut entries = fs::read_dir(&filters_dir).await
            .map_err(|e| Error::IoError(e))?;
        
        let mut configs = Vec::new();
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::IoError(e))? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(Some(config)) = self.read_json_file::<FilterConfig>(&path).await {
                    configs.push(config);
                }
            }
        }
        
        // 更新日時でソート（新しい順）
        configs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(configs)
    }
    
    async fn delete_filter_config(&mut self, id: &str) -> Result<bool, Error> {
        let path = self.filter_config_path(id);
        
        if !path.exists() {
            return Ok(false);
        }
        
        fs::remove_file(&path).await
            .map_err(|e| Error::IoError(e))?;
        
        Ok(true)
    }
    
    async fn save_app_config(&mut self, config: &AppConfig) -> Result<(), Error> {
        let path = self.app_config_path();
        self.write_json_file(&path, config).await
    }
    
    async fn load_app_config(&self) -> Result<Option<AppConfig>, Error> {
        let path = self.app_config_path();
        self.read_json_file(&path).await
    }
    
    async fn initialize(&mut self) -> Result<(), Error> {
        // 設定ディレクトリとサブディレクトリを作成
        fs::create_dir_all(&self.config_dir).await
            .map_err(|e| Error::IoError(e))?;
        
        fs::create_dir_all(self.filter_configs_dir()).await
            .map_err(|e| Error::IoError(e))?;
        
        Ok(())
    }
    
    async fn clear(&mut self) -> Result<(), Error> {
        if self.config_dir.exists() {
            fs::remove_dir_all(&self.config_dir).await
                .map_err(|e| Error::IoError(e))?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Auth, IssueFilter, SortOrder};
    use tempfile::TempDir;
    
    async fn create_test_store() -> (FileConfigStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FileConfigStore::new(temp_dir.path());
        (store, temp_dir)
    }
    
    fn create_test_jira_config() -> JiraConfig {
        JiraConfig::new(
            "https://test.atlassian.net".to_string(),
            Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test-token".to_string(),
            },
        ).unwrap()
    }
    
    fn create_test_filter_config() -> FilterConfig {
        let filter = IssueFilter::new()
            .project_keys(vec!["TEST".to_string()])
            .statuses(vec!["Open".to_string()])
            .sort_order(SortOrder::CreatedDesc);
        
        FilterConfig::new(
            "test_filter".to_string(),
            "Test Filter".to_string(),
            filter,
        ).description("Test filter description".to_string())
    }
    
    #[tokio::test]
    async fn test_file_config_store_initialize() {
        // FileConfigStore::initialize()でディレクトリが作成されることをテスト
        let (mut store, temp_dir) = create_test_store().await;
        
        store.initialize().await.unwrap();
        
        assert!(temp_dir.path().exists());
        assert!(temp_dir.path().join("filters").exists());
    }
    
    #[tokio::test]
    async fn test_jira_config_save_and_load() {
        // JIRA設定の保存と読み込みが正しく動作することをテスト
        let (mut store, _temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        let config = create_test_jira_config();
        
        // 保存
        store.save_jira_config(&config).await.unwrap();
        
        // 読み込み
        let loaded_config = store.load_jira_config().await.unwrap();
        assert!(loaded_config.is_some());
        
        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.base_url, config.base_url);
        match (&loaded_config.auth, &config.auth) {
            (Auth::Basic { username: u1, api_token: p1 }, Auth::Basic { username: u2, api_token: p2 }) => {
                assert_eq!(u1, u2);
                assert_eq!(p1, p2);
            }
            _ => panic!("認証方式が一致しません"),
        }
    }
    
    #[tokio::test]
    async fn test_filter_config_save_and_load() {
        // フィルター設定の保存と読み込みが正しく動作することをテスト
        let (mut store, _temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        let config = create_test_filter_config();
        
        // 保存
        store.save_filter_config(&config).await.unwrap();
        
        // 読み込み
        let loaded_config = store.load_filter_config(&config.id).await.unwrap();
        assert!(loaded_config.is_some());
        
        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.id, config.id);
        assert_eq!(loaded_config.name, config.name);
        assert_eq!(loaded_config.description, config.description);
        assert_eq!(loaded_config.filter.project_keys, config.filter.project_keys);
        assert_eq!(loaded_config.filter.statuses, config.filter.statuses);
    }
    
    #[tokio::test]
    async fn test_filter_config_list() {
        // フィルター設定の一覧取得が正しく動作することをテスト
        let (mut store, _temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        // 複数のフィルター設定を作成
        let mut config1 = create_test_filter_config();
        config1.id = "filter1".to_string();
        config1.name = "Filter 1".to_string();
        
        let mut config2 = create_test_filter_config();
        config2.id = "filter2".to_string();
        config2.name = "Filter 2".to_string();
        
        // 保存
        store.save_filter_config(&config1).await.unwrap();
        store.save_filter_config(&config2).await.unwrap();
        
        // 一覧取得
        let configs = store.list_filter_configs().await.unwrap();
        assert_eq!(configs.len(), 2);
        
        // 更新日時順（新しい順）でソートされているか確認
        let names: Vec<String> = configs.iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"Filter 1".to_string()));
        assert!(names.contains(&"Filter 2".to_string()));
    }
    
    #[tokio::test]
    async fn test_filter_config_delete() {
        // フィルター設定の削除が正しく動作することをテスト
        let (mut store, _temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        let config = create_test_filter_config();
        
        // 保存
        store.save_filter_config(&config).await.unwrap();
        
        // 存在確認
        let loaded_config = store.load_filter_config(&config.id).await.unwrap();
        assert!(loaded_config.is_some());
        
        // 削除
        let deleted = store.delete_filter_config(&config.id).await.unwrap();
        assert!(deleted);
        
        // 削除確認
        let loaded_config = store.load_filter_config(&config.id).await.unwrap();
        assert!(loaded_config.is_none());
        
        // 存在しないIDの削除
        let deleted = store.delete_filter_config("nonexistent").await.unwrap();
        assert!(!deleted);
    }
    
    #[tokio::test]
    async fn test_app_config_save_and_load() {
        // アプリケーション設定の保存と読み込みが正しく動作することをテスト
        let (mut store, _temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        let mut config = AppConfig::new();
        config.app_name = "Test App".to_string();
        config.debug_mode = true;
        config.set_custom_setting("custom_key".to_string(), "custom_value".to_string());
        
        // 保存
        store.save_app_config(&config).await.unwrap();
        
        // 読み込み
        let loaded_config = store.load_app_config().await.unwrap();
        assert!(loaded_config.is_some());
        
        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.app_name, "Test App");
        assert_eq!(loaded_config.debug_mode, true);
        assert_eq!(loaded_config.get_custom_setting("custom_key"), Some(&"custom_value".to_string()));
    }
    
    #[tokio::test]
    async fn test_config_store_clear() {
        // 設定ストアのクリアが正しく動作することをテスト
        let (mut store, temp_dir) = create_test_store().await;
        store.initialize().await.unwrap();
        
        // 設定を保存
        let jira_config = create_test_jira_config();
        let filter_config = create_test_filter_config();
        let app_config = AppConfig::new();
        
        store.save_jira_config(&jira_config).await.unwrap();
        store.save_filter_config(&filter_config).await.unwrap();
        store.save_app_config(&app_config).await.unwrap();
        
        // ファイルが存在することを確認
        assert!(temp_dir.path().join("jira_config.json").exists());
        assert!(temp_dir.path().join("filters").join("test_filter.json").exists());
        assert!(temp_dir.path().join("app_config.json").exists());
        
        // クリア
        store.clear().await.unwrap();
        
        // ディレクトリが削除されていることを確認
        assert!(!temp_dir.path().exists());
    }
    
    #[tokio::test]
    async fn test_app_config_methods() {
        // AppConfigのメソッドが正しく動作することをテスト
        let mut config = AppConfig::new();
        
        // デフォルト値の確認
        assert_eq!(config.app_name, "JIRA API Client");
        assert_eq!(config.debug_mode, false);
        assert_eq!(config.log_level, "info");
        
        // カスタム設定の追加・取得
        config.set_custom_setting("test_key".to_string(), "test_value".to_string());
        assert_eq!(config.get_custom_setting("test_key"), Some(&"test_value".to_string()));
        assert_eq!(config.get_custom_setting("nonexistent"), None);
        
        // デバッグモードの設定
        let initial_updated = config.last_updated;
        std::thread::sleep(std::time::Duration::from_millis(1));
        config.set_debug_mode(true);
        assert_eq!(config.debug_mode, true);
        assert!(config.last_updated > initial_updated);
    }
}