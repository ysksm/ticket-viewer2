use crate::error::Result;
use url::Url;
use std::sync::Arc;
use reqwest::{Client, header};
use base64::Engine;

#[derive(Debug, Clone)]
pub enum Auth {
    Basic { username: String, api_token: String },
    Bearer { token: String },
}

#[derive(Debug, Clone)]
pub struct JiraConfig {
    pub base_url: String,
    pub auth: Auth,
}

impl JiraConfig {
    pub fn new(base_url: impl Into<String>, auth: Auth) -> Result<Self> {
        let base_url = base_url.into();
        
        // Validate URL
        let _ = Url::parse(&base_url)
            .map_err(|_| crate::error::Error::InvalidConfiguration("Invalid base URL".to_string()))?;
        
        Ok(Self {
            base_url,
            auth,
        })
    }

    pub fn from_env() -> Result<Self> {
        use std::env;
        
        let base_url = env::var("JIRA_URL")
            .map_err(|_| crate::error::Error::ConfigurationMissing("JIRA_URL not found in environment".to_string()))?;
        
        let username = env::var("JIRA_USER")
            .map_err(|_| crate::error::Error::ConfigurationMissing("JIRA_USER not found in environment".to_string()))?;
        
        let api_token = env::var("JIRA_API_TOKEN")
            .map_err(|_| crate::error::Error::ConfigurationMissing("JIRA_API_TOKEN not found in environment".to_string()))?;
        
        let auth = Auth::Basic { username, api_token };
        
        Self::new(base_url, auth)
    }
}

#[derive(Debug, Clone)]
pub struct JiraClient {
    pub(crate) client: Client,
    pub(crate) config: Arc<JiraConfig>,
}

impl JiraClient {
    pub fn new(config: JiraConfig) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );

        // 認証ヘッダーを追加
        match &config.auth {
            Auth::Basic { username, api_token } => {
                let auth_value = format!("{}:{}", username, api_token);
                let encoded = base64::engine::general_purpose::STANDARD.encode(auth_value.as_bytes());
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Basic {}", encoded))
                        .map_err(|_| crate::error::Error::InvalidConfiguration("Invalid auth header".to_string()))?,
                );
            }
            Auth::Bearer { token } => {
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {}", token))
                        .map_err(|_| crate::error::Error::InvalidConfiguration("Invalid auth header".to_string()))?,
                );
            }
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| crate::error::Error::Unexpected(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    pub fn config(&self) -> &JiraConfig {
        &self.config
    }

    pub(crate) async fn get<T>(&self, endpoint: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.config.base_url, endpoint);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::error::Error::ApiError { status, message });
        }
        
        let data = response.json::<T>().await?;
        Ok(data)
    }

    pub(crate) async fn post<T, B>(&self, endpoint: &str, body: &B) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        let url = format!("{}{}", self.config.base_url, endpoint);
        
        let response = self.client
            .post(&url)
            .json(body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::error::Error::ApiError { status, message });
        }
        
        let data = response.json::<T>().await?;
        Ok(data)
    }

    pub async fn search_issues(&self, jql: &str, params: crate::models::SearchParams) -> Result<crate::models::SearchResult> {
        let mut body = serde_json::json!({
            "jql": jql
        });
        
        // SearchParamsの値をリクエストボディにマージ
        if let Some(start_at) = params.start_at {
            body["startAt"] = start_at.into();
        }
        if let Some(max_results) = params.max_results {
            body["maxResults"] = max_results.into();
        }
        if let Some(fields) = params.fields {
            body["fields"] = fields.into();
        }
        if let Some(expand) = params.expand {
            body["expand"] = expand.into();
        }
        if let Some(validate_query) = params.validate_query {
            body["validateQuery"] = validate_query.into();
        }

        self.post("/rest/api/3/search", &body).await
    }

    pub async fn get_projects(&self) -> Result<Vec<crate::models::Project>> {
        self.get("/rest/api/3/project").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jira_config_new_with_valid_url() {
        // Given: 有効なURLとBasic認証情報
        let base_url = "https://example.atlassian.net";
        let auth = Auth::Basic {
            username: "test@example.com".to_string(),
            api_token: "test_token".to_string(),
        };

        // When: JiraConfigを作成
        let result = JiraConfig::new(base_url, auth.clone());

        // Then: 成功し、正しい値が設定される
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.base_url, base_url);
        match config.auth {
            Auth::Basic { username, api_token } => {
                assert_eq!(username, "test@example.com");
                assert_eq!(api_token, "test_token");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn test_jira_config_new_with_bearer_auth() {
        // Given: 有効なURLとBearer認証情報
        let base_url = "https://example.atlassian.net";
        let auth = Auth::Bearer {
            token: "bearer_token_123".to_string(),
        };

        // When: JiraConfigを作成
        let result = JiraConfig::new(base_url, auth);

        // Then: 成功し、正しい値が設定される
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.base_url, base_url);
        match config.auth {
            Auth::Bearer { token } => {
                assert_eq!(token, "bearer_token_123");
            }
            _ => panic!("Expected Bearer auth"),
        }
    }

    #[test]
    fn test_jira_config_new_with_invalid_url() {
        // Given: 無効なURL
        let base_url = "not a valid url";
        let auth = Auth::Basic {
            username: "test@example.com".to_string(),
            api_token: "test_token".to_string(),
        };

        // When: JiraConfigを作成
        let result = JiraConfig::new(base_url, auth);

        // Then: エラーが返される
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::Error::InvalidConfiguration(msg) => {
                assert_eq!(msg, "Invalid base URL");
            }
            _ => panic!("Expected InvalidConfiguration error"),
        }
    }

    #[test]
    fn test_jira_config_from_env_with_basic_auth() {
        // Given: 環境変数を設定
        unsafe {
            std::env::set_var("JIRA_URL", "https://test.atlassian.net");
            std::env::set_var("JIRA_USER", "test@example.com");
            std::env::set_var("JIRA_API_TOKEN", "test_api_token");
        }

        // When: from_env()を呼び出す
        let result = JiraConfig::from_env();

        // Then: 成功し、正しい値が設定される
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.base_url, "https://test.atlassian.net");
        match config.auth {
            Auth::Basic { username, api_token } => {
                assert_eq!(username, "test@example.com");
                assert_eq!(api_token, "test_api_token");
            }
            _ => panic!("Expected Basic auth"),
        }

        // Cleanup
        unsafe {
            std::env::remove_var("JIRA_URL");
            std::env::remove_var("JIRA_USER");
            std::env::remove_var("JIRA_API_TOKEN");
        }
    }

    #[test]
    fn test_jira_config_from_env_missing_url() {
        // Given: JIRA_URLが設定されていない
        unsafe {
            std::env::remove_var("JIRA_URL");
            std::env::set_var("JIRA_USER", "test@example.com");
            std::env::set_var("JIRA_API_TOKEN", "test_api_token");
        }

        // When: from_env()を呼び出す
        let result = JiraConfig::from_env();

        // Then: エラーが返される
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::Error::ConfigurationMissing(msg) => {
                assert!(msg.contains("JIRA_URL"));
            }
            _ => panic!("Expected ConfigurationMissing error"),
        }

        // Cleanup
        unsafe {
            std::env::remove_var("JIRA_USER");
            std::env::remove_var("JIRA_API_TOKEN");
        }
    }

    #[test]
    fn test_jira_config_from_env_missing_auth() {
        // Given: 認証情報が不完全（まず全部クリアしてから設定）
        unsafe {
            std::env::remove_var("JIRA_URL");
            std::env::remove_var("JIRA_USER");
            std::env::remove_var("JIRA_API_TOKEN");
            
            std::env::set_var("JIRA_URL", "https://test.atlassian.net");
            std::env::set_var("JIRA_USER", "test@example.com");
            // JIRA_API_TOKENは設定しない
        }

        // When: from_env()を呼び出す
        let result = JiraConfig::from_env();

        // Then: エラーが返される
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::Error::ConfigurationMissing(msg) => {
                assert!(msg.contains("JIRA_API_TOKEN"));
            }
            _ => panic!("Expected ConfigurationMissing error"),
        }

        // Cleanup
        unsafe {
            std::env::remove_var("JIRA_URL");
            std::env::remove_var("JIRA_USER");
        }
    }

    #[test]
    fn test_jira_client_new() {
        // Given: 有効な設定
        let config = JiraConfig {
            base_url: "https://example.atlassian.net".to_string(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        // When: JiraClientを作成
        let result = JiraClient::new(config.clone());

        // Then: 成功し、正しい値が設定される
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.config().base_url, "https://example.atlassian.net");
    }

    #[test]
    fn test_jira_client_with_bearer_auth() {
        // Given: Bearer認証の設定
        let config = JiraConfig {
            base_url: "https://example.atlassian.net".to_string(),
            auth: Auth::Bearer {
                token: "bearer_token_123".to_string(),
            },
        };

        // When: JiraClientを作成
        let result = JiraClient::new(config);

        // Then: 成功する
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_request_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, header};
        use serde_json::json;

        // Given: モックサーバーを起動
        let mock_server = MockServer::start().await;
        
        // モックレスポンスを設定
        let response_body = json!({
            "id": "10000",
            "name": "Test Project"
        });
        
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/TEST"))
            .and(header("Authorization", "Basic dGVzdEBleGFtcGxlLmNvbTp0ZXN0X3Rva2Vu"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();

        // When: GETリクエストを送信
        let result: Result<serde_json::Value> = client.get("/rest/api/3/project/TEST").await;

        // Then: 成功し、正しいレスポンスが返る
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data["id"], "10000");
        assert_eq!(data["name"], "Test Project");
    }

    #[tokio::test]
    async fn test_get_request_error() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        // Given: エラーレスポンスを返すモックサーバー
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/TEST"))
            .respond_with(ResponseTemplate::new(404)
                .set_body_string("Project not found"))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();

        // When: GETリクエストを送信
        let result: Result<serde_json::Value> = client.get("/rest/api/3/project/TEST").await;

        // Then: エラーが返される
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::Error::ApiError { status, message } => {
                assert_eq!(status, 404);
                assert_eq!(message, "Project not found");
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[tokio::test]
    async fn test_search_issues_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, body_json};
        use serde_json::json;
        use crate::models::SearchParams;

        // Given: モックサーバーを起動
        let mock_server = MockServer::start().await;
        
        let response_body = json!({
            "startAt": 0,
            "maxResults": 50,
            "total": 1,
            "issues": [{
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
            }]
        });

        Mock::given(method("POST"))
            .and(path("/rest/api/3/search"))
            .and(body_json(json!({
                "jql": "project = TEST",
                "startAt": 0,
                "maxResults": 50
            })))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let params = SearchParams::new()
            .start_at(0)
            .max_results(50);

        // When: 検索を実行
        let result = client.search_issues("project = TEST", params).await;

        // Then: 成功し、正しい結果が返る
        assert!(result.is_ok());
        let search_result = result.unwrap();
        assert_eq!(search_result.start_at, 0);
        assert_eq!(search_result.max_results, 50);
        assert_eq!(search_result.total, 1);
        assert_eq!(search_result.issues.len(), 1);
        assert_eq!(search_result.issues[0].key, "TEST-1");
    }

    #[tokio::test]
    async fn test_search_issues_with_params() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, body_json};
        use serde_json::json;
        use crate::models::SearchParams;

        // Given: フィールド指定とexpandを含む検索
        let mock_server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .and(path("/rest/api/3/search"))
            .and(body_json(json!({
                "jql": "assignee = currentUser()",
                "fields": ["summary", "status", "assignee"],
                "expand": ["changelog"],
                "validateQuery": true
            })))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "startAt": 0,
                    "maxResults": 50,
                    "total": 0,
                    "issues": []
                })))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let params = SearchParams::new()
            .fields(vec!["summary".to_string(), "status".to_string(), "assignee".to_string()])
            .expand(vec!["changelog".to_string()])
            .validate_query(true);

        // When: 複雑なパラメータで検索を実行
        let result = client.search_issues("assignee = currentUser()", params).await;

        // Then: 成功する
        assert!(result.is_ok());
        let search_result = result.unwrap();
        assert_eq!(search_result.total, 0);
        assert_eq!(search_result.issues.len(), 0);
    }

    #[tokio::test]
    async fn test_get_projects_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use serde_json::json;

        // Given: プロジェクト一覧を返すモックサーバー
        let mock_server = MockServer::start().await;
        
        let response_body = json!([
            {
                "id": "10000",
                "key": "TEST",
                "name": "Test Project",
                "self": "https://example.atlassian.net/rest/api/3/project/10000",
                "description": "This is a test project",
                "projectTypeKey": "software",
                "simplified": false
            },
            {
                "id": "10001",
                "key": "DEMO",
                "name": "Demo Project",
                "self": "https://example.atlassian.net/rest/api/3/project/10001",
                "projectTypeKey": "business",
                "simplified": true
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/rest/api/3/project"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();

        // When: プロジェクト一覧を取得
        let result = client.get_projects().await;

        // Then: 成功し、プロジェクトリストが返る
        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].key, "TEST");
        assert_eq!(projects[0].name, "Test Project");
        assert_eq!(projects[1].key, "DEMO");
        assert_eq!(projects[1].name, "Demo Project");
    }

    #[tokio::test]
    async fn test_get_projects_error() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        // Given: エラーを返すモックサーバー
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project"))
            .respond_with(ResponseTemplate::new(403)
                .set_body_string("Insufficient permissions"))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test@example.com".to_string(),
                api_token: "test_token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();

        // When: プロジェクト一覧を取得
        let result = client.get_projects().await;

        // Then: エラーが返される
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::Error::ApiError { status, message } => {
                assert_eq!(status, 403);
                assert_eq!(message, "Insufficient permissions");
            }
            _ => panic!("Expected ApiError"),
        }
    }
}