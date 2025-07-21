/// JIRA チケット検索の高度な使用例
/// 
/// 様々なJQLクエリと検索パラメータの使い方を示します
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example search_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, SearchParams};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("JIRA チケット検索の高度な使用例");
    println!("==================================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. 基本的な検索
    println!("\n[1] 基本的な検索 - 最近作成されたチケット");
    let basic_search = SearchParams::new()
        .max_results(10)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "assignee".to_string(),
            "priority".to_string(),
            "reporter".to_string(),
            "created".to_string(),
            "updated".to_string(),
            "issuetype".to_string()
        ]);
    
    match client.search_issues("order by created DESC", basic_search).await {
        Ok(result) => {
            println!("   総件数: {} 件", result.total);
            for issue in result.issues.iter().take(5) {
                let assignee = issue.fields.assignee
                    .as_ref()
                    .map(|a| a.display_name.as_str())
                    .unwrap_or("未割当");
                let priority = issue.fields.priority
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("なし");
                    
                println!("   {} - {} [{}] (担当: {}, 優先度: {})",
                    issue.key,
                    issue.fields.summary,
                    issue.fields.status.name,
                    assignee,
                    priority
                );
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 2. 特定条件での検索
    println!("\n[2] 特定条件での検索 - 未解決チケット");
    let status_search = SearchParams::new()
        .max_results(5)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "reporter".to_string(),
            "created".to_string(),
            "updated".to_string(),
            "issuetype".to_string()
        ]);
    
    let jql = r#"
        resolution = Unresolved 
        AND status != Closed 
        ORDER BY updated DESC
    "#;
    
    match client.search_issues(jql, status_search).await {
        Ok(result) => {
            println!("   未解決チケット: {} 件", result.total);
            for issue in result.issues.iter().take(3) {
                println!("   {} - {} [{}]",
                    issue.key,
                    issue.fields.summary,
                    issue.fields.status.name
                );
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 3. ページネーション付き検索
    println!("\n[3] ページネーション付き検索");
    let mut start_at = 0;
    let page_size = 3;
    let mut total_fetched = 0;
    
    loop {
        let paginated_search = SearchParams::new()
            .start_at(start_at)
            .max_results(page_size)
            .fields(vec![
                "key".to_string(),
                "summary".to_string(),
                "status".to_string(),
                "reporter".to_string(),
                "created".to_string(),
                "updated".to_string(),
                "issuetype".to_string()
            ]);
        
        match client.search_issues("order by key ASC", paginated_search).await {
            Ok(result) => {
                if result.issues.is_empty() {
                    break;
                }
                
                println!("    ページ {}: {} 件 (全 {} 件中 {}-{} 件目)",
                    (start_at / page_size) + 1,
                    result.issues.len(),
                    result.total,
                    start_at + 1,
                    start_at + result.issues.len() as u32
                );
                
                for issue in &result.issues {
                    println!("      {} - {}", issue.key, issue.fields.summary);
                }
                
                total_fetched += result.issues.len();
                start_at += page_size;
                
                // 例として最初の2ページのみ取得
                if total_fetched >= 6 {
                    println!("    2ページ取得完了（デモのため制限）");
                    break;
                }
            }
            Err(e) => {
                println!("   [ERROR] ページ取得エラー: {}", e);
                break;
            }
        }
    }

    // 4. 複雑なJQLクエリ
    println!("\n[4] 複雑なJQLクエリの例");
    let complex_queries = vec![
        ("高優先度チケット", "priority in (High, Highest) ORDER BY created DESC"),
        ("今週更新されたチケット", "updated >= -7d ORDER BY updated DESC"),
        ("バグレポート", r#"issuetype = "Bug" AND status != Done ORDER BY priority DESC"#),
        ("自分に割当済み", "assignee = currentUser() ORDER BY updated DESC"),
    ];
    
    for (description, jql) in complex_queries {
        println!("    {}", description);
        let params = SearchParams::new()
            .max_results(3)
            .fields(vec![
                "key".to_string(),
                "summary".to_string(),
                "status".to_string(),
                "reporter".to_string(),
                "created".to_string(),
                "updated".to_string(),
                "issuetype".to_string()
            ]);
        
        match client.search_issues(jql, params).await {
            Ok(result) => {
                println!("      {} 件見つかりました", result.total);
                for issue in result.issues.iter().take(2) {
                    println!("      {} - {}", issue.key, issue.fields.summary);
                }
            }
            Err(e) => println!("     [ERROR] エラー: {}", e),
        }
    }

    // 5. 詳細情報付き検索
    println!("\n[5] 詳細情報付き検索");
    let detailed_search = SearchParams::new()
        .max_results(3)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "assignee".to_string(),
            "reporter".to_string(),
            "issuetype".to_string(),
            "priority".to_string(),
            "created".to_string(),
            "updated".to_string()
        ]);
    
    match client.search_issues("order by updated DESC", detailed_search).await {
        Ok(result) => {
            println!("    詳細情報付きチケット: {} 件", result.total);
            for issue in result.issues.iter().take(3) {
                println!("    {} - {}", issue.key, issue.fields.summary);
                println!("      ステータス: {}", issue.fields.status.name);
                println!("     報告者: {}", issue.fields.reporter.display_name);
                
                if let Some(assignee) = &issue.fields.assignee {
                    println!("     担当者: {}", assignee.display_name);
                } else {
                    println!("     担当者: 未割当");
                }
                
                if let Some(priority) = &issue.fields.priority {
                    println!("      優先度: {}", priority.name);
                }
                
                println!(); // 空行
            }
        }
        Err(e) => {
            println!("   [ERROR] エラー: {}", e);
        }
    }

    // 6. 検索結果の統計
    println!("\n[6] 検索結果の統計情報");
    let stats_search = SearchParams::new()
        .max_results(100) // より多くのデータを取得して統計を作成
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "priority".to_string(),
            "issuetype".to_string(),
            "assignee".to_string(),
            "reporter".to_string(),
            "created".to_string(),
            "updated".to_string()
        ]);
    
    match client.search_issues("order by created DESC", stats_search).await {
        Ok(result) => {
            println!("    統計対象: {} 件", result.issues.len());
            
            // ステータス別統計
            let mut status_counts: HashMap<String, usize> = HashMap::new();
            let mut priority_counts: HashMap<String, usize> = HashMap::new();
            let mut assignee_counts: HashMap<String, usize> = HashMap::new();
            
            for issue in &result.issues {
                // ステータス統計
                *status_counts.entry(issue.fields.status.name.clone()).or_insert(0) += 1;
                
                // 優先度統計
                if let Some(priority) = &issue.fields.priority {
                    *priority_counts.entry(priority.name.clone()).or_insert(0) += 1;
                }
                
                // 担当者統計
                let assignee = issue.fields.assignee
                    .as_ref()
                    .map(|a| a.display_name.clone())
                    .unwrap_or_else(|| "未割当".to_string());
                *assignee_counts.entry(assignee).or_insert(0) += 1;
            }
            
            // 統計結果を表示
            println!("     ステータス分布:");
            for (status, count) in status_counts.iter().take(5) {
                println!("     {} : {} 件", status, count);
            }
            
            println!("    優先度分布:");
            for (priority, count) in priority_counts.iter().take(5) {
                println!("     {} : {} 件", priority, count);
            }
            
            println!("    担当者分布 (上位5名):");
            let mut assignee_vec: Vec<_> = assignee_counts.iter().collect();
            assignee_vec.sort_by(|a, b| b.1.cmp(a.1));
            for (assignee, count) in assignee_vec.iter().take(5) {
                println!("     {} : {} 件", assignee, count);
            }
        }
        Err(e) => {
            println!("   [ERROR] エラー: {}", e);
            println!("   詳細: {:?}", e);
        }
    }

    println!("\n検索サンプル完了!");
    println!("\nその他のサンプル:");
    println!("   cargo run --example basic_usage");
    println!("   cargo run --example project_example");
    
    Ok(())
}