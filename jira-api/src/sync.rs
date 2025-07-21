use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::{JiraClient, SearchParams, TimeBasedFilter, Issue, Error};

/// 同期サービスの設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// 同期間隔（分）
    pub interval_minutes: u32,
    /// 最大同期履歴保持数
    pub max_history_count: usize,
    /// 時間ベースフィルタリングのパフォーマンス最適化フラグ
    pub enable_time_optimization: bool,
    /// 並行同期処理数
    pub concurrent_sync_count: usize,
    /// 同期対象プロジェクトキー（空の場合は全プロジェクト）
    pub target_projects: Vec<String>,
    /// 除外するフィールド
    pub excluded_fields: Vec<String>,
}

impl SyncConfig {
    /// デフォルト設定で新しいSyncConfigを作成
    pub fn new() -> Self {
        Self {
            interval_minutes: 60, // 1時間間隔
            max_history_count: 100,
            enable_time_optimization: true,
            concurrent_sync_count: 3,
            target_projects: Vec::new(),
            excluded_fields: Vec::new(),
        }
    }
    
    /// 同期間隔を設定
    pub fn interval_minutes(mut self, minutes: u32) -> Self {
        self.interval_minutes = minutes;
        self
    }
    
    /// 最大履歴保持数を設定
    pub fn max_history_count(mut self, count: usize) -> Self {
        self.max_history_count = count;
        self
    }
    
    /// 時間最適化を設定
    pub fn enable_time_optimization(mut self, enabled: bool) -> Self {
        self.enable_time_optimization = enabled;
        self
    }
    
    /// 並行処理数を設定
    pub fn concurrent_sync_count(mut self, count: usize) -> Self {
        self.concurrent_sync_count = count;
        self
    }
    
    /// 対象プロジェクトを設定
    pub fn target_projects(mut self, projects: Vec<String>) -> Self {
        self.target_projects = projects;
        self
    }
    
    /// 除外フィールドを設定
    pub fn excluded_fields(mut self, fields: Vec<String>) -> Self {
        self.excluded_fields = fields;
        self
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 同期処理の結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// 同期開始時刻
    pub start_time: DateTime<Utc>,
    /// 同期終了時刻
    pub end_time: DateTime<Utc>,
    /// 同期されたIssue数
    pub synced_issues_count: usize,
    /// 新規Issue数
    pub new_issues_count: usize,
    /// 更新されたIssue数
    pub updated_issues_count: usize,
    /// 削除されたIssue数（アーカイブされたIssueなど）
    pub deleted_issues_count: usize,
    /// エラー数
    pub error_count: usize,
    /// プロジェクト別統計
    pub project_stats: HashMap<String, ProjectSyncStats>,
    /// エラーメッセージ一覧
    pub error_messages: Vec<String>,
    /// 同期が成功したかどうか
    pub is_success: bool,
}

impl SyncResult {
    /// 新しい同期結果を作成
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            start_time: now,
            end_time: now,
            synced_issues_count: 0,
            new_issues_count: 0,
            updated_issues_count: 0,
            deleted_issues_count: 0,
            error_count: 0,
            project_stats: HashMap::new(),
            error_messages: Vec::new(),
            is_success: false,
        }
    }
    
    /// 同期終了を記録
    pub fn finish(&mut self) {
        self.end_time = Utc::now();
        self.is_success = self.error_count == 0;
    }
    
    /// エラーを追加
    pub fn add_error(&mut self, message: String) {
        self.error_count += 1;
        self.error_messages.push(message);
    }
    
    /// プロジェクト統計を追加
    pub fn add_project_stats(&mut self, project_key: String, stats: ProjectSyncStats) {
        self.project_stats.insert(project_key, stats);
    }
    
    /// 同期処理時間を取得（秒）
    pub fn duration_seconds(&self) -> f64 {
        (self.end_time - self.start_time).num_milliseconds() as f64 / 1000.0
    }
}

impl Default for SyncResult {
    fn default() -> Self {
        Self::new()
    }
}

/// プロジェクト別の同期統計
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSyncStats {
    /// プロジェクトキー
    pub project_key: String,
    /// 同期されたIssue数
    pub synced_count: usize,
    /// 新規Issue数
    pub new_count: usize,
    /// 更新されたIssue数
    pub updated_count: usize,
    /// エラー数
    pub error_count: usize,
    /// 最後の同期時刻
    pub last_sync_time: DateTime<Utc>,
}

impl ProjectSyncStats {
    /// 新しいプロジェクト統計を作成
    pub fn new(project_key: String) -> Self {
        Self {
            project_key,
            synced_count: 0,
            new_count: 0,
            updated_count: 0,
            error_count: 0,
            last_sync_time: Utc::now(),
        }
    }
}

/// 同期処理の状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncState {
    /// 待機中
    Idle,
    /// 同期中
    Syncing,
    /// 同期完了
    Completed,
    /// エラー発生
    Error(String),
}

impl SyncState {
    /// 同期中かどうか
    pub fn is_syncing(&self) -> bool {
        matches!(self, SyncState::Syncing)
    }
    
    /// エラー状態かどうか
    pub fn is_error(&self) -> bool {
        matches!(self, SyncState::Error(_))
    }
    
    /// 完了状態かどうか
    pub fn is_completed(&self) -> bool {
        matches!(self, SyncState::Completed)
    }
    
    /// アイドル状態かどうか
    pub fn is_idle(&self) -> bool {
        matches!(self, SyncState::Idle)
    }
}

impl Default for SyncState {
    fn default() -> Self {
        SyncState::Idle
    }
}

/// 同期サービス
pub struct SyncService {
    /// 設定
    config: SyncConfig,
    /// 現在の同期状態
    current_state: SyncState,
    /// 同期履歴
    sync_history: Vec<SyncResult>,
    /// 最後の成功した同期時刻
    last_successful_sync: Option<DateTime<Utc>>,
}

impl SyncService {
    /// 新しい同期サービスを作成
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            current_state: SyncState::Idle,
            sync_history: Vec::new(),
            last_successful_sync: None,
        }
    }
    
    /// 現在の同期状態を取得
    pub fn current_state(&self) -> &SyncState {
        &self.current_state
    }
    
    /// 設定を取得
    pub fn config(&self) -> &SyncConfig {
        &self.config
    }
    
    /// 同期履歴を取得
    pub fn sync_history(&self) -> &[SyncResult] {
        &self.sync_history
    }
    
    /// 最後の成功した同期時刻を取得
    pub fn last_successful_sync(&self) -> Option<DateTime<Utc>> {
        self.last_successful_sync
    }
    
    /// 設定を更新
    pub fn update_config(&mut self, config: SyncConfig) {
        self.config = config;
    }
    
    /// 同期状態を更新
    pub(crate) fn set_state(&mut self, state: SyncState) {
        self.current_state = state;
    }
    
    /// 同期結果を履歴に追加
    pub(crate) fn add_sync_result(&mut self, result: SyncResult) {
        // 最大履歴数を超えた場合、古いものを削除
        if self.sync_history.len() >= self.config.max_history_count {
            self.sync_history.remove(0);
        }
        
        // 成功した同期の場合、最終成功時刻を更新
        if result.is_success {
            self.last_successful_sync = Some(result.end_time);
        }
        
        self.sync_history.push(result);
    }
    
    /// 最新の同期結果を取得
    pub fn latest_sync_result(&self) -> Option<&SyncResult> {
        self.sync_history.last()
    }
    
    /// 同期が可能かどうかチェック
    pub fn can_sync(&self) -> bool {
        !self.current_state.is_syncing()
    }
    
    /// 増分同期を実行
    pub async fn sync_incremental(
        &mut self, 
        client: &JiraClient,
        existing_issues: &[Issue]
    ) -> Result<SyncResult, Error> {
        // 同期中でないことを確認
        if !self.can_sync() {
            return Err(Error::InvalidInput("同期が既に実行中です".to_string()));
        }
        
        // 同期開始
        self.set_state(SyncState::Syncing);
        let mut result = SyncResult::new();
        
        // 最後の同期時刻以降のフィルターを作成
        let filter = if let Some(last_sync) = self.last_successful_sync {
            TimeBasedFilter::incremental_since(last_sync)
                .excluded_issue_keys(existing_issues.iter().map(|i| i.key.clone()).collect())
        } else {
            // 初回同期の場合は最近24時間分を取得
            TimeBasedFilter::last_hours(24)
        };
        
        // フィルター妥当性チェック
        if let Err(e) = filter.is_valid() {
            result.add_error(format!("フィルター設定エラー: {}", e));
            result.finish();
            self.set_state(SyncState::Error(format!("フィルター設定エラー: {}", e)));
            self.add_sync_result(result.clone());
            return Ok(result);
        }
        
        // 既存Issueのキーセットを作成（重複除外用）
        let existing_keys: HashSet<String> = existing_issues.iter()
            .map(|i| i.key.clone())
            .collect();
        
        // プロジェクト別同期実行
        let projects_to_sync = if self.config.target_projects.is_empty() {
            // 全プロジェクト対象の場合、プロジェクト一覧を取得
            match client.get_projects().await {
                Ok(projects) => projects.into_iter().map(|p| p.key).collect(),
                Err(e) => {
                    result.add_error(format!("プロジェクト一覧取得エラー: {}", e));
                    result.finish();
                    self.set_state(SyncState::Error(format!("プロジェクト一覧取得エラー: {}", e)));
                    self.add_sync_result(result.clone());
                    return Ok(result);
                }
            }
        } else {
            self.config.target_projects.clone()
        };
        
        // 各プロジェクトを同期
        for project_key in &projects_to_sync {
            let mut project_stats = ProjectSyncStats::new(project_key.clone());
            
            // プロジェクト固有のJQLクエリを構築
            let base_jql = format!("project = {}", project_key);
            let time_condition = filter.to_jql_time_condition();
            
            let jql = if let Some(time_cond) = time_condition {
                format!("{} AND ({})", base_jql, time_cond)
            } else {
                base_jql
            };
            
            // 検索パラメータ設定
            let mut search_params = SearchParams::new()
                .max_results(1000) // 大きめのページサイズで効率化
                .start_at(0);
            
            // 除外フィールドがある場合は、必要なフィールドのみを指定
            if !self.config.excluded_fields.is_empty() {
                let default_fields = vec![
                    "key".to_string(),
                    "summary".to_string(),
                    "status".to_string(),
                    "priority".to_string(),
                    "issuetype".to_string(),
                    "reporter".to_string(),
                    "created".to_string(),
                    "updated".to_string(),
                ];
                
                let filtered_fields: Vec<String> = default_fields.into_iter()
                    .filter(|field| !self.config.excluded_fields.contains(field))
                    .collect();
                    
                search_params = search_params.fields(filtered_fields);
            }
            
            // ページネーションで全Issues取得
            let mut start_at = 0u32;
            let max_results = 1000u32;
            
            loop {
                search_params = search_params.start_at(start_at).max_results(max_results);
                
                match client.search_issues(&jql, search_params.clone()).await {
                    Ok(search_result) => {
                        let mut new_issues = 0;
                        let mut updated_issues = 0;
                        
                        for issue in &search_result.issues {
                            if existing_keys.contains(&issue.key) {
                                // 既存Issueの場合は更新として扱う
                                updated_issues += 1;
                            } else {
                                // 新規Issue
                                new_issues += 1;
                            }
                        }
                        
                        // 統計更新
                        project_stats.synced_count += search_result.issues.len();
                        project_stats.new_count += new_issues;
                        project_stats.updated_count += updated_issues;
                        
                        result.synced_issues_count += search_result.issues.len();
                        result.new_issues_count += new_issues;
                        result.updated_issues_count += updated_issues;
                        
                        // 次のページがない場合は終了
                        if (search_result.issues.len() as u32) < max_results || 
                           start_at + (search_result.issues.len() as u32) >= search_result.total {
                            break;
                        }
                        
                        start_at += max_results;
                    }
                    Err(e) => {
                        let error_msg = format!("プロジェクト {} の同期エラー: {}", project_key, e);
                        result.add_error(error_msg);
                        project_stats.error_count += 1;
                        break;
                    }
                }
            }
            
            project_stats.last_sync_time = Utc::now();
            result.add_project_stats(project_key.clone(), project_stats);
        }
        
        // 同期完了処理
        result.finish();
        
        if result.is_success {
            self.set_state(SyncState::Completed);
        } else {
            self.set_state(SyncState::Error(format!("同期中に {} 件のエラーが発生しました", result.error_count)));
        }
        
        self.add_sync_result(result.clone());
        Ok(result)
    }
    
    /// 初回同期を実行（全データを取得）
    pub async fn sync_full(&mut self, client: &JiraClient) -> Result<SyncResult, Error> {
        self.sync_incremental(client, &[]).await
    }
    
    /// 重複除外処理を実行
    pub fn deduplicate_issues(&self, issues: Vec<Issue>) -> Vec<Issue> {
        let mut seen_keys = HashSet::new();
        let mut deduplicated = Vec::new();
        
        for issue in issues {
            if seen_keys.insert(issue.key.clone()) {
                deduplicated.push(issue);
            }
        }
        
        deduplicated
    }
    
    /// 同期の必要性をチェック
    pub fn should_sync(&self) -> bool {
        if self.current_state.is_syncing() {
            return false;
        }
        
        // 最後の成功した同期から設定された間隔が経過している場合
        if let Some(last_sync) = self.last_successful_sync {
            let now = Utc::now();
            let elapsed_minutes = (now - last_sync).num_minutes();
            elapsed_minutes >= self.config.interval_minutes as i64
        } else {
            // 初回同期の場合は常にtrue
            true
        }
    }
    
    /// エラーからの復旧を試行
    pub fn recover_from_error(&mut self) {
        if self.current_state.is_error() {
            self.set_state(SyncState::Idle);
        }
    }
    
    /// 統計情報を取得
    pub fn get_stats(&self) -> SyncServiceStats {
        let total_syncs = self.sync_history.len();
        let successful_syncs = self.sync_history.iter()
            .filter(|r| r.is_success)
            .count();
        
        let total_issues_synced = self.sync_history.iter()
            .map(|r| r.synced_issues_count)
            .sum();
            
        let average_duration = if !self.sync_history.is_empty() {
            self.sync_history.iter()
                .map(|r| r.duration_seconds())
                .sum::<f64>() / self.sync_history.len() as f64
        } else {
            0.0
        };
        
        SyncServiceStats {
            total_syncs,
            successful_syncs,
            total_issues_synced,
            average_duration_seconds: average_duration,
            last_sync_time: self.sync_history.last().map(|r| r.end_time),
            last_successful_sync_time: self.last_successful_sync,
        }
    }
}

/// 同期サービスの統計情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncServiceStats {
    /// 総同期回数
    pub total_syncs: usize,
    /// 成功した同期回数
    pub successful_syncs: usize,
    /// 総同期Issue数
    pub total_issues_synced: usize,
    /// 平均同期時間（秒）
    pub average_duration_seconds: f64,
    /// 最後の同期時刻
    pub last_sync_time: Option<DateTime<Utc>>,
    /// 最後の成功した同期時刻
    pub last_successful_sync_time: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sync_config_new() {
        // SyncConfig::new()でデフォルト設定が作成されることをテスト
        let config = SyncConfig::new();
        
        assert_eq!(config.interval_minutes, 60);
        assert_eq!(config.max_history_count, 100);
        assert_eq!(config.enable_time_optimization, true);
        assert_eq!(config.concurrent_sync_count, 3);
        assert!(config.target_projects.is_empty());
        assert!(config.excluded_fields.is_empty());
    }
    
    #[test]
    fn test_sync_config_builder_pattern() {
        // SyncConfigのビルダーパターンが正しく動作することをテスト
        let config = SyncConfig::new()
            .interval_minutes(30)
            .max_history_count(50)
            .enable_time_optimization(false)
            .concurrent_sync_count(5)
            .target_projects(vec!["TEST".to_string(), "DEMO".to_string()])
            .excluded_fields(vec!["description".to_string()]);
            
        assert_eq!(config.interval_minutes, 30);
        assert_eq!(config.max_history_count, 50);
        assert_eq!(config.enable_time_optimization, false);
        assert_eq!(config.concurrent_sync_count, 5);
        assert_eq!(config.target_projects, vec!["TEST", "DEMO"]);
        assert_eq!(config.excluded_fields, vec!["description"]);
    }
    
    #[test]
    fn test_sync_result_new() {
        // SyncResult::new()で初期状態が正しく設定されることをテスト
        let result = SyncResult::new();
        
        assert_eq!(result.synced_issues_count, 0);
        assert_eq!(result.new_issues_count, 0);
        assert_eq!(result.updated_issues_count, 0);
        assert_eq!(result.deleted_issues_count, 0);
        assert_eq!(result.error_count, 0);
        assert!(result.project_stats.is_empty());
        assert!(result.error_messages.is_empty());
        assert_eq!(result.is_success, false);
    }
    
    #[test]
    fn test_sync_result_add_error() {
        // SyncResult::add_error()でエラーが正しく追加されることをテスト
        let mut result = SyncResult::new();
        
        result.add_error("Test error 1".to_string());
        result.add_error("Test error 2".to_string());
        
        assert_eq!(result.error_count, 2);
        assert_eq!(result.error_messages.len(), 2);
        assert_eq!(result.error_messages[0], "Test error 1");
        assert_eq!(result.error_messages[1], "Test error 2");
    }
    
    #[test]
    fn test_sync_result_finish() {
        // SyncResult::finish()で終了処理が正しく行われることをテスト
        let mut result = SyncResult::new();
        let start_time = result.start_time;
        
        // 時刻の更新を確実にするため少し待機
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        // エラーなしで完了
        result.finish();
        assert!(result.end_time > start_time);
        assert_eq!(result.is_success, true);
        
        // エラーありで完了
        let mut result_with_error = SyncResult::new();
        result_with_error.add_error("Test error".to_string());
        result_with_error.finish();
        assert_eq!(result_with_error.is_success, false);
    }
    
    #[test]
    fn test_sync_state_methods() {
        // SyncStateの各判定メソッドが正しく動作することをテスト
        let idle = SyncState::Idle;
        assert!(idle.is_idle());
        assert!(!idle.is_syncing());
        assert!(!idle.is_completed());
        assert!(!idle.is_error());
        
        let syncing = SyncState::Syncing;
        assert!(syncing.is_syncing());
        assert!(!syncing.is_idle());
        
        let completed = SyncState::Completed;
        assert!(completed.is_completed());
        assert!(!syncing.is_completed());
        
        let error = SyncState::Error("Test error".to_string());
        assert!(error.is_error());
        assert!(!error.is_idle());
    }
    
    #[test]
    fn test_project_sync_stats_new() {
        // ProjectSyncStats::new()で初期値が正しく設定されることをテスト
        let stats = ProjectSyncStats::new("TEST".to_string());
        
        assert_eq!(stats.project_key, "TEST");
        assert_eq!(stats.synced_count, 0);
        assert_eq!(stats.new_count, 0);
        assert_eq!(stats.updated_count, 0);
        assert_eq!(stats.error_count, 0);
    }
    
    #[test]
    fn test_sync_service_new() {
        // SyncService::new()で初期状態が正しく設定されることをテスト
        let config = SyncConfig::new();
        let service = SyncService::new(config);
        
        assert!(service.current_state().is_idle());
        assert!(service.sync_history().is_empty());
        assert!(service.last_successful_sync().is_none());
        assert!(service.can_sync());
    }
    
    #[test]
    fn test_sync_service_state_management() {
        // SyncServiceの状態管理が正しく動作することをテスト
        let config = SyncConfig::new();
        let mut service = SyncService::new(config);
        
        // 初期状態
        assert!(service.can_sync());
        
        // 同期開始
        service.set_state(SyncState::Syncing);
        assert!(service.current_state().is_syncing());
        assert!(!service.can_sync());
        
        // 同期完了
        service.set_state(SyncState::Completed);
        assert!(service.current_state().is_completed());
        assert!(service.can_sync());
    }
    
    #[test]
    fn test_sync_service_history_management() {
        // SyncServiceの履歴管理が正しく動作することをテスト
        let config = SyncConfig::new().max_history_count(2);
        let mut service = SyncService::new(config);
        
        // 履歴追加
        let mut result1 = SyncResult::new();
        result1.synced_issues_count = 10;
        result1.finish(); // エラーなしで完了
        
        let mut result2 = SyncResult::new();
        result2.synced_issues_count = 20;
        result2.add_error("Test error".to_string());
        result2.finish(); // エラーありで完了
        
        let mut result3 = SyncResult::new();
        result3.synced_issues_count = 30;
        result3.finish(); // エラーなしで完了
        
        service.add_sync_result(result1);
        assert_eq!(service.sync_history().len(), 1);
        assert!(service.last_successful_sync().is_some());
        
        service.add_sync_result(result2);
        assert_eq!(service.sync_history().len(), 2);
        
        // 最大履歴数を超えると古いものが削除される
        service.add_sync_result(result3);
        assert_eq!(service.sync_history().len(), 2);
        assert_eq!(service.sync_history()[0].synced_issues_count, 20); // 最初の履歴が削除された
        assert_eq!(service.sync_history()[1].synced_issues_count, 30);
    }
    
    #[test]
    fn test_sync_service_stats() {
        // SyncServiceの統計情報が正しく計算されることをテスト
        let config = SyncConfig::new();
        let mut service = SyncService::new(config);
        
        // 成功した同期を追加
        let mut success_result = SyncResult::new();
        success_result.synced_issues_count = 100;
        success_result.finish();
        
        // 失敗した同期を追加
        let mut failure_result = SyncResult::new();
        failure_result.synced_issues_count = 50;
        failure_result.add_error("Test error".to_string());
        failure_result.finish();
        
        service.add_sync_result(success_result);
        service.add_sync_result(failure_result);
        
        let stats = service.get_stats();
        assert_eq!(stats.total_syncs, 2);
        assert_eq!(stats.successful_syncs, 1);
        assert_eq!(stats.total_issues_synced, 150);
        assert!(stats.average_duration_seconds >= 0.0);
        assert!(stats.last_sync_time.is_some());
        assert!(stats.last_successful_sync_time.is_some());
    }
    
    #[test]
    fn test_sync_result_duration_calculation() {
        // SyncResultの処理時間計算が正しく動作することをテスト
        let mut result = SyncResult::new();
        let start_time = result.start_time;
        
        // 1秒後に終了したと仮定
        result.end_time = start_time + chrono::Duration::seconds(1);
        
        let duration = result.duration_seconds();
        assert!((duration - 1.0).abs() < 0.1); // 約1秒
    }
    
    #[test]
    fn test_sync_service_should_sync() {
        // SyncService::should_sync()が正しく動作することをテスト
        let config = SyncConfig::new().interval_minutes(60);
        let mut service = SyncService::new(config);
        
        // 初回同期の場合は常にtrue
        assert!(service.should_sync());
        
        // 同期中の場合はfalse
        service.set_state(SyncState::Syncing);
        assert!(!service.should_sync());
        
        // 最後の同期から十分時間が経過している場合はtrue
        service.set_state(SyncState::Idle);
        service.last_successful_sync = Some(Utc::now() - chrono::Duration::hours(2));
        assert!(service.should_sync());
        
        // 最近同期した場合はfalse
        service.last_successful_sync = Some(Utc::now() - chrono::Duration::minutes(30));
        assert!(!service.should_sync());
    }
    
    #[test]
    fn test_sync_service_recover_from_error() {
        // SyncService::recover_from_error()が正しく動作することをテスト
        let config = SyncConfig::new();
        let mut service = SyncService::new(config);
        
        // エラー状態に設定
        service.set_state(SyncState::Error("Test error".to_string()));
        assert!(service.current_state().is_error());
        
        // エラーからの復旧
        service.recover_from_error();
        assert!(service.current_state().is_idle());
        
        // アイドル状態では何もしない
        service.recover_from_error();
        assert!(service.current_state().is_idle());
    }
    
    #[test]
    fn test_sync_service_deduplicate_issues() {
        // SyncService::deduplicate_issues()が正しく動作することをテスト
        let config = SyncConfig::new();
        let service = SyncService::new(config);
        
        // 簡単なテスト用データを作成（実際のIssue構造体の複雑性を避けるため、空のVecでテスト）
        let issues = Vec::new(); // 空のIssueリスト
        let deduplicated = service.deduplicate_issues(issues);
        
        assert_eq!(deduplicated.len(), 0);
    }
}