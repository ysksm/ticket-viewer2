use dotenv::dotenv;
use jira_api::changelog_parser::ChangelogParser;
use jira_api::{JiraClient, JiraConfig, models::SearchParams};
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    dotenv().ok();

    println!("変更履歴解析（ChangelogParser）のサンプル");

    // 1. JIRAクライアント設定
    println!("\n=== 1. JIRA クライアントの設定 ===");
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    println!("JIRAクライアントを作成しました");

    // 2. 単一Issueの履歴取得（expand=changelog）
    println!("\n=== 2. 単一Issueの履歴取得 ===");
    let search_params = SearchParams::new()
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "created".to_string(),
            "updated".to_string(),
        ])
        .expand(vec!["changelog".to_string()])
        .max_results(1);

    // プロジェクトキーを環境変数から取得（フォールバック: TEST）
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    let jql_query = format!("project = {} ORDER BY updated DESC", project_key);
    println!("検索対象プロジェクト: {}", project_key);

    // 最新のIssueを1件取得
    let search_result = client.search_issues(&jql_query, search_params).await?;

    if search_result.issues.is_empty() {
        println!("取得できるIssueがありません。プロジェクトキーを確認してください。");
        return Ok(());
    }

    let issue = &search_result.issues[0];
    println!("取得Issue: {} - {}", issue.key, issue.fields.summary);
    println!(
        "作成日: {}",
        issue.fields.created.format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "更新日: {}",
        issue.fields.updated.format("%Y-%m-%d %H:%M:%S")
    );

    // 3. Changelogの存在確認
    println!("\n=== 3. Changelog の解析 ===");
    match &issue.changelog {
        Some(changelog) => {
            println!("Changelogデータが存在します");

            // 生のchangelogデータの一部を表示（デバッグ用）
            let changelog_str = serde_json::to_string_pretty(changelog)?;
            let lines: Vec<&str> = changelog_str.lines().collect();
            println!("Changelog JSON（最初の10行）:");
            for line in lines.iter().take(10) {
                println!("  {}", line);
            }
            if lines.len() > 10 {
                println!("  ... ({} 行省略)", lines.len() - 10);
            }
        }
        None => {
            println!("Changelogデータがありません");
            return Ok(());
        }
    }

    // 4. Changelogの解析
    println!("\n=== 4. Changelogの解析 ===");
    let changelog = issue.changelog.as_ref().unwrap();

    // Convert Changelog struct to JSON Value
    let changelog_json = serde_json::to_value(changelog)?;

    match ChangelogParser::parse_changelog(&issue.id, &issue.key, &changelog_json) {
        Ok(histories) => {
            println!("解析結果: {}件の履歴レコード", histories.len());

            if histories.is_empty() {
                println!("履歴レコードがありません");
                return Ok(());
            }

            // 5. 履歴レコードの詳細表示
            println!("\n=== 5. 履歴レコードの詳細 ===");
            for (i, history) in histories.iter().enumerate() {
                println!("\n--- 履歴 {} ---", i + 1);
                println!("変更ID: {}", history.change_id);
                println!(
                    "変更日時: {}",
                    history.change_timestamp.format("%Y-%m-%d %H:%M:%S")
                );

                if let Some(ref author) = history.author {
                    println!("変更者: {} ({})", author.display_name, author.account_id);
                } else {
                    println!("変更者: システム");
                }

                println!("フィールド: {}", history.field_name);
                if let Some(ref field_id) = history.field_id {
                    println!("フィールドID: {}", field_id);
                }

                println!("変更内容:");
                println!(
                    "  値: '{}' → '{}'",
                    history.from_value.as_deref().unwrap_or("None"),
                    history.to_value.as_deref().unwrap_or("None")
                );
                println!(
                    "  表示: '{}' → '{}'",
                    history.from_display_value.as_deref().unwrap_or("None"),
                    history.to_display_value.as_deref().unwrap_or("None")
                );

                println!("変更タイプ: {:?}", history.change_type());
                println!("変更サマリー: {}", history.change_summary());
            }

            // 6. 履歴分析機能のデモ
            println!("\n=== 6. 履歴分析機能 ===");

            // 変更統計
            let change_summary = ChangelogParser::generate_change_summary(&histories);
            println!("\n変更統計:");
            for (field_name, count) in change_summary {
                println!("  {}: {}回変更", field_name, count);
            }

            // 変更タイプ別分類
            let grouped_histories = ChangelogParser::group_by_change_type(&histories);
            println!("\n変更タイプ別分類:");
            for (change_type, type_histories) in grouped_histories {
                println!("  {}: {}件", change_type, type_histories.len());

                // 各タイプの詳細（最初の2件）
                for (j, history) in type_histories.iter().take(2).enumerate() {
                    println!("    {}: {}", j + 1, history.change_summary());
                }
                if type_histories.len() > 2 {
                    println!("    ... and {} more", type_histories.len() - 2);
                }
            }

            // 特定フィールドの変更抽出
            println!("\n=== 7. 特定フィールドの変更抽出 ===");

            // ステータス変更のみ
            let status_changes =
                ChangelogParser::extract_field_changes(&histories, &vec!["status".to_string()]);

            if !status_changes.is_empty() {
                println!("ステータス変更履歴 ({}件):", status_changes.len());
                for (i, change) in status_changes.iter().enumerate() {
                    println!(
                        "  {}. [{}] '{}' → '{}'",
                        i + 1,
                        change.change_timestamp.format("%Y-%m-%d %H:%M"),
                        change.from_display_value.as_deref().unwrap_or("None"),
                        change.to_display_value.as_deref().unwrap_or("None")
                    );
                }
            } else {
                println!("ステータス変更履歴はありません");
            }

            // 担当者変更のみ
            let assignee_changes =
                ChangelogParser::extract_field_changes(&histories, &vec!["assignee".to_string()]);

            if !assignee_changes.is_empty() {
                println!("\n担当者変更履歴 ({}件):", assignee_changes.len());
                for (i, change) in assignee_changes.iter().enumerate() {
                    println!(
                        "  {}. [{}] '{}' → '{}'",
                        i + 1,
                        change.change_timestamp.format("%Y-%m-%d %H:%M"),
                        change.from_display_value.as_deref().unwrap_or("未割当"),
                        change.to_display_value.as_deref().unwrap_or("未割当")
                    );
                }
            } else {
                println!("\n担当者変更履歴はありません");
            }
        }
        Err(e) => {
            eprintln!("Changelog解析エラー: {}", e);
            return Err(e.into());
        }
    }

    println!("\n=== ChangelogParser サンプル完了 ===");
    println!("このサンプルでは以下を学習できます:");
    println!("- expand=changelogを使用したJIRA API呼び出し");
    println!("- ChangelogParserによる生JSON解析");
    println!("- IssueHistoryオブジェクトの作成");
    println!("- 履歴データの分析・分類機能");
    println!("- 特定フィールドの変更履歴抽出");
    println!("- 変更統計の生成");

    Ok(())
}
