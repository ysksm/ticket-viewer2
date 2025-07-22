/// JIRA APIクライアントの基本的な使用例
///
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
///
/// 実行方法：
/// cargo run --example basic_usage
use dotenv::dotenv;
use std::env;

use jira_api::{Auth, JiraClient, JiraConfig, ProjectParams, SearchParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!("JIRA API クライアント基本使用例");
    println!("==================================");

    // 環境変数から設定を取得、またはfrom_env()を使用
    let config = match JiraConfig::from_env() {
        Ok(config) => {
            println!("[OK] 環境変数から設定を読み込みました");
            config
        }
        Err(_) => {
            println!("[WARNING] 環境変数が設定されていません。デフォルト値を使用します。");
            println!("   実際のAPIを呼び出すには環境変数を設定してください。");

            let base_url = env::var("JIRA_URL")
                .unwrap_or_else(|_| "https://your-domain.atlassian.net".to_string());
            let username =
                env::var("JIRA_USER").unwrap_or_else(|_| "your-email@example.com".to_string());
            let api_token =
                env::var("JIRA_API_TOKEN").unwrap_or_else(|_| "your-api-token".to_string());

            JiraConfig::new(
                base_url,
                Auth::Basic {
                    username,
                    api_token,
                },
            )?
        }
    };

    println!("[INFO] JIRA URL: {}", config.base_url);

    // JIRAクライアントを作成
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアントを作成しました");

    // プロジェクト一覧を取得
    println!("\n[INFO] プロジェクト一覧を取得中...");
    match client.get_projects().await {
        Ok(projects) => {
            println!("[OK] {} 個のプロジェクトが見つかりました:", projects.len());
            for (i, project) in projects.iter().take(5).enumerate() {
                println!(
                    "   {}. {} - {} ({})",
                    i + 1,
                    project.key,
                    project.name,
                    project.project_type_key.as_deref().unwrap_or("unknown")
                );
            }
            if projects.len() > 5 {
                println!("   ... and {} more", projects.len() - 5);
            }
        }
        Err(e) => {
            println!("[ERROR] プロジェクト取得エラー: {}", e);
            println!("   環境変数が正しく設定されているか確認してください。");
        }
    }

    // 詳細情報付きプロジェクト取得の例
    println!("\n[INFO] 詳細情報付きプロジェクト取得中...");
    let project_params = ProjectParams::new()
        .expand(vec!["lead".to_string(), "description".to_string()])
        .recent(3);

    match client.get_projects_with_params(project_params).await {
        Ok(projects) => {
            println!("[OK] {} 個のプロジェクト（詳細情報付き）:", projects.len());
            for project in projects.iter().take(3) {
                println!("   {} - {}", project.key, project.name);
                if let Some(desc) = &project.description {
                    println!("      説明: {}", desc);
                }
                if let Some(lead) = &project.lead {
                    println!("      リード: {}", lead.display_name);
                }
            }
        }
        Err(e) => {
            println!("[ERROR] 詳細プロジェクト取得エラー: {}", e);
        }
    }

    // チケット検索の例
    println!("\n[INFO] 最近のチケットを検索中...");
    let search_params = SearchParams::new().max_results(5).fields(vec![
        "key".to_string(),
        "summary".to_string(),
        "status".to_string(),
        "assignee".to_string(),
        "reporter".to_string(),
        "created".to_string(),
        "updated".to_string(),
        "issuetype".to_string(),
        "priority".to_string(),
    ]);

    match client
        .search_issues("order by created DESC", search_params)
        .await
    {
        Ok(search_result) => {
            println!(
                "[OK] 検索結果: {} 件中 {} 件を表示",
                search_result.total,
                search_result.issues.len()
            );

            for issue in &search_result.issues {
                let assignee = issue
                    .fields
                    .assignee
                    .as_ref()
                    .map(|a| a.display_name.as_str())
                    .unwrap_or("未割当");

                println!(
                    "   {} - {} [{}] (担当: {})",
                    issue.key, issue.fields.summary, issue.fields.status.name, assignee
                );
            }
        }
        Err(e) => {
            println!("[ERROR] チケット検索エラー: {}", e);
            println!("   詳細: {:?}", e);
        }
    }

    // プロジェクト固有検索の例（最初のプロジェクトが見つかった場合）
    println!("\n[INFO] プロジェクト固有検索の例...");
    match client.get_projects().await {
        Ok(projects) if !projects.is_empty() => {
            let first_project = &projects[0];
            let jql = format!("project = {} ORDER BY created DESC", first_project.key);
            let params = SearchParams::new().max_results(3).fields(vec![
                "key".to_string(),
                "summary".to_string(),
                "status".to_string(),
                "issuetype".to_string(),
                "reporter".to_string(),
                "created".to_string(),
                "updated".to_string(),
            ]);

            match client.search_issues(&jql, params).await {
                Ok(result) => {
                    println!(
                        "[OK] プロジェクト {} の最新チケット {} 件:",
                        first_project.key,
                        result.issues.len()
                    );
                    for issue in &result.issues {
                        println!("   {} - {}", issue.key, issue.fields.summary);
                    }
                }
                Err(e) => {
                    println!("[ERROR] プロジェクト検索エラー: {}", e);
                }
            }
        }
        _ => {
            println!("[WARNING] プロジェクト固有検索をスキップ（プロジェクトが見つからない）");
        }
    }

    println!("\n基本使用例完了!");
    println!("\n他のサンプル:");
    println!("   cargo run --example search_example");
    println!("   cargo run --example project_example");

    Ok(())
}
