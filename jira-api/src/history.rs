use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{DateRange, Error};

/// 課題の変更履歴レコード
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueHistory {
    /// 履歴レコードID（データベース生成）
    pub history_id: Option<i64>,
    /// 課題ID（issuesテーブルへの外部キー）
    pub issue_id: String,
    /// JIRAの課題キー（例：TEST-123）
    pub issue_key: String,
    /// JIRA内部の変更ID
    pub change_id: String,
    /// 変更が発生した日時
    pub change_timestamp: DateTime<Utc>,
    /// 変更者情報
    pub author: Option<HistoryAuthor>,
    /// 変更されたフィールド名
    pub field_name: String,
    /// カスタムフィールドのID（カスタムフィールドの場合）
    pub field_id: Option<String>,
    /// 変更前の値
    pub from_value: Option<String>,
    /// 変更後の値
    pub to_value: Option<String>,
    /// 変更前の表示値（ユーザーが見る値）
    pub from_display_value: Option<String>,
    /// 変更後の表示値（ユーザーが見る値）
    pub to_display_value: Option<String>,
    /// レコード作成日時
    pub created_at: DateTime<Utc>,
}

/// 変更者の情報
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryAuthor {
    /// アカウントID
    pub account_id: String,
    /// 表示名
    pub display_name: String,
    /// メールアドレス（オプション）
    pub email_address: Option<String>,
}

/// 履歴データのフィルター条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFilter {
    /// 対象課題キー
    pub issue_keys: Option<Vec<String>>,
    /// 対象フィールド名
    pub field_names: Option<Vec<String>>,
    /// 変更者アカウントID
    pub authors: Option<Vec<String>>,
    /// 変更日時範囲
    pub date_range: Option<DateRange>,
    /// 変更タイプ
    pub change_types: Option<Vec<ChangeType>>,
    /// 取得件数制限
    pub limit: Option<usize>,
    /// ソート順
    pub sort_order: HistorySortOrder,
}

/// 変更の種類
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    /// ステータス変更
    StatusChange,
    /// 担当者変更
    AssigneeChange,
    /// 優先度変更
    PriorityChange,
    /// 一般的なフィールド更新
    FieldUpdate,
    /// カスタムフィールド変更
    CustomField,
}

/// 履歴のソート順
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HistorySortOrder {
    /// 時系列昇順
    TimestampAsc,
    /// 時系列降順（デフォルト）
    TimestampDesc,
    /// 課題キー順
    IssueKey,
    /// フィールド名順
    FieldName,
}

/// 履歴データの統計情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStats {
    /// 総変更数
    pub total_changes: usize,
    /// 履歴がある課題数
    pub unique_issues: usize,
    /// 変更者数
    pub unique_authors: usize,
    /// フィールド別変更数
    pub field_change_counts: HashMap<String, usize>,
    /// 最古の変更日時
    pub oldest_change: Option<DateTime<Utc>>,
    /// 最新の変更日時
    pub newest_change: Option<DateTime<Utc>>,
}

impl IssueHistory {
    /// 新しい履歴レコードを作成
    pub fn new(
        issue_id: String,
        issue_key: String,
        change_id: String,
        change_timestamp: DateTime<Utc>,
        field_name: String,
    ) -> Self {
        Self {
            history_id: None,
            issue_id,
            issue_key,
            change_id,
            change_timestamp,
            author: None,
            field_name,
            field_id: None,
            from_value: None,
            to_value: None,
            from_display_value: None,
            to_display_value: None,
            created_at: Utc::now(),
        }
    }

    /// 変更者情報を設定
    pub fn with_author(mut self, author: HistoryAuthor) -> Self {
        self.author = Some(author);
        self
    }

    /// フィールド値の変更情報を設定
    pub fn with_field_change(
        mut self,
        from_value: Option<String>,
        to_value: Option<String>,
        from_display: Option<String>,
        to_display: Option<String>,
    ) -> Self {
        self.from_value = from_value;
        self.to_value = to_value;
        self.from_display_value = from_display;
        self.to_display_value = to_display;
        self
    }

    /// カスタムフィールドIDを設定
    pub fn with_field_id(mut self, field_id: String) -> Self {
        self.field_id = Some(field_id);
        self
    }

    /// レコード作成日時を設定
    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = created_at;
        self
    }

    /// 変更タイプを判定
    pub fn change_type(&self) -> ChangeType {
        match self.field_name.as_str() {
            "status" => ChangeType::StatusChange,
            "assignee" => ChangeType::AssigneeChange,
            "priority" => ChangeType::PriorityChange,
            _ if self.field_id.is_some() => ChangeType::CustomField,
            _ => ChangeType::FieldUpdate,
        }
    }

    /// 変更の概要を文字列で取得
    pub fn change_summary(&self) -> String {
        let author_name = self.author
            .as_ref()
            .map(|a| a.display_name.as_str())
            .unwrap_or("System");
            
        let from = self.from_display_value
            .as_deref()
            .unwrap_or("None");
            
        let to = self.to_display_value
            .as_deref()
            .unwrap_or("None");

        format!("{}: {} changed {} from '{}' to '{}'",
            self.change_timestamp.format("%Y-%m-%d %H:%M:%S"),
            author_name,
            self.field_name,
            from,
            to
        )
    }
}

impl HistoryFilter {
    /// 新しいフィルターを作成
    pub fn new() -> Self {
        Self {
            issue_keys: None,
            field_names: None,
            authors: None,
            date_range: None,
            change_types: None,
            limit: None,
            sort_order: HistorySortOrder::TimestampDesc,
        }
    }

    /// 課題キーでフィルター
    pub fn issue_keys(mut self, keys: Vec<String>) -> Self {
        self.issue_keys = Some(keys);
        self
    }

    /// フィールド名でフィルター
    pub fn field_names(mut self, fields: Vec<String>) -> Self {
        self.field_names = Some(fields);
        self
    }

    /// 変更者でフィルター
    pub fn authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    /// 日付範囲でフィルター
    pub fn date_range(mut self, range: DateRange) -> Self {
        self.date_range = Some(range);
        self
    }

    /// 変更タイプでフィルター
    pub fn change_types(mut self, types: Vec<ChangeType>) -> Self {
        self.change_types = Some(types);
        self
    }

    /// 取得件数制限
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// ソート順を設定
    pub fn sort_order(mut self, order: HistorySortOrder) -> Self {
        self.sort_order = order;
        self
    }

    /// フィルターが有効かどうか検証
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err(Error::InvalidFilter("Limit cannot be zero".to_string()));
            }
        }

        if let Some(range) = &self.date_range {
            range.validate()?;
        }

        Ok(())
    }
}

impl Default for HistoryFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HistorySortOrder {
    fn default() -> Self {
        HistorySortOrder::TimestampDesc
    }
}

impl HistoryStats {
    /// 新しい統計情報を作成
    pub fn new() -> Self {
        Self {
            total_changes: 0,
            unique_issues: 0,
            unique_authors: 0,
            field_change_counts: HashMap::new(),
            oldest_change: None,
            newest_change: None,
        }
    }

    /// 統計情報を更新
    pub fn update(&mut self, histories: &[IssueHistory]) {
        self.total_changes = histories.len();
        
        // ユニークな課題数
        let unique_issues: std::collections::HashSet<&String> = histories
            .iter()
            .map(|h| &h.issue_key)
            .collect();
        self.unique_issues = unique_issues.len();

        // ユニークな変更者数
        let unique_authors: std::collections::HashSet<&String> = histories
            .iter()
            .filter_map(|h| h.author.as_ref().map(|a| &a.account_id))
            .collect();
        self.unique_authors = unique_authors.len();

        // フィールド別変更数
        self.field_change_counts.clear();
        for history in histories {
            *self.field_change_counts
                .entry(history.field_name.clone())
                .or_insert(0) += 1;
        }

        // 最古・最新の変更日時
        if !histories.is_empty() {
            self.oldest_change = histories
                .iter()
                .map(|h| h.change_timestamp)
                .min();
            self.newest_change = histories
                .iter()
                .map(|h| h.change_timestamp)
                .max();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_issue_history_creation() {
        let history = IssueHistory::new(
            "10000".to_string(),
            "TEST-123".to_string(),
            "change_1".to_string(),
            Utc::now(),
            "status".to_string(),
        );

        assert_eq!(history.issue_id, "10000");
        assert_eq!(history.issue_key, "TEST-123");
        assert_eq!(history.field_name, "status");
        assert_eq!(history.change_type(), ChangeType::StatusChange);
    }

    #[test]
    fn test_history_with_author() {
        let author = HistoryAuthor {
            account_id: "user123".to_string(),
            display_name: "Test User".to_string(),
            email_address: Some("test@example.com".to_string()),
        };

        let history = IssueHistory::new(
            "10000".to_string(),
            "TEST-123".to_string(),
            "change_1".to_string(),
            Utc::now(),
            "assignee".to_string(),
        ).with_author(author.clone());

        assert!(history.author.is_some());
        assert_eq!(history.author.as_ref().unwrap().account_id, "user123");
        assert_eq!(history.change_type(), ChangeType::AssigneeChange);
    }

    #[test]
    fn test_history_filter_builder() {
        let filter = HistoryFilter::new()
            .issue_keys(vec!["TEST-123".to_string()])
            .field_names(vec!["status".to_string()])
            .limit(10);

        assert_eq!(filter.issue_keys.unwrap(), vec!["TEST-123"]);
        assert_eq!(filter.field_names.unwrap(), vec!["status"]);
        assert_eq!(filter.limit.unwrap(), 10);
    }

    #[test]
    fn test_change_type_detection() {
        let status_history = IssueHistory::new(
            "1".to_string(), "TEST-1".to_string(), "c1".to_string(),
            Utc::now(), "status".to_string()
        );
        assert_eq!(status_history.change_type(), ChangeType::StatusChange);

        let custom_history = IssueHistory::new(
            "1".to_string(), "TEST-1".to_string(), "c2".to_string(),
            Utc::now(), "customfield_10001".to_string()
        ).with_field_id("customfield_10001".to_string());
        assert_eq!(custom_history.change_type(), ChangeType::CustomField);
    }

    #[test]
    fn test_history_stats() {
        let mut stats = HistoryStats::new();
        
        let histories = vec![
            IssueHistory::new(
                "1".to_string(), "TEST-1".to_string(), "c1".to_string(),
                Utc::now(), "status".to_string()
            ),
            IssueHistory::new(
                "2".to_string(), "TEST-2".to_string(), "c2".to_string(),
                Utc::now(), "status".to_string()
            ),
            IssueHistory::new(
                "1".to_string(), "TEST-1".to_string(), "c3".to_string(),
                Utc::now(), "assignee".to_string()
            ),
        ];

        stats.update(&histories);
        
        assert_eq!(stats.total_changes, 3);
        assert_eq!(stats.unique_issues, 2); // TEST-1, TEST-2
        assert_eq!(stats.field_change_counts.get("status"), Some(&2));
        assert_eq!(stats.field_change_counts.get("assignee"), Some(&1));
    }

    #[test]
    fn test_change_summary() {
        let author = HistoryAuthor {
            account_id: "user123".to_string(),
            display_name: "Test User".to_string(),
            email_address: None,
        };

        let history = IssueHistory::new(
            "10000".to_string(),
            "TEST-123".to_string(),
            "change_1".to_string(),
            Utc::now(),
            "status".to_string(),
        )
        .with_author(author)
        .with_field_change(
            Some("Open".to_string()),
            Some("In Progress".to_string()),
            Some("Open".to_string()),
            Some("In Progress".to_string()),
        );

        let summary = history.change_summary();
        assert!(summary.contains("Test User"));
        assert!(summary.contains("status"));
        assert!(summary.contains("'Open'"));
        assert!(summary.contains("'In Progress'"));
    }

    #[test]
    fn test_filter_validation() {
        let valid_filter = HistoryFilter::new().limit(10);
        assert!(valid_filter.validate().is_ok());

        let invalid_filter = HistoryFilter::new().limit(0);
        assert!(invalid_filter.validate().is_err());
    }
}