use super::Issue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchParams {
    #[serde(rename = "startAt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<u32>,

    #[serde(rename = "maxResults")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<Vec<String>>,

    #[serde(rename = "validateQuery")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate_query: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "startAt")]
    pub start_at: u32,

    #[serde(rename = "maxResults")]
    pub max_results: u32,

    pub total: u32,

    pub issues: Vec<Issue>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
}

impl SearchParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_at(mut self, start_at: u32) -> Self {
        self.start_at = Some(start_at);
        self
    }

    pub fn max_results(mut self, max_results: u32) -> Self {
        self.max_results = Some(max_results);
        self
    }

    pub fn fields(mut self, fields: Vec<String>) -> Self {
        self.fields = Some(fields);
        self
    }

    pub fn expand(mut self, expand: Vec<String>) -> Self {
        self.expand = Some(expand);
        self
    }

    pub fn validate_query(mut self, validate: bool) -> Self {
        self.validate_query = Some(validate);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_search_params_builder() {
        let params = SearchParams::new()
            .start_at(0)
            .max_results(50)
            .fields(vec!["summary".to_string(), "status".to_string()])
            .expand(vec!["changelog".to_string()])
            .validate_query(true);

        assert_eq!(params.start_at, Some(0));
        assert_eq!(params.max_results, Some(50));
        assert!(params.fields.is_some());
        assert!(params.expand.is_some());
        assert_eq!(params.validate_query, Some(true));
    }

    #[test]
    fn test_search_params_serialization() {
        let params = SearchParams::new().start_at(10).max_results(25);

        let json = serde_json::to_value(&params).unwrap();

        assert_eq!(json["startAt"], 10);
        assert_eq!(json["maxResults"], 25);
        assert!(json.get("fields").is_none()); // None values should be omitted
    }

    #[test]
    fn test_search_result_deserialization() {
        let json_data = json!({
            "startAt": 0,
            "maxResults": 50,
            "total": 123,
            "issues": [
                {
                    "id": "10000",
                    "key": "TEST-1",
                    "self": "https://example.atlassian.net/rest/api/3/issue/10000",
                    "fields": {
                        "summary": "Test Issue",
                        "issuetype": {
                            "id": "1",
                            "name": "Bug",
                            "self": "https://example.atlassian.net/rest/api/3/issuetype/1"
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
                        "reporter": {
                            "accountId": "557058:f58131cb",
                            "displayName": "Test User",
                            "emailAddress": "test@example.com",
                            "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:f58131cb"
                        },
                        "created": "2024-01-01T00:00:00.000Z",
                        "updated": "2024-01-02T00:00:00.000Z"
                    }
                }
            ]
        });

        let result: SearchResult = serde_json::from_value(json_data).unwrap();

        assert_eq!(result.start_at, 0);
        assert_eq!(result.max_results, 50);
        assert_eq!(result.total, 123);
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].key, "TEST-1");
    }
}
