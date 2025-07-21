use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "emailAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_address: Option<String>,
    #[serde(rename = "self")]
    pub self_url: String,
    #[serde(rename = "avatarUrls")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_urls: Option<AvatarUrls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(rename = "timeZone")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(rename = "accountType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarUrls {
    #[serde(rename = "48x48")]
    pub size_48: String,
    #[serde(rename = "24x24")]
    pub size_24: String,
    #[serde(rename = "16x16")]
    pub size_16: String,
    #[serde(rename = "32x32")]
    pub size_32: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_user_deserialization() {
        let json_data = json!({
            "accountId": "557058:f58131cb-b67d-43c7-b30d-6b58d40bd077",
            "displayName": "Test User",
            "emailAddress": "test@example.com",
            "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:f58131cb",
            "avatarUrls": {
                "48x48": "https://avatar.example.com/48.png",
                "24x24": "https://avatar.example.com/24.png",
                "16x16": "https://avatar.example.com/16.png",
                "32x32": "https://avatar.example.com/32.png"
            },
            "active": true,
            "timeZone": "America/Los_Angeles",
            "accountType": "atlassian"
        });

        let user: User = serde_json::from_value(json_data).unwrap();
        
        assert_eq!(user.account_id, "557058:f58131cb-b67d-43c7-b30d-6b58d40bd077");
        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.email_address, Some("test@example.com".to_string()));
        assert_eq!(user.active, Some(true));
    }
}