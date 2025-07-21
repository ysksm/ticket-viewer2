use jira_api::{JiraConfig, JiraClient};
use dotenv::dotenv;
use std::error::Error;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // .envファイルを読み込み
    match dotenv() {
        Ok(path) => println!(".envファイルを読み込みました: {:?}", path),
        Err(_) => println!(".envファイルが見つかりません（システム環境変数を使用）"),
    }

    println!("JIRA APIレスポンスのデバッグテスト");
    
    // 環境変数の確認
    println!("\n=== 環境変数の確認 ===");
    let jira_url = env::var("JIRA_URL").unwrap_or_else(|_| "未設定".to_string());
    let jira_user = env::var("JIRA_USER").unwrap_or_else(|_| "未設定".to_string());
    let jira_token = if env::var("JIRA_API_TOKEN").is_ok() { "設定済み" } else { "未設定" };
    
    println!("JIRA_URL: {}", jira_url);
    println!("JIRA_USER: {}", jira_user);
    println!("JIRA_API_TOKEN: {}", jira_token);
    
    if jira_url == "未設定" || jira_user == "未設定" || jira_token == "未設定" {
        println!("\n❌ 必要な環境変数が設定されていません。");
        return Ok(());
    }
    
    // JIRAクライアント初期化
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    
    // まずプロジェクト一覧で基本接続をテスト
    println!("\n=== ステップ1: プロジェクト一覧取得 ===");
    match client.get_projects().await {
        Ok(projects) => {
            println!("✅ プロジェクト取得成功: {}件", projects.len());
            if let Some(first_project) = projects.first() {
                println!("最初のプロジェクト: {} ({})", first_project.key, first_project.name);
            }
        }
        Err(e) => {
            println!("❌ プロジェクト取得エラー: {}", e);
            return Err(e.into());
        }
    }
    
    // 次に、生のHTTPクライアントでsearch APIを試す
    println!("\n=== ステップ2: 生のHTTPリクエストでsearch APIテスト ===");
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    
    // reqwestを使って直接APIを呼び出し、生のレスポンスを確認
    let auth_header = format!("{}:{}", 
        env::var("JIRA_USER").unwrap(), 
        env::var("JIRA_API_TOKEN").unwrap()
    );
    let encoded_auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_header.as_bytes());
    
    let http_client = reqwest::Client::new();
    let search_url = format!("{}/rest/api/3/search", jira_url);
    
    let search_body = serde_json::json!({
        "jql": format!("project = {} ORDER BY updated DESC", project_key),
        "maxResults": 1
    });
    
    println!("リクエストURL: {}", search_url);
    println!("リクエストボディ: {}", serde_json::to_string_pretty(&search_body)?);
    
    let response = http_client
        .post(&search_url)
        .header("Authorization", format!("Basic {}", encoded_auth))
        .header("Content-Type", "application/json")
        .json(&search_body)
        .send()
        .await?;
    
    let status = response.status();
    println!("レスポンスステータス: {}", status);
    
    if status.is_success() {
        let response_text = response.text().await?;
        println!("レスポンス長: {} bytes", response_text.len());
        
        // JSONとしてパースしてみる
        match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json_value) => {
                println!("✅ JSONパース成功");
                println!("レスポンス構造（最初の100文字）:");
                let pretty_json = serde_json::to_string_pretty(&json_value)?;
                let preview = if pretty_json.len() > 500 {
                    format!("{}...", &pretty_json[..500])
                } else {
                    pretty_json
                };
                println!("{}", preview);
                
                // issuesフィールドの最初のアイテムの構造を確認
                if let Some(issues) = json_value.get("issues").and_then(|v| v.as_array()) {
                    if let Some(first_issue) = issues.first() {
                        println!("\n=== 最初のIssueの構造 ===");
                        if let Some(fields) = first_issue.get("fields") {
                            println!("fieldsオブジェクトのキー:");
                            if let Some(fields_obj) = fields.as_object() {
                                for key in fields_obj.keys() {
                                    println!("  - {}", key);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ JSONパースエラー: {}", e);
                println!("レスポンステキスト（最初の500文字）:");
                let preview = if response_text.len() > 500 {
                    format!("{}...", &response_text[..500])
                } else {
                    response_text
                };
                println!("{}", preview);
            }
        }
    } else {
        let error_text = response.text().await?;
        println!("❌ HTTPエラー: {}", error_text);
    }
    
    println!("\nデバッグテスト完了");
    Ok(())
}