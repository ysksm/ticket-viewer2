/// JIRA プロジェクト管理の使用例
/// 
/// プロジェクト取得、フィルタリング、プロジェクト固有検索の使い方を示します
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example project_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, ProjectParams, SearchParams};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("[INFO] JIRA プロジェクト管理の使用例");
    println!("===============================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. 基本的なプロジェクト一覧取得
    println!("\n[1] 基本的なプロジェクト一覧");
    match client.get_projects().await {
        Ok(projects) => {
            println!("    総プロジェクト数: {} 個", projects.len());
            
            // プロジェクト種別別統計
            let mut type_counts: HashMap<String, usize> = HashMap::new();
            for project in &projects {
                let project_type = project.project_type_key
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_string();
                *type_counts.entry(project_type).or_insert(0) += 1;
            }
            
            println!("     プロジェクト種別:");
            for (ptype, count) in &type_counts {
                println!("     {} : {} 個", ptype, count);
            }
            
            println!("\n    プロジェクト一覧 (最初の10個):");
            for (i, project) in projects.iter().take(10).enumerate() {
                println!("     {}. {} - {} ({})", 
                    i + 1,
                    project.key, 
                    project.name,
                    project.project_type_key.as_deref().unwrap_or("unknown")
                );
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 2. 詳細情報付きプロジェクト取得
    println!("\n[2] 詳細情報付きプロジェクト取得");
    let detailed_params = ProjectParams::new()
        .expand(vec![
            "lead".to_string(),
            "description".to_string(),
            "url".to_string(),
            "projectCategory".to_string()
        ]);
    
    match client.get_projects_with_params(detailed_params).await {
        Ok(projects) => {
            println!("    詳細情報付きプロジェクト: {} 個", projects.len());
            
            for project in projects.iter().take(5) {
                println!("\n    {} - {}", project.key, project.name);
                
                if let Some(description) = &project.description {
                    let short_desc = if description.len() > 100 {
                        format!("{}...", &description[..100])
                    } else {
                        description.clone()
                    };
                    println!("      説明: {}", short_desc);
                }
                
                if let Some(lead) = &project.lead {
                    println!("      プロジェクトリード: {} ({})", 
                        lead.display_name,
                        lead.email_address.as_deref().unwrap_or("メールなし")
                    );
                }
                
                if let Some(url) = &project.url {
                    println!("      URL: {}", url);
                }
                
                println!("      簡略化モード: {}", 
                    project.simplified.unwrap_or(false)
                );
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 3. 最近使用したプロジェクト
    println!("\n[3] 最近使用したプロジェクト");
    let recent_params = ProjectParams::new()
        .recent(5)
        .expand(vec!["lead".to_string()]);
    
    match client.get_projects_with_params(recent_params).await {
        Ok(projects) => {
            println!("    最近使用したプロジェクト: {} 個", projects.len());
            
            for (i, project) in projects.iter().enumerate() {
                let lead_name = project.lead
                    .as_ref()
                    .map(|l| l.display_name.as_str())
                    .unwrap_or("リードなし");
                    
                println!("     {}. {} - {} (リード: {})", 
                    i + 1,
                    project.key, 
                    project.name,
                    lead_name
                );
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 4. プロジェクト固有のチケット分析
    println!("\n[4] プロジェクト固有のチケット分析");
    
    // まずプロジェクト一覧を取得
    match client.get_projects().await {
        Ok(projects) if !projects.is_empty() => {
            // 最初の3プロジェクトについて分析
            for project in projects.iter().take(3) {
                println!("\n    プロジェクト {} ({}) の分析:", 
                    project.key, 
                    project.name
                );
                
                // プロジェクトのチケット総数を取得
                let count_params = SearchParams::new()
                    .max_results(1)
                    .fields(vec![
                        "key".to_string(),
                        "summary".to_string(),
                        "status".to_string(),
                        "reporter".to_string(),
                        "created".to_string(),
                        "updated".to_string(),
                        "issuetype".to_string()
                    ]);
                
                let jql = format!("project = {}", project.key);
                match client.search_issues(&jql, count_params).await {
                    Ok(result) => {
                        println!("      総チケット数: {} 件", result.total);
                        
                        if result.total > 0 {
                            // ステータス分布を取得
                            let status_params = SearchParams::new()
                                .max_results(50)
                                .fields(vec![
                                    "key".to_string(),
                                    "summary".to_string(),
                                    "status".to_string(),
                                    "priority".to_string(),
                                    "issuetype".to_string(),
                                    "reporter".to_string(),
                                    "created".to_string(),
                                    "updated".to_string()
                                ]);
                            
                            match client.search_issues(&jql, status_params).await {
                                Ok(detailed_result) => {
                                    // ステータス統計
                                    let mut status_counts: HashMap<String, usize> = HashMap::new();
                                    let mut priority_counts: HashMap<String, usize> = HashMap::new();
                                    let mut type_counts: HashMap<String, usize> = HashMap::new();
                                    
                                    for issue in &detailed_result.issues {
                                        *status_counts.entry(issue.fields.status.name.clone()).or_insert(0) += 1;
                                        
                                        if let Some(priority) = &issue.fields.priority {
                                            *priority_counts.entry(priority.name.clone()).or_insert(0) += 1;
                                        }
                                        
                                        *type_counts.entry(issue.fields.issue_type.name.clone()).or_insert(0) += 1;
                                    }
                                    
                                    println!("       ステータス分布:");
                                    for (status, count) in status_counts.iter().take(3) {
                                        println!("       {} : {} 件", status, count);
                                    }
                                    
                                    if !priority_counts.is_empty() {
                                        println!("      優先度分布:");
                                        for (priority, count) in priority_counts.iter().take(3) {
                                            println!("       {} : {} 件", priority, count);
                                        }
                                    }
                                    
                                    println!("      チケット種別:");
                                    for (issue_type, count) in type_counts.iter().take(3) {
                                        println!("       {} : {} 件", issue_type, count);
                                    }
                                }
                                Err(e) => println!("     [ERROR] 詳細分析エラー: {}", e),
                            }
                            
                            // 最新チケットを表示
                            let latest_params = SearchParams::new()
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
                            
                            let latest_jql = format!("project = {} ORDER BY created DESC", project.key);
                            match client.search_issues(&latest_jql, latest_params).await {
                                Ok(latest_result) => {
                                    println!("      最新チケット:");
                                    for issue in &latest_result.issues {
                                        println!("       {} - {} [{}]",
                                            issue.key,
                                            issue.fields.summary,
                                            issue.fields.status.name
                                        );
                                    }
                                }
                                Err(e) => println!("     [ERROR] 最新チケット取得エラー: {}", e),
                            }
                        } else {
                            println!("      チケットが存在しません");
                        }
                    }
                    Err(e) => println!("     [ERROR] チケット数取得エラー: {}", e),
                }
            }
        }
        Ok(_) => println!("   [WARNING]  プロジェクトが見つかりません"),
        Err(e) => println!("   [ERROR] プロジェクト取得エラー: {}", e),
    }

    // 5. プロジェクト検索とフィルタリング
    println!("\n[5] プロジェクト検索の応用例");
    
    match client.get_projects().await {
        Ok(projects) => {
            // ソフトウェアプロジェクトのフィルタリング
            let software_projects: Vec<_> = projects.iter()
                .filter(|p| p.project_type_key.as_deref() == Some("software"))
                .collect();
                
            println!("    ソフトウェアプロジェクト: {} 個", software_projects.len());
            for project in software_projects.iter().take(3) {
                println!("     {} - {}", project.key, project.name);
            }
            
            // 名前に特定の文字列を含むプロジェクト
            let test_projects: Vec<_> = projects.iter()
                .filter(|p| p.name.to_lowercase().contains("test") || 
                           p.name.to_lowercase().contains("demo"))
                .collect();
                
            if !test_projects.is_empty() {
                println!("\n    テスト・デモプロジェクト: {} 個", test_projects.len());
                for project in &test_projects {
                    println!("     {} - {}", project.key, project.name);
                }
            }
            
            // プロジェクトキー別ソート
            let mut sorted_projects = projects.clone();
            sorted_projects.sort_by(|a, b| a.key.cmp(&b.key));
            
            println!("\n    キー順プロジェクト (最初の5個):");
            for project in sorted_projects.iter().take(5) {
                println!("     {} - {}", project.key, project.name);
            }
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    // 6. プロジェクト情報の完全性チェック
    println!("\n[6] プロジェクト情報の完全性チェック");
    
    let complete_params = ProjectParams::new()
        .expand(vec!["lead".to_string(), "description".to_string()]);
    
    match client.get_projects_with_params(complete_params).await {
        Ok(projects) => {
            let mut complete_count = 0;
            let mut missing_lead = 0;
            let mut missing_description = 0;
            
            for project in &projects {
                let mut is_complete = true;
                
                if project.lead.is_none() {
                    missing_lead += 1;
                    is_complete = false;
                }
                
                if project.description.is_none() || 
                   project.description.as_ref().map_or(true, |d| d.is_empty()) {
                    missing_description += 1;
                    is_complete = false;
                }
                
                if is_complete {
                    complete_count += 1;
                }
            }
            
            println!("    プロジェクト情報完全性レポート:");
            println!("     [OK] 完全なプロジェクト: {} 個", complete_count);
            println!("      リードが未設定: {} 個", missing_lead);
            println!("      説明が未設定: {} 個", missing_description);
            println!("      完全性: {:.1}%", 
                (complete_count as f64 / projects.len() as f64) * 100.0
            );
        }
        Err(e) => println!("   [ERROR] エラー: {}", e),
    }

    println!("\nプロジェクト管理サンプル完了!");
    println!("\nその他のサンプル:");
    println!("   cargo run --example basic_usage");
    println!("   cargo run --example search_example");
    
    Ok(())
}