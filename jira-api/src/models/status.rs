use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub id: String,
    pub name: String,
    #[serde(rename = "self")]
    pub self_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "iconUrl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(rename = "statusCategory")]
    pub status_category: StatusCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCategory {
    pub id: u32,
    pub key: String,
    pub name: String,
    #[serde(rename = "colorName")]
    pub color_name: String,
    #[serde(rename = "self")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_status_deserialization() {
        let json_data = json!({
            "id": "1",
            "name": "To Do",
            "self": "https://example.atlassian.net/rest/api/3/status/1",
            "description": "The issue is open and ready for the assignee to start work on it.",
            "iconUrl": "https://example.atlassian.net/images/icons/status_open.png",
            "statusCategory": {
                "id": 2,
                "key": "new",
                "name": "To Do",
                "colorName": "blue-gray",
                "self": "https://example.atlassian.net/rest/api/3/statuscategory/2"
            }
        });

        let status: Status = serde_json::from_value(json_data).unwrap();

        assert_eq!(status.id, "1");
        assert_eq!(status.name, "To Do");
        assert_eq!(status.status_category.key, "new");
        assert_eq!(status.status_category.color_name, "blue-gray");
    }
}
