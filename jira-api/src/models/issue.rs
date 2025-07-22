use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
    pub fields: IssueFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<Changelog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueFields {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<serde_json::Value>, // 文字列またはADF形式のオブジェクト
    #[serde(rename = "issuetype")]
    pub issue_type: IssueType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<User>,
    pub reporter: User,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(rename = "resolutiondate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<Project>,

    // カスタムフィールドは動的に追加
    #[serde(flatten)]
    pub custom_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changelog {
    #[serde(rename = "startAt")]
    pub start_at: u32,
    #[serde(rename = "maxResults")]
    pub max_results: u32,
    pub total: u32,
    pub histories: Vec<History>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub id: String,
    pub author: User,
    pub created: DateTime<Utc>,
    pub items: Vec<HistoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub field: String,
    #[serde(rename = "fieldtype")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(rename = "fromString")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(rename = "toString")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_string: Option<String>,
}

// Re-export dependent types that will be defined in other modules
use super::{IssueType, Priority, Project, Status, User};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_issue_deserialization() {
        let json_data = json!({
            "id": "10000",
            "key": "TEST-1",
            "self": "https://example.atlassian.net/rest/api/3/issue/10000",
            "fields": {
                "summary": "Test Issue",
                "description": "This is a test issue",
                "issuetype": {
                    "id": "1",
                    "name": "Bug",
                    "self": "https://example.atlassian.net/rest/api/3/issuetype/1",
                    "subtask": false
                },
                "priority": {
                    "id": "3",
                    "name": "Medium",
                    "self": "https://example.atlassian.net/rest/api/3/priority/3"
                },
                "status": {
                    "id": "1",
                    "name": "To Do",
                    "self": "https://example.atlassian.net/rest/api/3/status/1",
                    "statusCategory": {
                        "id": 2,
                        "key": "new",
                        "name": "To Do",
                        "colorName": "blue-gray"
                    }
                },
                "assignee": null,
                "reporter": {
                    "accountId": "557058:f58131cb-b67d-43c7-b30d-6b58d40bd077",
                    "displayName": "Test User",
                    "emailAddress": "test@example.com",
                    "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:f58131cb"
                },
                "created": "2024-01-01T00:00:00.000Z",
                "updated": "2024-01-02T00:00:00.000Z",
                "customfield_10001": "Custom Value"
            }
        });

        let issue: Issue = serde_json::from_value(json_data).unwrap();

        assert_eq!(issue.id, "10000");
        assert_eq!(issue.key, "TEST-1");
        assert_eq!(issue.fields.summary, "Test Issue");
        assert_eq!(
            issue.fields.description,
            Some(serde_json::Value::String(
                "This is a test issue".to_string()
            ))
        );
        assert_eq!(
            issue.fields.custom_fields.get("customfield_10001").unwrap(),
            "Custom Value"
        );
    }
}
