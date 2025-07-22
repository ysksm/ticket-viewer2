use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Priority {
    pub id: String,
    pub name: String,
    #[serde(rename = "self")]
    pub self_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "iconUrl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(rename = "statusColor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_color: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_priority_deserialization() {
        let json_data = json!({
            "id": "3",
            "name": "Medium",
            "self": "https://example.atlassian.net/rest/api/3/priority/3",
            "iconUrl": "https://example.atlassian.net/images/icons/priority_medium.png",
            "statusColor": "#EA7D24"
        });

        let priority: Priority = serde_json::from_value(json_data).unwrap();

        assert_eq!(priority.id, "3");
        assert_eq!(priority.name, "Medium");
        assert_eq!(priority.status_color, Some("#EA7D24".to_string()));
    }
}
