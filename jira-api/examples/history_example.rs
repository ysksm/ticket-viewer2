use dotenv::dotenv;
use jira_api::{
    ChangelogParser, DuckDBStore, HistoryFilter, HistorySortOrder, JiraClient, JiraConfig,
    PersistenceStore, models::SearchParams,
};
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    dotenv().ok();

    println!("履歴機能のサンプルを開始します");

    // 1. JIRAクライアント設定
    println!("\n=== 1. JIRA クライアントの設定 ===");
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    println!("JIRAクライアントを作成しました");

    // 2. DuckDBストアの初期化
    println!("\n=== 2. DuckDB ストアの初期化 ===");
    let mut store = DuckDBStore::new_in_memory()?;
    store.initialize().await?;
    println!("DuckDBストアを初期化しました");

    // 3. expand=changelogを使用したIssue取得
    println!("\n=== 3. 履歴データ付きIssue取得 ===");
    let search_params = SearchParams::new()
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
        ])
        .expand(vec!["changelog".to_string()])
        .max_results(10);

    // プロジェクトキーを環境変数から取得（フォールバック: TEST）
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    let jql_query = format!("project = {} ORDER BY updated DESC", project_key);
    println!("検索対象プロジェクト: {}", project_key);

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
                    for history in &histories {
                        let summary = history.change_summary();
                        println!("  {}", summary);
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
        println!("{}件の履歴データを保存しました", saved_history_count);
    } else {
        println!("保存する履歴データがありません");
    }

    // 7. 履歴データの検索例
    println!("\n=== 7. 履歴データ検索の例 ===");

    // 全履歴の取得
    println!("\n[7.1] 全履歴データ");
    let all_filter = HistoryFilter::new().limit(20);
    let all_loaded_histories = store.load_issue_history(&all_filter).await?;
    println!("履歴データ総数（最大20件）: {}", all_loaded_histories.len());

    // ステータス変更のみの取得
    println!("\n[7.2] ステータス変更のみ");
    let status_filter = HistoryFilter::new()
        .field_names(vec!["status".to_string()])
        .sort_order(HistorySortOrder::TimestampDesc)
        .limit(10);
    let status_histories = store.load_issue_history(&status_filter).await?;
    println!("ステータス変更履歴: {}件", status_histories.len());
    for history in &status_histories {
        println!(
            "  {}: {} → {}",
            history.issue_key,
            history.from_display_value.as_deref().unwrap_or("None"),
            history.to_display_value.as_deref().unwrap_or("None")
        );
    }

    // 特定課題の履歴
    if let Some(first_issue) = search_result.issues.first() {
        println!("\n[7.3] 特定課題の履歴: {}", first_issue.key);
        let issue_filter = HistoryFilter::new()
            .issue_keys(vec![first_issue.key.clone()])
            .sort_order(HistorySortOrder::TimestampAsc);
        let issue_histories = store.load_issue_history(&issue_filter).await?;
        println!("{}の履歴: {}件", first_issue.key, issue_histories.len());
        for history in &issue_histories {
            println!(
                "  [{}] {} changed from '{}' to '{}'",
                history.change_timestamp.format("%Y-%m-%d %H:%M:%S"),
                history.field_name,
                history.from_display_value.as_deref().unwrap_or("None"),
                history.to_display_value.as_deref().unwrap_or("None")
            );
        }
    }

    // 8. 履歴統計の取得
    println!("\n=== 8. 履歴統計情報 ===");
    let stats = store.get_history_stats().await?;
    println!("総変更数: {}", stats.total_changes);
    println!("履歴のある課題数: {}", stats.unique_issues);
    println!("変更者数: {}", stats.unique_authors);

    println!("\nフィールド別変更数:");
    for (field_name, count) in &stats.field_change_counts {
        println!("  {}: {}回", field_name, count);
    }

    // 9. 履歴解析の追加機能デモ
    println!("\n=== 9. 履歴解析の追加機能 ===");

    // 変更タイプ別の分類
    if !all_histories.is_empty() {
        let grouped_histories = ChangelogParser::group_by_change_type(&all_histories);
        println!("\n変更タイプ別の履歴数:");
        for (change_type, histories) in &grouped_histories {
            println!("  {}: {}件", change_type, histories.len());
        }

        // 変更統計の生成
        let change_summary = ChangelogParser::generate_change_summary(&all_histories);
        println!("\n変更統計サマリー:");
        for (field_name, count) in change_summary {
            println!("  {}: {}回", field_name, count);
        }

        // 特定フィールドの変更のみを抽出
        let status_only =
            ChangelogParser::extract_field_changes(&all_histories, &vec!["status".to_string()]);
        println!("\nステータス変更のみ抽出: {}件", status_only.len());
    }

    println!("\n=== 履歴機能サンプル完了 ===");
    println!("このサンプルでは以下の機能を確認できます:");
    println!("- expand=changelogを使用した履歴データ付きIssue取得");
    println!("- changelogの解析とIssueHistoryレコードの生成");
    println!("- DuckDBへの履歴データ保存");
    println!("- 様々な条件での履歴検索（課題別、フィールド別、日時順）");
    println!("- 履歴統計の取得");
    println!("- 履歴データの分析・分類機能");

    Ok(())
}
