use jira_api::{JiraConfig, JiraClient, DuckDBStore, PersistenceStore, models::SearchParams};
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

    println!("基本的な履歴機能のサンプル");
    
    // 環境変数の確認
    println!("\n=== 環境変数の確認 ===");
    let jira_url = env::var("JIRA_URL").unwrap_or_else(|_| "未設定".to_string());
    let jira_user = env::var("JIRA_USER").unwrap_or_else(|_| "未設定".to_string());
    let jira_token = if env::var("JIRA_API_TOKEN").is_ok() { "設定済み" } else { "未設定" };
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "未設定（デフォルト: TEST）".to_string());
    
    println!("JIRA_URL: {}", jira_url);
    println!("JIRA_USER: {}", jira_user);
    println!("JIRA_API_TOKEN: {}", jira_token);
    println!("JIRA_PROJECT_KEY: {}", project_key);
    
    if jira_url == "未設定" || jira_user == "未設定" || jira_token == "未設定" {
        println!("\nNG 必要な環境変数が設定されていません。");
        println!(".envファイルを作成するか、環境変数を設定してください。");
        return Ok(());
    }
    
    // JIRAクライアントとストア初期化
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    
    let mut store = DuckDBStore::new_in_memory()?;
    store.initialize().await?;
    
    println!("初期化完了");
    
    // プロジェクトキーを環境変数から取得（フォールバック: TEST）
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    let jql_query = format!("project = {} ORDER BY updated DESC", project_key);
    
    // まず利用可能なプロジェクト一覧を取得してテスト
    println!("接続テスト: 利用可能なプロジェクト一覧を取得中...");
    match client.get_projects().await {
        Ok(projects) => {
            if projects.is_empty() {
                println!("警告: プロジェクトが見つかりませんでした。認証情報を確認してください。");
            } else {
                println!("利用可能なプロジェクト ({} 件):", projects.len());
                for project in projects.iter().take(5) {
                    println!("  - {} ({})", project.key, project.name);
                }
                if projects.len() > 5 {
                    println!("  ... ({} 件省略)", projects.len() - 5);
                }
                
                // 指定されたプロジェクトが存在するかチェック
                let project_exists = projects.iter().any(|p| p.key == project_key);
                if !project_exists {
                    println!("警告: 指定されたプロジェクト '{}' が見つかりません", project_key);
                    if let Some(first_project) = projects.first() {
                        println!("代わりに '{}' を試してみてください", first_project.key);
                    }
                }
            }
        }
        Err(e) => {
            println!("プロジェクト取得エラー: {}", e);
            println!("基本的な接続に問題がある可能性があります");
            return Err(e.into());
        }
    }
    
    println!("検索対象プロジェクト: {}", project_key);
    println!("JQLクエリ: {}", jql_query);
    
    // まずchangelog無しでテスト
    println!("\n=== ステップ1: changelog無しでのIssue検索 ===");
    let basic_search_params = SearchParams::new()
        .max_results(3); // fieldsを指定しない = 全フィールド取得
    
    println!("基本検索パラメータ: {:?}", basic_search_params);
    
    match client.search_issues(&jql_query, basic_search_params).await {
        Ok(search_result) => {
            println!("✅ 基本検索成功: {}件のIssue", search_result.issues.len());
            for issue in &search_result.issues {
                println!("  - {} | {}", issue.key, issue.fields.summary);
            }
            
            if search_result.issues.is_empty() {
                println!("❌ プロジェクトにIssueがありません");
                return Ok(());
            }
        }
        Err(e) => {
            println!("❌ 基本検索エラー: {}", e);
            return Err(e.into());
        }
    }
    
    // changelog付きでテスト
    println!("\n=== ステップ2: changelog付きでのIssue検索 ===");
    let changelog_search_params = SearchParams::new()
        .expand(vec!["changelog".to_string()])
        .max_results(3); // fieldsを指定しない = 全フィールド取得
    
    println!("changelog検索パラメータ: {:?}", changelog_search_params);
    let params_json = serde_json::to_string_pretty(&changelog_search_params).unwrap_or_else(|_| "変換エラー".to_string());
    println!("SearchParams JSON:\n{}", params_json);
    
    match client.search_issues(&jql_query, changelog_search_params).await {
        Ok(search_result) => {
            println!("✅ changelog検索成功: {}件のIssue", search_result.issues.len());
            
            if search_result.issues.is_empty() {
                println!("指定されたプロジェクト '{}' にIssueが見つかりませんでした", project_key);
                println!("プロジェクトキーが正しいか確認してください");
                return Ok(());
            }
            
            for issue in &search_result.issues {
                println!("Issue: {} - {}", issue.key, issue.fields.summary);
                
                if let Some(ref _changelog) = issue.changelog {
                    println!("  changelogデータあり");
                } else {
                    println!("  changelogデータなし");
                }
            }
            
            // Issueを保存
            let saved_count = store.save_issues(&search_result.issues).await?;
            println!("{}件のIssueを保存しました", saved_count);
        }
        Err(e) => {
            println!("❌ changelog検索エラー: {}", e);
            println!();
            println!("changelogエラーの一般的な原因:");
            println!("1. JIRA Cloud vs Server/Data Center の違い");
            println!("2. changelog へのアクセス権限不足");
            println!("3. 古いJIRAバージョンでの expand パラメータ非対応");
            println!();
            
            // 代替パターンを試す
            println!("=== 代替パターンのテスト ===");
            
            // パターン1: expand なしで再試行
            println!("パターン1: expand なしで再試行...");
            let alt_params1 = SearchParams::new().max_results(1);
                
            match client.search_issues(&jql_query, alt_params1).await {
                Ok(_) => println!("✅ expand無しは成功 → changelogサポートの問題"),
                Err(_) => println!("❌ expand無しも失敗 → 基本的な接続問題"),
            }
            
            // パターン2: 異なるexpandパラメータを試す
            println!("パターン2: expand=names,schema を試行...");
            let alt_params2 = SearchParams::new()
                .expand(vec!["names".to_string(), "schema".to_string()])
                .max_results(1);
                
            match client.search_issues(&jql_query, alt_params2).await {
                Ok(_) => println!("✅ 他のexpandは成功 → changelogサポートの問題"),
                Err(_) => println!("❌ 他のexpandも失敗 → expand機能全体の問題"),
            }
            
            println!();
            println!("解決方法:");
            println!("- JIRA管理者にchangelog閲覧権限の確認を依頼");
            println!("- JIRAのバージョンとchangelogサポート状況を確認");
            println!("- 基本的なIssue検索は動作するため、手動でchangelog取得を検討");
        }
    }
    
    println!("サンプル完了");
    Ok(())
}