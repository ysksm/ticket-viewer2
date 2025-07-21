use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueType {
    pub id: String,
    pub name: String,
    #[serde(rename = "self")]
    pub self_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
    #[serde(rename = "iconUrl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_issue_type_deserialization() {
        let json_data = json!({
            "id": "1",
            "name": "Bug",
            "self": "https://example.atlassian.net/rest/api/3/issuetype/1",
            "description": "A problem or error",
            "subtask": false,
            "iconUrl": "https://example.atlassian.net/images/icons/bug.png"
        });

        let issue_type: IssueType = serde_json::from_value(json_data).unwrap();
        
        assert_eq!(issue_type.id, "1");
        assert_eq!(issue_type.name, "Bug");
        assert_eq!(issue_type.subtask, Some(false));
    }
}