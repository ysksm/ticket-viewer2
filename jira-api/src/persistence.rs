use crate::{Error, Issue};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// データ永続化ストアの抽象化トレイト
#[async_trait]
pub trait PersistenceStore: Send + Sync {
    /// Issueを保存
    async fn save_issues(&mut self, issues: &[Issue]) -> Result<usize, Error>;

    /// フィルター条件でIssueを読み込み
    async fn load_issues(&self, filter: &IssueFilter) -> Result<Vec<Issue>, Error>;

    /// 全てのIssueを読み込み
    async fn load_all_issues(&self) -> Result<Vec<Issue>, Error>;

    /// Issueの件数を取得
    async fn count_issues(&self, filter: &IssueFilter) -> Result<usize, Error>;

    /// 指定されたIssueキーのIssueを削除
    async fn delete_issues(&mut self, issue_keys: &[String]) -> Result<usize, Error>;

    /// ストレージを最適化（インデックス再構築、圧縮など）
    async fn optimize(&mut self) -> Result<(), Error>;

    /// ストレージの統計情報を取得
    async fn get_stats(&self) -> Result<StorageStats, Error>;

    /// フィルター設定を保存
    async fn save_filter_config(&mut self, config: &FilterConfig) -> Result<(), Error>;

    /// フィルター設定を読み込み
    async fn load_filter_config(&self) -> Result<Option<FilterConfig>, Error>;

    /// 履歴データを保存
    async fn save_issue_history(&mut self, history: &[crate::IssueHistory])
    -> Result<usize, Error>;

    /// 履歴データを取得
    async fn load_issue_history(
        &self,
        filter: &crate::HistoryFilter,
    ) -> Result<Vec<crate::IssueHistory>, Error>;

    /// 履歴統計情報を取得
    async fn get_history_stats(&self) -> Result<crate::HistoryStats, Error>;

    /// 指定課題キーの履歴を削除
    async fn delete_issue_history(&mut self, issue_keys: &[String]) -> Result<usize, Error>;
}

/// Issue検索フィルター
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueFilter {
    /// プロジェクトキー
    pub project_keys: Vec<String>,
    /// ステータス名
    pub statuses: Vec<String>,
    /// 優先度名
    pub priorities: Vec<String>,
    /// Issue種別名
    pub issue_types: Vec<String>,
    /// 報告者のユーザー名
    pub reporters: Vec<String>,
    /// 担当者のユーザー名
    pub assignees: Vec<String>,
    /// 作成日時の範囲
    pub created_range: Option<DateRange>,
    /// 更新日時の範囲
    pub updated_range: Option<DateRange>,
    /// サマリー検索（部分一致）
    pub summary_contains: Option<String>,
    /// 説明検索（部分一致）
    pub description_contains: Option<String>,
    /// ラベル
    pub labels: Vec<String>,
    /// 最大取得件数
    pub limit: Option<usize>,
    /// スキップする件数（オフセット）
    pub offset: Option<usize>,
    /// ソート順
    pub sort_order: SortOrder,
}

impl IssueFilter {
    /// 新しいフィルターを作成
    pub fn new() -> Self {
        Self {
            project_keys: Vec::new(),
            statuses: Vec::new(),
            priorities: Vec::new(),
            issue_types: Vec::new(),
            reporters: Vec::new(),
            assignees: Vec::new(),
            created_range: None,
            updated_range: None,
            summary_contains: None,
            description_contains: None,
            labels: Vec::new(),
            limit: None,
            offset: None,
            sort_order: SortOrder::CreatedDesc,
        }
    }

    /// プロジェクトキーでフィルタ
    pub fn project_keys(mut self, keys: Vec<String>) -> Self {
        self.project_keys = keys;
        self
    }

    /// ステータスでフィルタ
    pub fn statuses(mut self, statuses: Vec<String>) -> Self {
        self.statuses = statuses;
        self
    }

    /// 優先度でフィルタ
    pub fn priorities(mut self, priorities: Vec<String>) -> Self {
        self.priorities = priorities;
        self
    }

    /// Issue種別でフィルタ
    pub fn issue_types(mut self, issue_types: Vec<String>) -> Self {
        self.issue_types = issue_types;
        self
    }

    /// 報告者でフィルタ
    pub fn reporters(mut self, reporters: Vec<String>) -> Self {
        self.reporters = reporters;
        self
    }

    /// 担当者でフィルタ
    pub fn assignees(mut self, assignees: Vec<String>) -> Self {
        self.assignees = assignees;
        self
    }

    /// 作成日時範囲でフィルタ
    pub fn created_range(mut self, range: DateRange) -> Self {
        self.created_range = Some(range);
        self
    }

    /// 更新日時範囲でフィルタ
    pub fn updated_range(mut self, range: DateRange) -> Self {
        self.updated_range = Some(range);
        self
    }

    /// サマリー検索
    pub fn summary_contains(mut self, text: String) -> Self {
        self.summary_contains = Some(text);
        self
    }

    /// 説明検索
    pub fn description_contains(mut self, text: String) -> Self {
        self.description_contains = Some(text);
        self
    }

    /// ラベルでフィルタ
    pub fn labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// 最大取得件数を設定
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// オフセットを設定
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// ソート順を設定
    pub fn sort_order(mut self, order: SortOrder) -> Self {
        self.sort_order = order;
        self
    }

    /// フィルターが空かどうか判定
    pub fn is_empty(&self) -> bool {
        self.project_keys.is_empty()
            && self.statuses.is_empty()
            && self.priorities.is_empty()
            && self.issue_types.is_empty()
            && self.reporters.is_empty()
            && self.assignees.is_empty()
            && self.created_range.is_none()
            && self.updated_range.is_none()
            && self.summary_contains.is_none()
            && self.description_contains.is_none()
            && self.labels.is_empty()
    }

    /// Issueがフィルター条件に一致するかチェック
    pub fn matches(&self, issue: &Issue) -> bool {
        // プロジェクトキーでフィルタ
        if !self.project_keys.is_empty() {
            if let Some(ref project) = issue.fields.project {
                if !self.project_keys.contains(&project.key) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // ステータスでフィルタ
        if !self.statuses.is_empty() && !self.statuses.contains(&issue.fields.status.name) {
            return false;
        }

        // 優先度でフィルタ
        if !self.priorities.is_empty() {
            if let Some(ref priority) = issue.fields.priority {
                if !self.priorities.contains(&priority.name) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Issue種別でフィルタ
        if !self.issue_types.is_empty() && !self.issue_types.contains(&issue.fields.issue_type.name)
        {
            return false;
        }

        // 報告者でフィルタ
        if !self.reporters.is_empty() {
            if !self.reporters.contains(&issue.fields.reporter.display_name) {
                return false;
            }
        }

        // 担当者でフィルタ
        if !self.assignees.is_empty() {
            if let Some(ref assignee) = issue.fields.assignee {
                if !self.assignees.contains(&assignee.display_name) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // 作成日時範囲でフィルタ
        if let Some(ref range) = self.created_range {
            if !range.contains(&issue.fields.created) {
                return false;
            }
        }

        // 更新日時範囲でフィルタ
        if let Some(ref range) = self.updated_range {
            if !range.contains(&issue.fields.updated) {
                return false;
            }
        }

        // サマリー検索
        if let Some(ref text) = self.summary_contains {
            if !issue
                .fields
                .summary
                .to_lowercase()
                .contains(&text.to_lowercase())
            {
                return false;
            }
        }

        // 説明検索
        if let Some(ref text) = self.description_contains {
            if let Some(ref description) = issue.fields.description {
                let description_text = match description {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Object(_) => description.to_string(), // ADF形式の場合は文字列化
                    _ => description.to_string(),
                };
                if !description_text
                    .to_lowercase()
                    .contains(&text.to_lowercase())
                {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl Default for IssueFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// 日時範囲
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    /// 開始日時
    pub start: DateTime<Utc>,
    /// 終了日時
    pub end: DateTime<Utc>,
}

impl DateRange {
    /// 新しい日時範囲を作成
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// 日時範囲の妥当性を検証
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.start > self.end {
            return Err(crate::Error::InvalidFilter(
                "Start date must be before end date".to_string(),
            ));
        }
        Ok(())
    }

    /// 指定した日時が範囲に含まれるかチェック
    pub fn contains(&self, datetime: &DateTime<Utc>) -> bool {
        datetime >= &self.start && datetime <= &self.end
    }

    /// 最近N日間の範囲を作成
    pub fn last_days(days: u32) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(days as i64);
        Self::new(start, end)
    }

    /// 最近N時間の範囲を作成
    pub fn last_hours(hours: u32) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(hours as i64);
        Self::new(start, end)
    }
}

/// ソート順
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    /// 作成日時昇順
    CreatedAsc,
    /// 作成日時降順
    CreatedDesc,
    /// 更新日時昇順
    UpdatedAsc,
    /// 更新日時降順
    UpdatedDesc,
    /// キー昇順
    KeyAsc,
    /// キー降順
    KeyDesc,
    /// 優先度昇順（優先度が高い順）
    PriorityAsc,
    /// 優先度降順（優先度が低い順）
    PriorityDesc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::CreatedDesc
    }
}

/// ストレージ統計情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// 総Issue件数
    pub total_issues: usize,
    /// プロジェクト別Issue件数
    pub issues_by_project: HashMap<String, usize>,
    /// ステータス別Issue件数
    pub issues_by_status: HashMap<String, usize>,
    /// Issue種別別件数
    pub issues_by_type: HashMap<String, usize>,
    /// ストレージサイズ（バイト）
    pub storage_size_bytes: u64,
    /// 最後の更新日時
    pub last_updated: DateTime<Utc>,
    /// インデックス数
    pub index_count: usize,
    /// 圧縮率（0.0-1.0）
    pub compression_ratio: f64,
}

impl StorageStats {
    /// 新しい統計情報を作成
    pub fn new() -> Self {
        Self {
            total_issues: 0,
            issues_by_project: HashMap::new(),
            issues_by_status: HashMap::new(),
            issues_by_type: HashMap::new(),
            storage_size_bytes: 0,
            last_updated: Utc::now(),
            index_count: 0,
            compression_ratio: 0.0,
        }
    }
}

impl Default for StorageStats {
    fn default() -> Self {
        Self::new()
    }
}

/// フィルター設定の永続化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// 設定ID
    pub id: String,
    /// 設定名
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// フィルター条件
    pub filter: IssueFilter,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 更新日時
    pub updated_at: DateTime<Utc>,
    /// 使用回数
    pub usage_count: u32,
    /// 最後に使用した日時
    pub last_used_at: Option<DateTime<Utc>>,
}

impl FilterConfig {
    /// 新しいフィルター設定を作成
    pub fn new(id: String, name: String, filter: IssueFilter) -> Self {
        let now = Utc::now();
        Self {
            id,
            name,
            description: None,
            filter,
            created_at: now,
            updated_at: now,
            usage_count: 0,
            last_used_at: None,
        }
    }

    /// 説明を設定
    pub fn description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 使用回数を増加
    pub fn increment_usage(&mut self) {
        self.usage_count += 1;
        self.last_used_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// フィルターを更新
    pub fn update_filter(&mut self, filter: IssueFilter) {
        self.filter = filter;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_issue_filter_new() {
        // IssueFilter::new()でデフォルト値が正しく設定されることをテスト
        let filter = IssueFilter::new();

        assert!(filter.project_keys.is_empty());
        assert!(filter.statuses.is_empty());
        assert!(filter.priorities.is_empty());
        assert!(filter.issue_types.is_empty());
        assert!(filter.reporters.is_empty());
        assert!(filter.assignees.is_empty());
        assert!(filter.created_range.is_none());
        assert!(filter.updated_range.is_none());
        assert!(filter.summary_contains.is_none());
        assert!(filter.description_contains.is_none());
        assert!(filter.labels.is_empty());
        assert!(filter.limit.is_none());
        assert!(filter.offset.is_none());
        assert!(matches!(filter.sort_order, SortOrder::CreatedDesc));
        assert!(filter.is_empty());
    }

    #[test]
    fn test_issue_filter_builder_pattern() {
        // IssueFilterのビルダーパターンが正しく動作することをテスト
        let filter = IssueFilter::new()
            .project_keys(vec!["TEST".to_string(), "DEMO".to_string()])
            .statuses(vec!["Open".to_string(), "In Progress".to_string()])
            .priorities(vec!["High".to_string()])
            .issue_types(vec!["Bug".to_string()])
            .summary_contains("error".to_string())
            .limit(100)
            .offset(10)
            .sort_order(SortOrder::UpdatedDesc);

        assert_eq!(filter.project_keys, vec!["TEST", "DEMO"]);
        assert_eq!(filter.statuses, vec!["Open", "In Progress"]);
        assert_eq!(filter.priorities, vec!["High"]);
        assert_eq!(filter.issue_types, vec!["Bug"]);
        assert_eq!(filter.summary_contains, Some("error".to_string()));
        assert_eq!(filter.limit, Some(100));
        assert_eq!(filter.offset, Some(10));
        assert!(matches!(filter.sort_order, SortOrder::UpdatedDesc));
        assert!(!filter.is_empty());
    }

    #[test]
    fn test_date_range_new() {
        // DateRange::new()で正しく作成されることをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();

        let range = DateRange::new(start, end);

        assert_eq!(range.start, start);
        assert_eq!(range.end, end);
    }

    #[test]
    fn test_date_range_contains() {
        // DateRange::contains()が正しく動作することをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();
        let range = DateRange::new(start, end);

        // 範囲内
        let within = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        assert!(range.contains(&within));

        // 範囲外（前）
        let before = Utc.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();
        assert!(!range.contains(&before));

        // 範囲外（後）
        let after = Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap();
        assert!(!range.contains(&after));

        // 境界値
        assert!(range.contains(&start));
        assert!(range.contains(&end));
    }

    #[test]
    fn test_date_range_last_days() {
        // DateRange::last_days()が正しく動作することをテスト
        let range = DateRange::last_days(7);

        // 範囲が正しく設定されているか確認
        let duration = range.end - range.start;
        assert_eq!(duration.num_days(), 7);

        // 6日前は範囲内、8日前は範囲外
        let six_days_ago = range.end - chrono::Duration::days(6);
        let eight_days_ago = range.start - chrono::Duration::days(1);

        assert!(range.contains(&six_days_ago));
        assert!(!range.contains(&eight_days_ago));
    }

    #[test]
    fn test_date_range_last_hours() {
        // DateRange::last_hours()が正しく動作することをテスト
        let range = DateRange::last_hours(24);

        // 範囲が正しく設定されているか確認
        let duration = range.end - range.start;
        assert_eq!(duration.num_hours(), 24);

        // 12時間前は範囲内、25時間前は範囲外
        let twelve_hours_ago = range.end - chrono::Duration::hours(12);
        let twenty_five_hours_ago = range.start - chrono::Duration::hours(1);

        assert!(range.contains(&twelve_hours_ago));
        assert!(!range.contains(&twenty_five_hours_ago));
    }

    #[test]
    fn test_storage_stats_new() {
        // StorageStats::new()で初期値が正しく設定されることをテスト
        let stats = StorageStats::new();

        assert_eq!(stats.total_issues, 0);
        assert!(stats.issues_by_project.is_empty());
        assert!(stats.issues_by_status.is_empty());
        assert!(stats.issues_by_type.is_empty());
        assert_eq!(stats.storage_size_bytes, 0);
        assert_eq!(stats.index_count, 0);
        assert_eq!(stats.compression_ratio, 0.0);
    }

    #[test]
    fn test_filter_config_new() {
        // FilterConfig::new()で初期値が正しく設定されることをテスト
        let filter = IssueFilter::new().project_keys(vec!["TEST".to_string()]);
        let config = FilterConfig::new(
            "config1".to_string(),
            "Test Filter".to_string(),
            filter.clone(),
        );

        assert_eq!(config.id, "config1");
        assert_eq!(config.name, "Test Filter");
        assert!(config.description.is_none());
        assert_eq!(config.filter.project_keys, vec!["TEST"]);
        assert_eq!(config.usage_count, 0);
        assert!(config.last_used_at.is_none());
    }

    #[test]
    fn test_filter_config_increment_usage() {
        // FilterConfig::increment_usage()が正しく動作することをテスト
        let filter = IssueFilter::new();
        let mut config =
            FilterConfig::new("config1".to_string(), "Test Filter".to_string(), filter);

        let initial_updated_at = config.updated_at;

        // わずかに待機してタイムスタンプの差を確保
        std::thread::sleep(std::time::Duration::from_millis(1));

        // 使用回数を増加
        config.increment_usage();

        assert_eq!(config.usage_count, 1);
        assert!(config.last_used_at.is_some());
        assert!(config.updated_at > initial_updated_at);

        // 再度増加
        config.increment_usage();

        assert_eq!(config.usage_count, 2);
    }

    #[test]
    fn test_filter_config_update_filter() {
        // FilterConfig::update_filter()が正しく動作することをテスト
        let initial_filter = IssueFilter::new().project_keys(vec!["TEST".to_string()]);
        let mut config = FilterConfig::new(
            "config1".to_string(),
            "Test Filter".to_string(),
            initial_filter,
        );

        let initial_updated_at = config.updated_at;

        // 時刻の更新を確実にするため少し待機
        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_filter = IssueFilter::new()
            .project_keys(vec!["DEMO".to_string()])
            .statuses(vec!["Open".to_string()]);

        config.update_filter(new_filter);

        assert_eq!(config.filter.project_keys, vec!["DEMO"]);
        assert_eq!(config.filter.statuses, vec!["Open"]);
        assert!(config.updated_at > initial_updated_at);
    }
}
