use crate::{Error, HistoryAuthor, IssueHistory};
use chrono::Utc;
use serde_json::Value;

/// JIRAのchangelogを解析して履歴データを生成するパーサー
pub struct ChangelogParser;

impl ChangelogParser {
    /// JIRAのchangelog JSONを解析してIssueHistoryレコードを生成
    pub fn parse_changelog(
        issue_id: &str,
        issue_key: &str,
        changelog_json: &Value,
    ) -> Result<Vec<IssueHistory>, Error> {
        let mut histories = Vec::new();

        // changelog.historiesが存在することを確認
        let histories_array = changelog_json
            .get("histories")
            .and_then(|h| h.as_array())
            .ok_or_else(|| Error::InvalidData("No histories array in changelog".to_string()))?;

        for history_entry in histories_array {
            // 変更ID、作成日時、作成者の情報を取得
            let change_id = history_entry
                .get("id")
                .and_then(|id| id.as_str())
                .ok_or_else(|| Error::InvalidData("Missing change id in history".to_string()))?;

            let created_str = history_entry
                .get("created")
                .and_then(|c| c.as_str())
                .ok_or_else(|| {
                    Error::InvalidData("Missing created timestamp in history".to_string())
                })?;

            let change_timestamp = Self::parse_iso_timestamp(created_str)
                .map_err(|e| Error::InvalidData(format!("Invalid timestamp format: {}", e)))?;

            // 作成者情報の取得（オプション）
            let author = if let Some(author_obj) = history_entry.get("author") {
                Some(Self::parse_author(author_obj)?)
            } else {
                None
            };

            // items配列から個別の変更を処理
            let items = history_entry
                .get("items")
                .and_then(|i| i.as_array())
                .ok_or_else(|| Error::InvalidData("Missing items array in history".to_string()))?;

            for item in items {
                let field_name = item.get("field").and_then(|f| f.as_str()).ok_or_else(|| {
                    Error::InvalidData("Missing field name in history item".to_string())
                })?;

                let field_id = item
                    .get("fieldId")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string());

                let from_value = item
                    .get("from")
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string());

                let to_value = item
                    .get("to")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());

                let from_display = item
                    .get("fromString")
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string());

                let to_display = item
                    .get("toString")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());

                let mut history = IssueHistory::new(
                    issue_id.to_string(),
                    issue_key.to_string(),
                    change_id.to_string(),
                    change_timestamp,
                    field_name.to_string(),
                );

                if let Some(author) = author.clone() {
                    history = history.with_author(author);
                }

                history = history.with_field_change(from_value, to_value, from_display, to_display);

                if let Some(field_id) = field_id {
                    history = history.with_field_id(field_id);
                }

                histories.push(history);
            }
        }

        Ok(histories)
    }

    /// ISO形式のタイムスタンプをパース
    fn parse_iso_timestamp(timestamp: &str) -> Result<chrono::DateTime<Utc>, chrono::ParseError> {
        // JIRA changelogのタイムスタンプ形式: "2024-01-15T10:30:00.000+0000"
        // RFC3339形式に変換してパース
        let rfc3339_format = if timestamp.contains('+') {
            // "2024-01-15T10:30:00.000+0000" → "2024-01-15T10:30:00.000+00:00"
            timestamp.replace("+0000", "+00:00")
        } else {
            timestamp.to_string()
        };

        chrono::DateTime::parse_from_rfc3339(&rfc3339_format).map(|dt| dt.with_timezone(&Utc))
    }

    /// 作成者情報をパース
    fn parse_author(author_obj: &Value) -> Result<HistoryAuthor, Error> {
        let account_id = author_obj
            .get("accountId")
            .and_then(|id| id.as_str())
            .ok_or_else(|| Error::InvalidData("Missing accountId in author".to_string()))?;

        let display_name = author_obj
            .get("displayName")
            .and_then(|name| name.as_str())
            .ok_or_else(|| Error::InvalidData("Missing displayName in author".to_string()))?;

        let email_address = author_obj
            .get("emailAddress")
            .and_then(|email| email.as_str())
            .map(|s| s.to_string());

        Ok(HistoryAuthor {
            account_id: account_id.to_string(),
            display_name: display_name.to_string(),
            email_address,
        })
    }

    /// 特定のフィールド変更のみを抽出
    pub fn extract_field_changes(
        histories: &[IssueHistory],
        field_names: &[String],
    ) -> Vec<IssueHistory> {
        histories
            .iter()
            .filter(|h| field_names.contains(&h.field_name))
            .cloned()
            .collect()
    }

    /// 変更の統計情報を生成
    pub fn generate_change_summary(
        histories: &[IssueHistory],
    ) -> std::collections::HashMap<String, usize> {
        let mut summary = std::collections::HashMap::new();

        for history in histories {
            *summary.entry(history.field_name.clone()).or_insert(0) += 1;
        }

        summary
    }

    /// 変更タイプ別に履歴を分類
    pub fn group_by_change_type(
        histories: &[IssueHistory],
    ) -> std::collections::HashMap<String, Vec<IssueHistory>> {
        let mut groups = std::collections::HashMap::new();

        for history in histories {
            let change_type = match history.field_name.as_str() {
                "status" => "Status Changes",
                "assignee" => "Assignee Changes",
                "priority" => "Priority Changes",
                _ if history.field_id.is_some() => "Custom Field Changes",
                _ => "Other Changes",
            };

            groups
                .entry(change_type.to_string())
                .or_insert_with(Vec::new)
                .push(history.clone());
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_changelog_basic() {
        let changelog = json!({
            "histories": [
                {
                    "id": "12345",
                    "created": "2024-01-15T10:30:00.000+0000",
                    "author": {
                        "accountId": "user123",
                        "displayName": "Test User",
                        "emailAddress": "test@example.com"
                    },
                    "items": [
                        {
                            "field": "status",
                            "from": "1",
                            "to": "3",
                            "fromString": "Open",
                            "toString": "In Progress"
                        }
                    ]
                }
            ]
        });

        let result = ChangelogParser::parse_changelog("10000", "TEST-123", &changelog);

        assert!(result.is_ok());
        let histories = result.unwrap();
        assert_eq!(histories.len(), 1);

        let history = &histories[0];
        assert_eq!(history.issue_id, "10000");
        assert_eq!(history.issue_key, "TEST-123");
        assert_eq!(history.change_id, "12345");
        assert_eq!(history.field_name, "status");
        assert_eq!(history.from_value, Some("1".to_string()));
        assert_eq!(history.to_value, Some("3".to_string()));
        assert_eq!(history.from_display_value, Some("Open".to_string()));
        assert_eq!(history.to_display_value, Some("In Progress".to_string()));

        assert!(history.author.is_some());
        let author = history.author.as_ref().unwrap();
        assert_eq!(author.account_id, "user123");
        assert_eq!(author.display_name, "Test User");
        assert_eq!(author.email_address, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_parse_changelog_multiple_items() {
        let changelog = json!({
            "histories": [
                {
                    "id": "12345",
                    "created": "2024-01-15T10:30:00.000+0000",
                    "items": [
                        {
                            "field": "status",
                            "from": "1",
                            "to": "3",
                            "fromString": "Open",
                            "toString": "In Progress"
                        },
                        {
                            "field": "assignee",
                            "from": null,
                            "to": "user456",
                            "fromString": null,
                            "toString": "Jane Doe"
                        }
                    ]
                }
            ]
        });

        let result = ChangelogParser::parse_changelog("10000", "TEST-123", &changelog);

        assert!(result.is_ok());
        let histories = result.unwrap();
        assert_eq!(histories.len(), 2);

        let status_change = histories.iter().find(|h| h.field_name == "status").unwrap();
        let assignee_change = histories
            .iter()
            .find(|h| h.field_name == "assignee")
            .unwrap();

        assert_eq!(status_change.from_display_value, Some("Open".to_string()));
        assert_eq!(
            assignee_change.to_display_value,
            Some("Jane Doe".to_string())
        );
    }

    #[test]
    fn test_extract_field_changes() {
        let histories = vec![
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c1".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c2".to_string(),
                Utc::now(),
                "assignee".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c3".to_string(),
                Utc::now(),
                "priority".to_string(),
            ),
        ];

        let status_changes =
            ChangelogParser::extract_field_changes(&histories, &vec!["status".to_string()]);

        assert_eq!(status_changes.len(), 1);
        assert_eq!(status_changes[0].field_name, "status");
    }

    #[test]
    fn test_generate_change_summary() {
        let histories = vec![
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c1".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c2".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c3".to_string(),
                Utc::now(),
                "assignee".to_string(),
            ),
        ];

        let summary = ChangelogParser::generate_change_summary(&histories);

        assert_eq!(summary.get("status"), Some(&2));
        assert_eq!(summary.get("assignee"), Some(&1));
    }

    #[test]
    fn test_group_by_change_type() {
        let histories = vec![
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c1".to_string(),
                Utc::now(),
                "status".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c2".to_string(),
                Utc::now(),
                "assignee".to_string(),
            ),
            IssueHistory::new(
                "1".to_string(),
                "TEST-1".to_string(),
                "c3".to_string(),
                Utc::now(),
                "customfield_10001".to_string(),
            )
            .with_field_id("customfield_10001".to_string()),
        ];

        let groups = ChangelogParser::group_by_change_type(&histories);

        assert!(groups.contains_key("Status Changes"));
        assert!(groups.contains_key("Assignee Changes"));
        assert!(groups.contains_key("Custom Field Changes"));

        assert_eq!(groups["Status Changes"].len(), 1);
        assert_eq!(groups["Assignee Changes"].len(), 1);
        assert_eq!(groups["Custom Field Changes"].len(), 1);
    }

    #[test]
    fn test_invalid_changelog_structure() {
        let invalid_changelog = json!({
            "invalid": "structure"
        });

        let result = ChangelogParser::parse_changelog("10000", "TEST-123", &invalid_changelog);

        assert!(result.is_err());
    }
}
