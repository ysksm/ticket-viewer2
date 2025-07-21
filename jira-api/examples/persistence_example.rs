/// JIRA データ永続化の使用例
/// 
/// JSONストアを使ったIssueの保存・読み込み、フィルタリング機能を示します
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example persistence_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, JsonStore, PersistenceStore, IssueFilter, SortOrder, DateRange};
use tempfile::TempDir;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("[INFO] JIRA データ永続化の使用例");
    println!("==============================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. 一時ディレクトリでJSONストアを作成（実際にはパーマネントな場所を使用）
    println!("\n[1] JSONストアの初期化");
    
    let temp_dir = TempDir::new()?;
    let mut store = JsonStore::new(temp_dir.path())
        .with_compression(true);
    
    store.initialize().await?;
    println!("    ストレージディレクトリ: {:?}", temp_dir.path());
    println!("    gzip圧縮: 有効");

    // 2. JIRAからIssueを取得して保存
    println!("\n[2] JIRAからIssueを取得して保存");
    
    let jql_query = "ORDER BY created DESC";
    let search_params = jira_api::SearchParams::new()
        .max_results(50)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "priority".to_string(),
            "issuetype".to_string(),
            "reporter".to_string(),
            "created".to_string(),
            "updated".to_string(),
            "project".to_string(),
        ]);
    
    match client.search_issues(jql_query, search_params).await {
        Ok(result) => {
            println!("    JIRAから {} 件のIssueを取得", result.issues.len());
            
            if !result.issues.is_empty() {
                // Issueを保存
                let saved_count = store.save_issues(&result.issues).await?;
                println!("    {} 件のIssueをストレージに保存", saved_count);
                
                // 統計情報を表示
                let stats = store.get_stats().await?;
                print_storage_stats(&stats);
                
                // 3. フィルタリングによる検索デモ
                demonstrate_filtering(&store).await?;
                
                // 4. ソート機能のデモ
                demonstrate_sorting(&store).await?;
                
                // 5. データの管理機能デモ
                demonstrate_data_management(&mut store).await?;
                
            } else {
                println!("    [INFO] 取得できるIssueがありませんでした");
                
                // デモ用データを作成
                println!("    デモ用のサンプルデータを作成します...");
                create_demo_data(&mut store).await?;
                
                demonstrate_filtering(&store).await?;
                demonstrate_sorting(&store).await?;
                demonstrate_data_management(&mut store).await?;
            }
        }
        Err(e) => {
            println!("    [ERROR] JIRA検索エラー: {}", e);
            println!("    デモ用のサンプルデータを作成します...");
            
            create_demo_data(&mut store).await?;
            
            demonstrate_filtering(&store).await?;
            demonstrate_sorting(&store).await?;
            demonstrate_data_management(&mut store).await?;
        }
    }

    println!("\nデータ永続化サンプル完了!");
    println!("\n高度な使用例:");
    println!("   // 圧縮無しのJSONストア");
    println!("   let store = JsonStore::new(\"/path/to/data\").with_compression(false);");
    println!();
    println!("   // 複雑なフィルタ条件");
    println!("   let filter = IssueFilter::new()");
    println!("       .project_keys(vec![\"PROJECT1\".to_string()])");
    println!("       .statuses(vec![\"In Progress\".to_string(), \"Done\".to_string()])");
    println!("       .created_range(DateRange::last_days(30))");
    println!("       .summary_contains(\"bug\".to_string())");
    println!("       .sort_order(SortOrder::UpdatedDesc)");
    println!("       .limit(100);");
    println!();
    println!("その他のサンプル:");
    println!("   cargo run --example basic_usage");
    println!("   cargo run --example sync_example");
    
    Ok(())
}

/// ストレージ統計情報を表示
fn print_storage_stats(stats: &jira_api::StorageStats) {
    println!("\n    [ストレージ統計]");
    println!("    総Issue数: {} 件", stats.total_issues);
    println!("    最終更新: {}", stats.last_updated.format("%Y-%m-%d %H:%M:%S"));
    println!("    圧縮率: {:.1}%", stats.compression_ratio * 100.0);
    
    if !stats.issues_by_project.is_empty() {
        println!("    プロジェクト別:");
        let mut project_stats: Vec<_> = stats.issues_by_project.iter().collect();
        project_stats.sort_by(|a, b| b.1.cmp(a.1)); // 件数降順
        
        for (project, count) in project_stats.iter().take(5) {
            println!("      {}: {} 件", project, count);
        }
        
        if project_stats.len() > 5 {
            println!("      ... 他 {} プロジェクト", project_stats.len() - 5);
        }
    }
    
    if !stats.issues_by_status.is_empty() {
        println!("    ステータス別:");
        let mut status_stats: Vec<_> = stats.issues_by_status.iter().collect();
        status_stats.sort_by(|a, b| b.1.cmp(a.1)); // 件数降順
        
        for (status, count) in status_stats.iter().take(3) {
            println!("      {}: {} 件", status, count);
        }
        
        if status_stats.len() > 3 {
            println!("      ... 他 {} ステータス", status_stats.len() - 3);
        }
    }
}

/// フィルタリング機能のデモ
async fn demonstrate_filtering(store: &JsonStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[3] フィルタリング機能のデモ");
    
    // 全Issue取得
    let all_issues = store.load_all_issues().await?;
    println!("    総Issue数: {} 件", all_issues.len());
    
    if all_issues.is_empty() {
        return Ok(());
    }
    
    // プロジェクト別フィルタリング
    let projects: std::collections::HashSet<String> = all_issues.iter()
        .filter_map(|i| i.fields.project.as_ref().map(|p| p.key.clone()))
        .collect();
    
    if let Some(project_key) = projects.iter().next() {
        let filter = IssueFilter::new()
            .project_keys(vec![project_key.clone()]);
        
        let filtered_issues = store.load_issues(&filter).await?;
        println!("    プロジェクト '{}' のIssue: {} 件", project_key, filtered_issues.len());
        
        for issue in filtered_issues.iter().take(3) {
            println!("      {} - {} [{}]", 
                issue.key, 
                issue.fields.summary, 
                issue.fields.status.name
            );
        }
    }
    
    // ステータス別フィルタリング
    let statuses: std::collections::HashSet<String> = all_issues.iter()
        .map(|i| i.fields.status.name.clone())
        .collect();
    
    if let Some(status) = statuses.iter().next() {
        let filter = IssueFilter::new()
            .statuses(vec![status.clone()]);
        
        let filtered_issues = store.load_issues(&filter).await?;
        println!("    ステータス '{}' のIssue: {} 件", status, filtered_issues.len());
    }
    
    // 時間範囲フィルタリング
    let filter = IssueFilter::new()
        .created_range(DateRange::last_days(30))
        .limit(10);
    
    let recent_issues = store.load_issues(&filter).await?;
    println!("    最近30日のIssue: {} 件", recent_issues.len());
    
    // テキスト検索
    let filter = IssueFilter::new()
        .summary_contains("test".to_string())
        .limit(5);
    
    let text_filtered_issues = store.load_issues(&filter).await?;
    println!("    サマリーに'test'を含むIssue: {} 件", text_filtered_issues.len());
    
    Ok(())
}

/// ソート機能のデモ
async fn demonstrate_sorting(store: &JsonStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[4] ソート機能のデモ");
    
    let sort_orders = vec![
        (SortOrder::CreatedDesc, "作成日時降順"),
        (SortOrder::UpdatedDesc, "更新日時降順"),
        (SortOrder::KeyAsc, "キー昇順"),
        (SortOrder::PriorityDesc, "優先度降順"),
    ];
    
    for (sort_order, description) in sort_orders {
        let filter = IssueFilter::new()
            .sort_order(sort_order)
            .limit(5);
        
        let sorted_issues = store.load_issues(&filter).await?;
        
        println!("    {} (上位5件):", description);
        for (i, issue) in sorted_issues.iter().enumerate() {
            let priority = issue.fields.priority.as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("None");
            
            println!("      {}. {} - {} [優先度: {}]", 
                i + 1,
                issue.key, 
                issue.fields.summary.chars().take(40).collect::<String>(),
                priority
            );
        }
    }
    
    Ok(())
}

/// データ管理機能のデモ
async fn demonstrate_data_management(store: &mut JsonStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[5] データ管理機能のデモ");
    
    // 削除前の件数
    let before_count = store.count_issues(&IssueFilter::new()).await?;
    println!("    削除前のIssue数: {} 件", before_count);
    
    // 最初の2件のキーを取得
    let issues_to_delete = store.load_issues(&IssueFilter::new().limit(2)).await?;
    
    if !issues_to_delete.is_empty() {
        let keys_to_delete: Vec<String> = issues_to_delete.iter()
            .map(|i| i.key.clone())
            .collect();
        
        println!("    削除対象: {:?}", keys_to_delete);
        
        let deleted_count = store.delete_issues(&keys_to_delete).await?;
        println!("    削除されたIssue数: {} 件", deleted_count);
        
        let after_count = store.count_issues(&IssueFilter::new()).await?;
        println!("    削除後のIssue数: {} 件", after_count);
    }
    
    // ストレージ最適化
    println!("\n    ストレージ最適化を実行中...");
    store.optimize().await?;
    
    // 最終統計
    let final_stats = store.get_stats().await?;
    println!("    最終統計:");
    println!("      総Issue数: {} 件", final_stats.total_issues);
    println!("      プロジェクト数: {} 個", final_stats.issues_by_project.len());
    println!("      ステータス種別数: {} 個", final_stats.issues_by_status.len());
    
    Ok(())
}

/// デモ用データの作成
async fn create_demo_data(store: &mut JsonStore) -> Result<(), Box<dyn std::error::Error>> {
    use jira_api::{Issue, IssueFields, Status, StatusCategory, IssueType, Project, User, Priority};
    use chrono::Utc;
    
    // デモ用のIssueデータを作成
    let mut demo_issues = Vec::new();
    
    for i in 1..=10 {
        let status_category = StatusCategory {
            id: 1,
            key: "new".to_string(),
            name: "新規".to_string(),
            color_name: "blue-gray".to_string(),
            self_url: Some("http://example.com".to_string()),
        };
        
        let status = Status {
            id: "1".to_string(),
            name: if i <= 3 { "Open" } else if i <= 7 { "In Progress" } else { "Done" }.to_string(),
            description: None,
            icon_url: None,
            status_category,
            self_url: "http://example.com".to_string(),
        };
        
        let issue_type = IssueType {
            id: "1".to_string(),
            name: if i % 2 == 0 { "Bug" } else { "Task" }.to_string(),
            description: None,
            icon_url: None,
            subtask: Some(false),
            self_url: "http://example.com".to_string(),
        };
        
        let project = Project {
            id: "1".to_string(),
            key: if i <= 5 { "DEMO" } else { "TEST" }.to_string(),
            name: if i <= 5 { "Demo Project" } else { "Test Project" }.to_string(),
            project_type_key: Some("software".to_string()),
            description: None,
            lead: None,
            url: None,
            simplified: None,
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
        };
        
        let reporter = User {
            account_id: "demo_user".to_string(),
            display_name: "Demo User".to_string(),
            email_address: Some("demo@example.com".to_string()),
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
            active: Some(true),
            time_zone: None,
            account_type: None,
        };
        
        let priority = Priority {
            id: "1".to_string(),
            name: match i % 3 {
                0 => "Low",
                1 => "Medium",
                _ => "High",
            }.to_string(),
            description: None,
            icon_url: None,
            status_color: None,
            self_url: "http://example.com".to_string(),
        };
        
        let fields = IssueFields {
            summary: format!("Demo issue {} - {}", i, if i % 2 == 0 { "bug fix" } else { "new feature" }),
            description: Some(format!("This is a demo issue for testing purposes: {}", i)),
            status,
            priority: Some(priority),
            issue_type,
            assignee: None,
            reporter,
            created: Utc::now() - chrono::Duration::days(i as i64),
            updated: Utc::now() - chrono::Duration::hours(i as i64),
            resolution_date: None,
            project: Some(project),
            custom_fields: HashMap::new(),
        };
        
        let issue = Issue {
            id: i.to_string(),
            key: format!("{}-{}", if i <= 5 { "DEMO" } else { "TEST" }, i),
            fields,
            self_url: "http://example.com".to_string(),
            changelog: None,
        };
        
        demo_issues.push(issue);
    }
    
    let saved_count = store.save_issues(&demo_issues).await?;
    println!("    {} 件のデモデータを作成しました", saved_count);
    
    Ok(())
}