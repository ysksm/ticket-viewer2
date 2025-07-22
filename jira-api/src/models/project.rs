use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(rename = "self")]
    pub self_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "projectTypeKey")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_type_key: Option<String>,
    #[serde(rename = "avatarUrls")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_urls: Option<ProjectAvatarUrls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simplified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAvatarUrls {
    #[serde(rename = "48x48")]
    pub size_48: String,
    #[serde(rename = "24x24")]
    pub size_24: String,
    #[serde(rename = "16x16")]
    pub size_16: String,
    #[serde(rename = "32x32")]
    pub size_32: String,
}

// Re-export dependent types
use super::User;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<String>>,
}

impl ProjectParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn expand(mut self, expand: Vec<String>) -> Self {
        self.expand = Some(expand);
        self
    }

    pub fn recent(mut self, count: u32) -> Self {
        self.recent = Some(count);
        self
    }

    pub fn properties(mut self, properties: Vec<String>) -> Self {
        self.properties = Some(properties);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_project_deserialization() {
        let json_data = json!({
            "id": "10000",
            "key": "TEST",
            "name": "Test Project",
            "self": "https://example.atlassian.net/rest/api/3/project/10000",
            "description": "This is a test project",
            "projectTypeKey": "software",
            "avatarUrls": {
                "48x48": "https://example.atlassian.net/secure/projectavatar?pid=10000&avatarId=10200&size=large",
                "24x24": "https://example.atlassian.net/secure/projectavatar?pid=10000&avatarId=10200&size=small",
                "16x16": "https://example.atlassian.net/secure/projectavatar?pid=10000&avatarId=10200&size=xsmall",
                "32x32": "https://example.atlassian.net/secure/projectavatar?pid=10000&avatarId=10200&size=medium"
            },
            "simplified": false
        });

        let project: Project = serde_json::from_value(json_data).unwrap();

        assert_eq!(project.id, "10000");
        assert_eq!(project.key, "TEST");
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.project_type_key, Some("software".to_string()));
    }

    #[test]
    fn test_project_params_builder() {
        let params = ProjectParams::new()
            .expand(vec!["lead".to_string(), "description".to_string()])
            .recent(10)
            .properties(vec!["*all".to_string()]);

        assert_eq!(
            params.expand,
            Some(vec!["lead".to_string(), "description".to_string()])
        );
        assert_eq!(params.recent, Some(10));
        assert_eq!(params.properties, Some(vec!["*all".to_string()]));
    }

    #[test]
    fn test_project_params_serialization() {
        let params = ProjectParams::new()
            .expand(vec!["lead".to_string()])
            .recent(5);

        let json = serde_json::to_value(&params).unwrap();

        assert_eq!(json["expand"], json!(["lead"]));
        assert_eq!(json["recent"], 5);
        assert!(json.get("properties").is_none()); // None values should be omitted
    }
}
