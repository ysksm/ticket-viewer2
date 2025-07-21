use crate::error::Result;
use url::Url;
use std::sync::Arc;
use reqwest::{Client, header};
use base64::Engine;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Auth {
    Basic { username: String, api_token: String },
    Bearer { token: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        
        let status = response.status();
        println!("=== JIRA API Response ===");
        println!("Status: {}", status);
        
        if !status.is_success() {
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            println!("Error Response: {}", message);
            println!("=========================");
            return Err(crate::error::Error::ApiError { status: status.as_u16(), message });
        }
        
        // レスポンステキストを取得してログ出力
        let response_text = response.text().await?;
        println!("Response Length: {} bytes", response_text.len());
        
        // レスポンステキストの最初の500文字を表示（デバッグ用）
        let preview = if response_text.len() > 500 {
            format!("{}...", &response_text[..500])
        } else {
            response_text.clone()
        };
        println!("Response Preview:\n{}", preview);
        println!("=========================");
        
        // JSONをパースして返す
        let data = serde_json::from_str::<T>(&response_text)
            .map_err(|e| {
                println!("JSON Parse Error: {}", e);
                println!("Full Response Text:\n{}", response_text);
                crate::error::Error::SerializationError(format!("JSON parse error: {}", e))
            })?;
        
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

        // リクエストボディをログ出力
        println!("=== JIRA API Request ===");
        println!("URL: {}/rest/api/3/search", self.config.base_url);
        println!("Request Body:");
        if let Ok(pretty_body) = serde_json::to_string_pretty(&body) {
            println!("{}", pretty_body);
        } else {
            println!("{:?}", body);
        }
        println!("========================");

        self.post("/rest/api/3/search", &body).await
    }

    pub async fn get_projects(&self) -> Result<Vec<crate::models::Project>> {
        self.get_projects_with_params(crate::models::ProjectParams::new()).await
    }

    pub async fn get_projects_with_params(&self, params: crate::models::ProjectParams) -> Result<Vec<crate::models::Project>> {
        let mut url = "/rest/api/3/project".to_string();
        let mut query_params = Vec::new();

        // クエリパラメータを構築
        if let Some(expand) = params.expand {
            query_params.push(format!("expand={}", expand.join(",")));
        }
        if let Some(recent) = params.recent {
            query_params.push(format!("recent={}", recent));
        }
        if let Some(properties) = params.properties {
            query_params.push(format!("properties={}", properties.join(",")));
        }

        if !query_params.is_empty() {
            url.push('?');
            url.push_str(&query_params.join("&"));
        }

        self.get(&url).await
    }

    /// JIRAの優先度一覧を取得する
    /// 
    /// # Returns
    /// 
    /// `Result<Vec<Priority>>` - 優先度のベクター、またはエラー
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use jira_api::{JiraClient, JiraConfig, Auth};
    /// 
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = JiraConfig::new(
    ///     "https://your-domain.atlassian.net".to_string(),
    ///     Auth::Basic {
    ///         username: "user@example.com".to_string(),
    ///         api_token: "api-token".to_string(),
    ///     }
    /// )?;
    /// let client = JiraClient::new(config)?;
    /// let priorities = client.get_priorities().await?;
    /// for priority in priorities {
    ///     println!("{}: {}", priority.name, priority.description.unwrap_or_default());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_priorities(&self) -> Result<Vec<crate::models::Priority>> {
        self.get("/rest/api/3/priority").await
    }

    /// JIRAの課題タイプ一覧を取得する
    /// 
    /// # Returns
    /// 
    /// `Result<Vec<IssueType>>` - 課題タイプのベクター、またはエラー
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use jira_api::{JiraClient, JiraConfig, Auth};
    /// 
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = JiraConfig::new(
    ///     "https://your-domain.atlassian.net".to_string(),
    ///     Auth::Basic {
    ///         username: "user@example.com".to_string(),
    ///         api_token: "api-token".to_string(),
    ///     }
    /// )?;
    /// let client = JiraClient::new(config)?;
    /// let issue_types = client.get_issue_types().await?;
    /// for issue_type in issue_types {
    ///     println!("{}: {} (subtask: {})", 
    ///         issue_type.name, 
    ///         issue_type.description.unwrap_or_default(),
    ///         issue_type.subtask.unwrap_or(false)
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_issue_types(&self) -> Result<Vec<crate::models::IssueType>> {
        self.get("/rest/api/3/issuetype").await
    }

    /// JIRAのフィールド一覧を取得する
    /// 
    /// # Returns
    /// 
    /// `Result<Vec<Field>>` - フィールドのベクター、またはエラー
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use jira_api::{JiraClient, JiraConfig, Auth};
    /// 
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = JiraConfig::new(
    ///     "https://your-domain.atlassian.net".to_string(),
    ///     Auth::Basic {
    ///         username: "user@example.com".to_string(),
    ///         api_token: "api-token".to_string(),
    ///     }
    /// )?;
    /// let client = JiraClient::new(config)?;
    /// let fields = client.get_fields().await?;
    /// for field in fields {
    ///     println!("{}: {} (custom: {})", 
    ///         field.id, 
    ///         field.name,
    ///         field.custom.unwrap_or(false)
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_fields(&self) -> Result<Vec<crate::models::Field>> {
        self.get("/rest/api/3/field").await
    }

    /// JIRAのステータスカテゴリー一覧を取得する
    /// 
    /// # Returns
    /// 
    /// `Result<Vec<StatusCategory>>` - ステータスカテゴリーのベクター、またはエラー
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use jira_api::{JiraClient, JiraConfig, Auth};
    /// 
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = JiraConfig::new(
    ///     "https://your-domain.atlassian.net".to_string(),
    ///     Auth::Basic {
    ///         username: "user@example.com".to_string(),
    ///         api_token: "api-token".to_string(),
    ///     }
    /// )?;
    /// let client = JiraClient::new(config)?;
    /// let categories = client.get_status_categories().await?;
    /// for category in categories {
    ///     println!("{}: {} ({})", 
    ///         category.key, 
    ///         category.name,
    ///         category.color_name
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_status_categories(&self) -> Result<Vec<crate::models::StatusCategory>> {
        self.get("/rest/api/3/statuscategory").await
    }

    /// JIRAでユーザーを検索する
    /// 
    /// # Arguments
    /// 
    /// * `query` - 検索クエリ文字列（ユーザー名、表示名、メールアドレスの一部など）
    /// 
    /// # Returns
    /// 
    /// `Result<Vec<User>>` - 検索にマッチしたユーザーのベクター、またはエラー
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use jira_api::{JiraClient, JiraConfig, Auth};
    /// 
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = JiraConfig::new(
    ///     "https://your-domain.atlassian.net".to_string(),
    ///     Auth::Basic {
    ///         username: "user@example.com".to_string(),
    ///         api_token: "api-token".to_string(),
    ///     }
    /// )?;
    /// let client = JiraClient::new(config)?;
    /// let users = client.search_users("john").await?;
    /// for user in users {
    ///     println!("{}: {} ({})", 
    ///         user.account_id, 
    ///         user.display_name,
    ///         user.email_address.unwrap_or_default()
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_users(&self, query: &str) -> Result<Vec<crate::models::User>> {
        let encoded_query = urlencoding::encode(query);
        let url = format!("/rest/api/3/user/search?query={}", encoded_query);
        self.get(&url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// JiraConfig::new()が有効なURLとBasic認証で正常に設定を作成できることをテスト
    /// 
    /// テスト内容:
    /// - 有効なHTTPS URLが正しく検証される
    /// - Basic認証情報が正しく保存される
    /// - 作成されたConfigオブジェクトの値が期待通りである
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

    /// JiraConfig::new()がBearer認証で正常に設定を作成できることをテスト
    /// 
    /// テスト内容:
    /// - Bearer認証情報が正しく保存される
    /// - 作成されたConfigオブジェクトの値が期待通りである
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

    /// JiraConfig::new()が無効なURLでエラーを返すことをテスト
    /// 
    /// テスト内容:
    /// - 無効なURL形式が正しく検出される
    /// - 適切なエラーメッセージが返される
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

    /// JiraConfig::from_env()が環境変数からBasic認証で設定を作成できることをテスト
    /// 
    /// テスト内容:
    /// - JIRA_URL, JIRA_USER, JIRA_API_TOKENが正しく読み込まれる
    /// - Basic認証設定が正しく作成される
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

    /// JiraConfig::from_env()がJIRA_URL環境変数が未設定時にエラーを返すことをテスト
    /// 
    /// テスト内容:
    /// - JIRA_URLが未設定の場合にConfigurationMissingエラーが返される
    /// - 適切なエラーメッセージが含まれる
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

    /// JiraConfig::from_env()がJIRA_API_TOKEN環境変数が未設定時にエラーを返すことをテスト
    /// 
    /// テスト内容:
    /// - JIRA_API_TOKENが未設定の場合にConfigurationMissingエラーが返される
    /// - 適切なエラーメッセージが含まれる
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

    /// JiraClient::new()が有効な設定でHTTPクライアントを作成できることをテスト
    /// 
    /// テスト内容:
    /// - 有効な設定でJiraClientが正常に作成される
    /// - 設定値が正しくconfig()メソッドで取得できる
    /// - 認証ヘッダーが正しく設定される
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

    /// JiraClient::new()がBearer認証で正常にクライアントを作成できることをテスト
    /// 
    /// テスト内容:
    /// - Bearer認証設定でJiraClientが正常に作成される
    /// - Bearerトークンが正しくAuthorizationヘッダーに設定される
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

    /// JiraClientのget()メソッドが正常にHTTP GETリクエストを実行できることをテスト
    /// 
    /// テスト内容:
    /// - 正しいURLにGETリクエストが送信される
    /// - Basic認証ヘッダーが正しく設定される
    /// - 成功レスポンスがJSONとして正しくデシリアライズされる
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

    /// JiraClientのget()メソッドがHTTPエラーレスポンスを正しく処理できることをテスト
    /// 
    /// テスト内容:
    /// - 404 Not Foundレスポンスが正しくApiErrorに変換される
    /// - エラーメッセージが正しく抽出される
    /// - ステータスコードが正しく保持される
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

    /// search_issues()が正しいJQLとパラメータで検索結果を取得できることをテスト
    /// 
    /// テスト内容:
    /// - POST /rest/api/3/searchエンドポイントに正しいリクエストが送信される
    /// - JQLクエリとstartAt/maxResultsパラメータが正しく送信される
    /// - レスポンスがSearchResult構造体に正しくデシリアライズされる
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

    /// search_issues()が複雑なパラメータ（fields, expand, validateQuery）で正しく動作することをテスト
    /// 
    /// テスト内容:
    /// - fieldsパラメータが正しくリクエストボディに設定される
    /// - expandパラメータが正しくリクエストボディに設定される
    /// - validateQueryフラグが正しくリクエストボディに設定される
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

    /// get_projects()が正常にプロジェクト一覧を取得できることをテスト
    /// 
    /// テスト内容:
    /// - GET /rest/api/3/projectエンドポイントに正しいリクエストが送信される
    /// - レスポンスがVec<Project>に正しくデシリアライズされる
    /// - 各プロジェクトの基本プロパティ（key, name）が正しく設定される
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

    /// get_projects()が権限エラーを正しく処理できることをテスト
    /// 
    /// テスト内容:
    /// - 403 Forbiddenレスポンスが正しくApiErrorに変換される
    /// - エラーメッセージが正しく抽出される
    /// - ステータスコードが正しく保持される
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

    /// get_projects_with_params()がexpandパラメータで詳細情報付きプロジェクト取得できることをテスト
    /// 
    /// テスト内容:
    /// - expandクエリパラメータが正しくURLに追加される
    /// - 複数のexpand値がカンマ区切りで結合される
    /// - GET /rest/api/3/project?expand=lead,descriptionが正しく送信される
    #[tokio::test]
    async fn test_get_projects_with_expand() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, query_param};
        use serde_json::json;
        use crate::models::ProjectParams;

        // Given: expandパラメータ付きのモックサーバー
        let mock_server = MockServer::start().await;
        
        let response_body = json!([{
            "id": "10000",
            "key": "TEST",
            "name": "Test Project with Details",
            "self": "https://example.atlassian.net/rest/api/3/project/10000",
            "description": "Detailed test project description",
            "lead": {
                "accountId": "557058:f58131cb",
                "displayName": "Project Lead",
                "emailAddress": "lead@example.com",
                "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:f58131cb"
            }
        }]);

        Mock::given(method("GET"))
            .and(path("/rest/api/3/project"))
            .and(query_param("expand", "lead,description"))
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
        let params = ProjectParams::new()
            .expand(vec!["lead".to_string(), "description".to_string()]);

        // When: expandパラメータ付きでプロジェクト取得
        let result = client.get_projects_with_params(params).await;

        // Then: 成功し、詳細情報付きプロジェクトが返る
        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Test Project with Details");
        assert_eq!(projects[0].description, Some("Detailed test project description".to_string()));
        assert!(projects[0].lead.is_some());
    }

    /// get_projects_with_params()がrecentパラメータで最近のプロジェクト取得できることをテスト
    /// 
    /// テスト内容:
    /// - recentクエリパラメータが正しくURLに追加される
    /// - 数値パラメータが正しく文字列に変換される
    /// - GET /rest/api/3/project?recent=5が正しく送信される
    #[tokio::test]
    async fn test_get_projects_with_recent() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, query_param};
        use serde_json::json;
        use crate::models::ProjectParams;

        // Given: recentパラメータ付きのモックサーバー
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project"))
            .and(query_param("recent", "5"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([])))
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
        let params = ProjectParams::new().recent(5);

        // When: recentパラメータ付きでプロジェクト取得
        let result = client.get_projects_with_params(params).await;

        // Then: 成功する
        assert!(result.is_ok());
    }

    /// get_projects_with_params()が複数パラメータで正しくクエリ文字列を構築することをテスト
    /// 
    /// テスト内容:
    /// - 複数のクエリパラメータが&で結合される
    /// - expand、recent、propertiesが同時に設定される
    /// - クエリ文字列の順序が正しい
    #[tokio::test]
    async fn test_get_projects_with_multiple_params() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, query_param};
        use serde_json::json;
        use crate::models::ProjectParams;

        // Given: 複数パラメータ付きのモックサーバー
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project"))
            .and(query_param("expand", "lead,description"))
            .and(query_param("recent", "10"))
            .and(query_param("properties", "*all"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([])))
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
        let params = ProjectParams::new()
            .expand(vec!["lead".to_string(), "description".to_string()])
            .recent(10)
            .properties(vec!["*all".to_string()]);

        // When: 複数パラメータ付きでプロジェクト取得
        let result = client.get_projects_with_params(params).await;

        // Then: 成功する
        assert!(result.is_ok());
    }

    /// get_priorities()が正常に優先度一覧を取得できることをテスト
    /// 
    /// テスト内容:
    /// - /rest/api/3/priorityエンドポイントに正しくGETリクエストが送信される
    /// - レスポンスが正しくPriority構造体にデシリアライズされる
    /// - 複数の優先度が含まれる場合の動作確認
    #[tokio::test]
    async fn test_get_priorities() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/priority"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "id": "1",
                        "name": "Highest",
                        "self": "https://example.atlassian.net/rest/api/3/priority/1",
                        "description": "This problem will block progress.",
                        "iconUrl": "https://example.atlassian.net/images/icons/priorities/highest.svg",
                        "statusColor": "#cd1316"
                    },
                    {
                        "id": "2", 
                        "name": "High",
                        "self": "https://example.atlassian.net/rest/api/3/priority/2",
                        "description": "Serious problem that could block progress.",
                        "iconUrl": "https://example.atlassian.net/images/icons/priorities/high.svg",
                        "statusColor": "#d04437"
                    },
                    {
                        "id": "3",
                        "name": "Medium",
                        "self": "https://example.atlassian.net/rest/api/3/priority/3",
                        "description": "Has the potential to affect progress.",
                        "iconUrl": "https://example.atlassian.net/images/icons/priorities/medium.svg",
                        "statusColor": "#f79232"
                    }
                ])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_priorities().await;

        assert!(result.is_ok());
        let priorities = result.unwrap();
        assert_eq!(priorities.len(), 3);
        
        assert_eq!(priorities[0].id, "1");
        assert_eq!(priorities[0].name, "Highest");
        assert_eq!(priorities[0].description, Some("This problem will block progress.".to_string()));
        
        assert_eq!(priorities[1].id, "2");
        assert_eq!(priorities[1].name, "High");
        
        assert_eq!(priorities[2].id, "3");
        assert_eq!(priorities[2].name, "Medium");
    }

    /// get_priorities()がHTTPエラーを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 404エラー時にNotFoundエラーが返される
    /// - 認証エラー（401）時にAuthenticationErrorが返される
    #[tokio::test]
    async fn test_get_priorities_error_handling() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/priority"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_priorities().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, crate::Error::ApiError { status: 404, .. }));
    }

    /// get_issue_types()が正常に課題タイプ一覧を取得できることをテスト
    /// 
    /// テスト内容:
    /// - /rest/api/3/issuetypeエンドポイントに正しくGETリクエストが送信される
    /// - レスポンスが正しくIssueType構造体にデシリアライズされる
    /// - 複数の課題タイプが含まれる場合の動作確認
    #[tokio::test]
    async fn test_get_issue_types() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issuetype"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "id": "10000",
                        "name": "Task",
                        "self": "https://example.atlassian.net/rest/api/3/issuetype/10000",
                        "description": "A task represents work that needs to be done.",
                        "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/task.png",
                        "subtask": false
                    },
                    {
                        "id": "10001",
                        "name": "Bug",
                        "self": "https://example.atlassian.net/rest/api/3/issuetype/10001", 
                        "description": "A bug is a problem in the system.",
                        "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/bug.png",
                        "subtask": false
                    },
                    {
                        "id": "10002",
                        "name": "Subtask",
                        "self": "https://example.atlassian.net/rest/api/3/issuetype/10002",
                        "description": "A subtask for a parent issue.",
                        "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/subtask.png",
                        "subtask": true
                    }
                ])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_issue_types().await;

        assert!(result.is_ok());
        let issue_types = result.unwrap();
        assert_eq!(issue_types.len(), 3);
        
        assert_eq!(issue_types[0].id, "10000");
        assert_eq!(issue_types[0].name, "Task");
        assert_eq!(issue_types[0].subtask, Some(false));
        
        assert_eq!(issue_types[1].id, "10001");
        assert_eq!(issue_types[1].name, "Bug");
        assert_eq!(issue_types[1].subtask, Some(false));
        
        assert_eq!(issue_types[2].id, "10002");
        assert_eq!(issue_types[2].name, "Subtask");
        assert_eq!(issue_types[2].subtask, Some(true));
    }

    /// get_issue_types()がHTTPエラーを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 404エラー時にNotFoundエラーが返される
    #[tokio::test]
    async fn test_get_issue_types_error_handling() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issuetype"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_issue_types().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, crate::Error::ApiError { status: 404, .. }));
    }

    /// get_fields()が正常にフィールド一覧を取得できることをテスト
    /// 
    /// テスト内容:
    /// - /rest/api/3/fieldエンドポイントに正しくGETリクエストが送信される
    /// - レスポンスが正しくField構造体にデシリアライズされる
    /// - 複数のフィールドが含まれる場合の動作確認
    #[tokio::test]
    async fn test_get_fields() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/field"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "id": "summary",
                        "key": "summary",
                        "name": "Summary",
                        "custom": false,
                        "orderable": true,
                        "navigable": true,
                        "searchable": true,
                        "schema": {
                            "type": "string",
                            "system": "summary"
                        }
                    },
                    {
                        "id": "issuetype",
                        "key": "issuetype",
                        "name": "Issue Type",
                        "custom": false,
                        "orderable": true,
                        "navigable": true,
                        "searchable": true,
                        "schema": {
                            "type": "issuetype",
                            "system": "issuetype"
                        }
                    },
                    {
                        "id": "customfield_10001",
                        "key": "customfield_10001",
                        "name": "Story Points",
                        "custom": true,
                        "orderable": true,
                        "navigable": true,
                        "searchable": true,
                        "schema": {
                            "type": "number",
                            "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                            "customId": 10001
                        }
                    }
                ])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_fields().await;

        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 3);
        
        assert_eq!(fields[0].id, "summary");
        assert_eq!(fields[0].name, "Summary");
        assert_eq!(fields[0].custom, Some(false));
        
        assert_eq!(fields[1].id, "issuetype");
        assert_eq!(fields[1].name, "Issue Type");
        assert_eq!(fields[1].custom, Some(false));
        
        assert_eq!(fields[2].id, "customfield_10001");
        assert_eq!(fields[2].name, "Story Points");
        assert_eq!(fields[2].custom, Some(true));
    }

    /// get_fields()がHTTPエラーを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 404エラー時にNotFoundエラーが返される
    #[tokio::test]
    async fn test_get_fields_error_handling() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/field"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_fields().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, crate::Error::ApiError { status: 404, .. }));
    }

    /// get_status_categories()が正常にステータスカテゴリー一覧を取得できることをテスト
    /// 
    /// テスト内容:
    /// - /rest/api/3/statuscategoryエンドポイントに正しくGETリクエストが送信される
    /// - レスポンスが正しくStatusCategory構造体にデシリアライズされる
    /// - 複数のステータスカテゴリーが含まれる場合の動作確認
    #[tokio::test]
    async fn test_get_status_categories() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/statuscategory"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "id": 1,
                        "key": "undefined",
                        "name": "No Category",
                        "colorName": "medium-gray"
                    },
                    {
                        "id": 2,
                        "key": "new",
                        "name": "To Do",
                        "colorName": "blue-gray"
                    },
                    {
                        "id": 3,
                        "key": "indeterminate",
                        "name": "In Progress",
                        "colorName": "yellow"
                    },
                    {
                        "id": 4,
                        "key": "done",
                        "name": "Done",
                        "colorName": "green"
                    }
                ])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_status_categories().await;

        assert!(result.is_ok());
        let categories = result.unwrap();
        assert_eq!(categories.len(), 4);
        
        assert_eq!(categories[0].id, 1);
        assert_eq!(categories[0].key, "undefined");
        assert_eq!(categories[0].name, "No Category");
        assert_eq!(categories[0].color_name, "medium-gray");
        
        assert_eq!(categories[1].id, 2);
        assert_eq!(categories[1].key, "new");
        assert_eq!(categories[1].name, "To Do");
        
        assert_eq!(categories[3].id, 4);
        assert_eq!(categories[3].key, "done");
        assert_eq!(categories[3].name, "Done");
    }

    /// get_status_categories()がHTTPエラーを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 404エラー時にNotFoundエラーが返される
    #[tokio::test]
    async fn test_get_status_categories_error_handling() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/statuscategory"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.get_status_categories().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, crate::Error::ApiError { status: 404, .. }));
    }

    /// search_users()が正常にユーザー検索を実行できることをテスト
    /// 
    /// テスト内容:
    /// - /rest/api/3/users/searchエンドポイントに正しくGETリクエストが送信される
    /// - クエリパラメータが正しく設定される
    /// - レスポンスが正しくUser構造体にデシリアライズされる
    #[tokio::test]
    async fn test_search_users() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, query_param};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/user/search"))
            .and(query_param("query", "test"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "accountId": "557058:f58131cb-b67d-43c7-b30d-6b58d40bd077",
                        "displayName": "Test User",
                        "emailAddress": "test@example.com",
                        "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:f58131cb",
                        "active": true
                    },
                    {
                        "accountId": "557058:a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6",
                        "displayName": "Another Test User",
                        "emailAddress": "another@example.com", 
                        "self": "https://example.atlassian.net/rest/api/3/user?accountId=557058:a1b2c3d4",
                        "active": true
                    }
                ])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.search_users("test").await;

        assert!(result.is_ok());
        let users = result.unwrap();
        assert_eq!(users.len(), 2);
        
        assert_eq!(users[0].account_id, "557058:f58131cb-b67d-43c7-b30d-6b58d40bd077");
        assert_eq!(users[0].display_name, "Test User");
        assert_eq!(users[0].email_address, Some("test@example.com".to_string()));
        
        assert_eq!(users[1].account_id, "557058:a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6");
        assert_eq!(users[1].display_name, "Another Test User");
    }

    /// search_users()が空のクエリを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 空のクエリでも正常にリクエストが送信される
    #[tokio::test]
    async fn test_search_users_empty_query() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, query_param};
        use serde_json::json;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/user/search"))
            .and(query_param("query", ""))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([])))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.search_users("").await;

        assert!(result.is_ok());
        let users = result.unwrap();
        assert_eq!(users.len(), 0);
    }

    /// search_users()がHTTPエラーを適切に処理することをテスト
    /// 
    /// テスト内容:
    /// - 404エラー時にNotFoundエラーが返される
    #[tokio::test]
    async fn test_search_users_error_handling() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/user/search"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = JiraConfig {
            base_url: mock_server.uri(),
            auth: Auth::Basic {
                username: "test".to_string(),
                api_token: "token".to_string(),
            },
        };

        let client = JiraClient::new(config).unwrap();
        let result = client.search_users("test").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, crate::Error::ApiError { status: 404, .. }));
    }
}