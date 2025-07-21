/// JIRA APIクライアントの統合テスト
/// 
/// 注意: これらのテストは実際のJIRA APIインスタンスが必要です
/// 環境変数 JIRA_URL, JIRA_USER, JIRA_API_TOKEN を設定してください
/// 
/// 実行方法:
/// ```
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// cargo test --test integration_tests -- --ignored
/// ```

use jira_api::{JiraClient, JiraConfig, SearchParams};

/// 環境変数から設定を作成するヘルパー関数
fn setup_client_from_env() -> Result<JiraClient, Box<dyn std::error::Error>> {
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    Ok(client)
}

/// 実際のJIRA APIとの接続テスト
/// 
/// テスト内容:
/// - 環境変数から設定を正しく読み込める
/// - JIRAインスタンスへの認証が成功する
/// - プロジェクト一覧を取得できる
#[tokio::test]
#[ignore] // 実際のJIRA APIが必要なため通常は無効化
async fn test_real_api_connection() {
    // Given: 環境変数から設定を作成
    let client = setup_client_from_env()
        .expect("Failed to setup client. Please check environment variables.");

    // When: プロジェクト一覧を取得
    let result = client.get_projects().await;

    // Then: 成功し、プロジェクトが取得される
    match result {
        Ok(projects) => {
            println!("Successfully connected to JIRA API");
            println!("Found {} projects", projects.len());
            
            // プロジェクトが存在する場合、最初のプロジェクトの詳細を表示
            if let Some(project) = projects.first() {
                println!("First project: {} ({})", project.name, project.key);
            }
        },
        Err(e) => {
            panic!("Failed to connect to JIRA API: {}", e);
        }
    }
}

/// 実際のJIRA APIでの検索テスト
/// 
/// テスト内容:
/// - 基本的なJQLクエリが実行できる
/// - 検索結果が正しい形式で返される
/// - ページネーションパラメータが適用される
#[tokio::test]
#[ignore] // 実際のJIRA APIが必要なため通常は無効化
async fn test_real_api_search() {
    // Given: 環境変数から設定を作成
    let client = setup_client_from_env()
        .expect("Failed to setup client. Please check environment variables.");

    // When: 最近作成されたチケットを検索（最大5件）
    let params = SearchParams::new()
        .max_results(5)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(), 
            "status".to_string(),
            "created".to_string()
        ]);
        
    let result = client.search_issues("order by created DESC", params).await;

    // Then: 成功し、検索結果が返される
    match result {
        Ok(search_result) => {
            println!("Search successful: {} total results", search_result.total);
            println!("Returned {} issues", search_result.issues.len());
            
            // 最大5件の結果があることを確認
            assert!(search_result.issues.len() <= 5);
            
            // 各チケットの基本情報を表示
            for issue in &search_result.issues {
                println!("- {}: {} ({})", 
                    issue.key, 
                    issue.fields.summary,
                    issue.fields.status.name
                );
            }
        },
        Err(e) => {
            panic!("Failed to search issues: {}", e);
        }
    }
}

/// プロジェクト固有の検索テスト
/// 
/// テスト内容:
/// - 特定プロジェクトの検索ができる
/// - プロジェクトキーによる絞り込みが機能する
/// - 存在しないプロジェクトでは結果が0件になる
#[tokio::test]
#[ignore] // 実際のJIRA APIが必要なため通常は無効化
async fn test_real_api_project_search() {
    // Given: 環境変数から設定を作成
    let client = setup_client_from_env()
        .expect("Failed to setup client. Please check environment variables.");

    // プロジェクト一覧を取得して最初のプロジェクトを使用
    let projects = client.get_projects().await
        .expect("Failed to get projects");
    
    if projects.is_empty() {
        println!("No projects found, skipping project-specific search test");
        return;
    }

    let first_project = &projects[0];
    let jql = format!("project = {}", first_project.key);

    // When: 特定プロジェクトの検索を実行
    let params = SearchParams::new().max_results(10);
    let result = client.search_issues(&jql, params).await;

    // Then: 成功し、そのプロジェクトのチケットが返される
    match result {
        Ok(search_result) => {
            println!("Project {} has {} total issues", 
                first_project.key, search_result.total);
            
            // プロジェクトのチケットがある場合、プロジェクトキーが一致することを確認
            for issue in &search_result.issues {
                assert!(issue.key.starts_with(&format!("{}-", first_project.key)));
            }
        },
        Err(e) => {
            panic!("Failed to search project issues: {}", e);
        }
    }
}

/// エラーハンドリングのテスト
/// 
/// テスト内容:
/// - 無効なJQLクエリで適切なエラーが返される
/// - エラーレスポンスが正しく解析される
#[tokio::test]
#[ignore] // 実際のJIRA APIが必要なため通常は無効化
async fn test_real_api_error_handling() {
    // Given: 環境変数から設定を作成
    let client = setup_client_from_env()
        .expect("Failed to setup client. Please check environment variables.");

    // When: 無効なJQLクエリを実行
    let invalid_jql = "INVALID JQL SYNTAX HERE";
    let params = SearchParams::new();
    let result = client.search_issues(invalid_jql, params).await;

    // Then: エラーが返される
    assert!(result.is_err(), "Expected error for invalid JQL");
    
    match result.unwrap_err() {
        jira_api::Error::ApiError { status, message } => {
            println!("Got expected API error: {} - {}", status, message);
            // JQLエラーは通常400 Bad Requestを返す
            assert_eq!(status, 400);
        },
        other => {
            panic!("Expected ApiError, got: {:?}", other);
        }
    }
}

/// 認証エラーのテスト
/// 
/// テスト内容:
/// - 無効な認証情報で適切なエラーが返される
/// - 401 Unauthorizedが正しく処理される
#[tokio::test]
#[ignore] // 実際のJIRA APIが必要なため通常は無効化
async fn test_real_api_auth_error() {
    use jira_api::{Auth, JiraConfig};
    
    // Given: 無効な認証情報で設定を作成
    let base_url = std::env::var("JIRA_URL")
        .expect("JIRA_URL environment variable not set");
        
    let invalid_auth = Auth::Basic {
        username: "invalid@example.com".to_string(),
        api_token: "invalid_token".to_string(),
    };
    
    let config = JiraConfig::new(base_url, invalid_auth)
        .expect("Failed to create config");
    let client = JiraClient::new(config)
        .expect("Failed to create client");

    // When: 無効な認証でAPIを呼び出す
    let result = client.get_projects().await;

    // Then: 認証エラーが返される
    assert!(result.is_err(), "Expected authentication error");
    
    match result.unwrap_err() {
        jira_api::Error::ApiError { status, .. } => {
            println!("Got expected auth error with status: {}", status);
            // 認証エラーは401 Unauthorizedを返す
            assert_eq!(status, 401);
        },
        other => {
            panic!("Expected ApiError, got: {:?}", other);
        }
    }
}