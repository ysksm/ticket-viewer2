use jira_api::{
    JiraConfig, JiraClient, DuckDBStore, PersistenceStore,
    SyncService, SyncConfig, ChangelogParser,
    models::SearchParams
};
use dotenv::dotenv;
use std::error::Error;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    dotenv().ok();

    println!("同期処理と履歴管理を組み合わせたサンプル");
    
    // 1. システム初期化
    println!("\n=== 1. システム初期化 ===");
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    
    let mut store = DuckDBStore::new_in_memory()?;
    store.initialize().await?;
    println!("JIRAクライアントとDuckDBストアを初期化しました");
    
    // プロジェクトキーを環境変数から取得
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    println!("検索対象プロジェクト: {}", project_key);
    
    // 2. 同期設定
    println!("\n=== 2. 同期設定の構成 ===");
    let sync_config = SyncConfig::new()
        .target_projects(vec![project_key.clone()])
        .interval_minutes(30) // 30分間隔
        .max_history_count(1000); // 最大履歴数
    
    println!("同期設定を構成しました:");
    println!("  プロジェクト: {:?}", sync_config.target_projects);
    println!("  間隔: {}分", sync_config.interval_minutes);
    println!("  最大履歴数: {}", sync_config.max_history_count);
    
    // 3. 同期サービス初期化
    println!("\n=== 3. 同期サービス初期化 ===");
    let mut sync_service = SyncService::new(sync_config.clone());
    println!("同期サービスを初期化しました");
    
    // 4. JIRAからIssueデータを取得（expand=changelogを使用）
    println!("\n=== 4. Issue取得（履歴データ付き） ===");
    let search_params = SearchParams::new()
        .expand(vec!["changelog".to_string()])
        .max_results(10); // サンプルなので少なめに
    
    let jql_query = format!("project = {} ORDER BY updated DESC", project_key);
    let search_result = client.search_issues(&jql_query, search_params).await?;
    
    println!("取得したIssue数: {}", search_result.issues.len());
    
    // 5. Issueをストアに保存
    println!("\n=== 5. Issueデータの保存 ===");
    let saved_count = store.save_issues(&search_result.issues).await?;
    println!("{}件のIssueを保存しました", saved_count);
    
    // 6. 履歴データの解析と保存
    println!("\n=== 6. 履歴データの解析と保存 ===");
    let mut all_histories = Vec::new();
    let mut processed_count = 0;
    
    for issue in &search_result.issues {
        if let Some(ref changelog) = issue.changelog {
            // Convert Changelog struct to JSON Value
            let changelog_json = serde_json::to_value(changelog)?;
            match ChangelogParser::parse_changelog(&issue.id, &issue.key, &changelog_json) {
                Ok(histories) => {
                    if !histories.is_empty() {
                        println!("  {}の履歴: {}件", issue.key, histories.len());
                        all_histories.extend(histories);
                        processed_count += 1;
                    }
                }
                Err(e) => {
                    println!("  {}の履歴解析エラー: {}", issue.key, e);
                }
            }
        }
    }
    
    if !all_histories.is_empty() {
        let saved_history_count = store.save_issue_history(&all_histories).await?;
        println!("{}件のIssueから{}件の履歴レコードを保存しました", processed_count, saved_history_count);
    } else {
        println!("履歴データが見つかりませんでした");
    }
    
    // 7. 統計情報の表示
    println!("\n=== 7. 統計情報 ===");
    let stats = store.get_stats().await?;
    println!("保存されているIssue総数: {}", stats.total_issues);
    
    let history_stats = store.get_history_stats().await?;
    println!("保存されている履歴レコード総数: {}", history_stats.total_changes);
    println!("履歴対象Issue数: {}", history_stats.unique_issues);
    println!("変更者数: {}", history_stats.unique_authors);
    println!("最古の履歴: {:?}", history_stats.oldest_change);
    println!("最新の履歴: {:?}", history_stats.newest_change);
    
    // 8. 簡単な同期テスト
    println!("\n=== 8. 同期機能のテスト ===");
    let empty_issues = Vec::new();
    match sync_service.sync_incremental(&client, &empty_issues).await {
        Ok(sync_result) => {
            println!("同期テスト成功:");
            println!("  同期したIssue数: {}", sync_result.synced_issues_count);
            println!("  新規Issue数: {}", sync_result.new_issues_count);
            println!("  更新されたIssue数: {}", sync_result.updated_issues_count);
            let duration = sync_result.end_time - sync_result.start_time;
            println!("  実行時間: {:?}", duration);
        }
        Err(e) => {
            println!("同期エラー: {}", e);
        }
    }
    
    println!("\n=== サンプル完了 ===");
    println!("このサンプルでは以下を学習できます:");
    println!("- SyncConfigとSyncServiceの基本的な使用方法");
    println!("- expand=changelogを使った履歴データ付きIssue取得");
    println!("- ChangelogParserによる履歴データの解析");
    println!("- DuckDBStoreを使った履歴データの永続化");
    println!("- 統計情報の取得とモニタリング");
    
    Ok(())
}