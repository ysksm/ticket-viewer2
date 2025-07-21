use jira_api::{
    JiraConfig, JiraClient, JsonStore, PersistenceStore, HistoryFilter,
    ChangelogParser, HistorySortOrder, models::SearchParams
};
use dotenv::dotenv;
use std::error::Error;
use std::env;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    dotenv().ok();

    println!("JSON Store 履歴機能のサンプルを開始します");
    
    // 1. JIRAクライアント設定
    println!("\n=== 1. JIRA クライアントの設定 ===");
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    println!("JIRAクライアントを作成しました");
    
    // 2. JsonStoreの初期化（一時ディレクトリ使用）
    println!("\n=== 2. JSON Store の初期化 ===");
    let temp_dir = TempDir::new()?;
    let mut store = JsonStore::new(temp_dir.path()).with_compression(false); // 確認のため圧縮なし
    store.initialize().await?;
    println!("JsonStore を初期化しました: {:?}", temp_dir.path());
    
    // プロジェクトキーを環境変数から取得（フォールバック: TEST）
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    let jql_query = format!("project = {} ORDER BY updated DESC", project_key);
    println!("検索対象プロジェクト: {}", project_key);
    
    // 3. expand=changelogを使用したIssue取得
    println!("\n=== 3. 履歴データ付きIssue取得 ===");
    let search_params = SearchParams::new()
        .expand(vec!["changelog".to_string()])
        .max_results(5); // JsonStoreのサンプルは少なめに
    
    let search_result = client.search_issues(&jql_query, search_params).await?;
    
    println!("取得したIssue数: {}", search_result.issues.len());
    
    // 4. Issueの保存（通常データ）
    println!("\n=== 4. Issue データの保存 ===");
    let saved_count = store.save_issues(&search_result.issues).await?;
    println!("{}件のIssueを保存しました", saved_count);
    
    // 5. changelogの解析と履歴データ生成
    println!("\n=== 5. 履歴データの解析と生成 ===");
    let mut all_histories = Vec::new();
    
    for issue in &search_result.issues {
        if let Some(ref changelog) = issue.changelog {
            // Convert Changelog struct to JSON Value
            let changelog_json = serde_json::to_value(changelog)?;
            match ChangelogParser::parse_changelog(&issue.id, &issue.key, &changelog_json) {
                Ok(histories) => {
                    println!("{}の履歴: {}件", issue.key, histories.len());
                    // サンプルの履歴を表示
                    for (i, history) in histories.iter().enumerate() {
                        if i < 3 { // 最初の3件のみ表示
                            println!("  - {}: {} → {}", 
                                history.field_name,
                                history.from_display_value.as_deref().unwrap_or("None"),
                                history.to_display_value.as_deref().unwrap_or("None")
                            );
                        }
                    }
                    if histories.len() > 3 {
                        println!("  ... and {} more", histories.len() - 3);
                    }
                    all_histories.extend(histories);
                }
                Err(e) => {
                    eprintln!("{}の履歴解析エラー: {}", issue.key, e);
                }
            }
        }
    }
    
    println!("総履歴データ数: {}", all_histories.len());
    
    // 6. 履歴データの保存
    println!("\n=== 6. 履歴データの保存 ===");
    if !all_histories.is_empty() {
        let saved_history_count = store.save_issue_history(&all_histories).await?;
        println!("{}件の履歴データをJSONファイルに保存しました", saved_history_count);
        
        // 保存されたファイルの確認
        let history_file = temp_dir.path().join("history").join("history.json");
        if history_file.exists() {
            println!("履歴ファイル作成: {:?}", history_file);
        }
    } else {
        println!("保存する履歴データがありません");
    }
    
    // 7. 履歴データの検索例
    println!("\n=== 7. 履歴データ検索の例 ===");
    
    // 全履歴の取得
    println!("\n[7.1] 全履歴データ");
    let all_filter = HistoryFilter::new().limit(10);
    let all_loaded_histories = store.load_issue_history(&all_filter).await?;
    println!("履歴データ総数（最大10件）: {}", all_loaded_histories.len());
    
    // 担当者変更のみの取得
    println!("\n[7.2] 担当者変更のみ");
    let assignee_filter = HistoryFilter::new()
        .field_names(vec!["assignee".to_string()])
        .sort_order(HistorySortOrder::TimestampDesc);
    let assignee_histories = store.load_issue_history(&assignee_filter).await?;
    println!("担当者変更履歴: {}件", assignee_histories.len());
    for history in &assignee_histories {
        println!("  {}: {} → {}", 
            history.issue_key,
            history.from_display_value.as_deref().unwrap_or("未割当"),
            history.to_display_value.as_deref().unwrap_or("未割当")
        );
    }
    
    // 特定課題の履歴（時系列順）
    if let Some(first_issue) = search_result.issues.first() {
        println!("\n[7.3] 特定課題の時系列履歴: {}", first_issue.key);
        let issue_filter = HistoryFilter::new()
            .issue_keys(vec![first_issue.key.clone()])
            .sort_order(HistorySortOrder::TimestampAsc);
        let issue_histories = store.load_issue_history(&issue_filter).await?;
        println!("{}の履歴: {}件", first_issue.key, issue_histories.len());
        for (i, history) in issue_histories.iter().enumerate() {
            println!("  {}. [{}] {} changed",
                i + 1,
                history.change_timestamp.format("%Y-%m-%d %H:%M"),
                history.field_name
            );
        }
    }
    
    // 8. 履歴統計の取得
    println!("\n=== 8. 履歴統計情報 ===");
    let stats = store.get_history_stats().await?;
    println!("総変更数: {}", stats.total_changes);
    println!("履歴のある課題数: {}", stats.unique_issues);
    println!("変更者数: {}", stats.unique_authors);
    
    if !stats.field_change_counts.is_empty() {
        println!("\nフィールド別変更数:");
        let mut sorted_fields: Vec<_> = stats.field_change_counts.iter().collect();
        sorted_fields.sort_by(|a, b| b.1.cmp(a.1)); // 変更数の降順
        for (field_name, count) in sorted_fields.iter().take(5) { // 上位5フィールド
            println!("  {}: {}回", field_name, count);
        }
    }
    
    // 9. JSONファイルでの履歴管理の利点
    println!("\n=== 9. JSONファイル履歴管理の特徴 ===");
    println!("✓ 人間が読みやすいJSONフォーマット");
    println!("✓ 外部ツールでの分析が容易");
    println!("✓ バックアップとポータビリティに優れる");
    println!("✓ 圧縮オプションでストレージ効率向上");
    
    // ファイル一覧を表示
    println!("\n作成されたファイル:");
    for entry in std::fs::read_dir(temp_dir.path())? {
        let entry = entry?;
        if entry.path().is_dir() {
            println!("  📁 {}/", entry.file_name().to_string_lossy());
            for sub_entry in std::fs::read_dir(entry.path())? {
                let sub_entry = sub_entry?;
                let metadata = sub_entry.metadata()?;
                println!("     📄 {} ({} bytes)", 
                    sub_entry.file_name().to_string_lossy(),
                    metadata.len()
                );
            }
        }
    }
    
    // 10. 履歴データの削除デモ
    println!("\n=== 10. 履歴データの削除デモ ===");
    if let Some(first_issue) = search_result.issues.first() {
        println!("{}の履歴を削除します", first_issue.key);
        let deleted_count = store.delete_issue_history(&[first_issue.key.clone()]).await?;
        println!("{}件の履歴を削除しました", deleted_count);
        
        // 削除後の統計を確認
        let updated_stats = store.get_history_stats().await?;
        println!("削除後の総変更数: {}", updated_stats.total_changes);
    }
    
    println!("\n=== JSON Store 履歴機能サンプル完了 ===");
    println!("このサンプルでは以下を学習できます:");
    println!("- JSONファイルベースの履歴データ管理");
    println!("- インメモリフィルタリングとソート");
    println!("- ファイルサイズと可読性のバランス");
    println!("- 他のツールとの連携の容易さ");
    
    // 一時ディレクトリは自動削除される
    println!("\n注意: 一時ディレクトリは自動削除されます");
    
    Ok(())
}