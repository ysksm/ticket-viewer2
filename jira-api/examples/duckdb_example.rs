/// JIRA DuckDBストアの使用例
/// 
/// DuckDBStoreを使ったIssueの保存・読み込み、高度なフィルタリング・ソート機能を示します
/// DuckDBの強力なSQL機能とパフォーマンスを活用したデータ永続化のデモンストレーション
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example duckdb_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, DuckDBStore, PersistenceStore, IssueFilter, SortOrder, DateRange};
use tempfile::TempDir;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("[INFO] JIRA DuckDBストアの使用例");
    println!("================================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. DuckDBストアの初期化（実際にはパーマネントな場所を使用）
    println!("\n[1] DuckDBストアの初期化");
    
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("jira_data.duckdb");
    let mut store = DuckDBStore::new(&db_path)?;
    
    store.initialize().await?;
    println!("    データベースファイル: {:?}", db_path);
    println!("    スキーマ初期化完了");

    // 2. JIRAからIssueを取得して保存
    println!("\n[2] JIRAからIssueを取得してDuckDBに保存");
    
    let jql_query = "ORDER BY created DESC";
    let search_params = jira_api::SearchParams::new()
        .max_results(100)
        .fields(vec![
            "key".to_string(),
            "summary".to_string(),
            "status".to_string(),
            "priority".to_string(),
            "issuetype".to_string(),
            "reporter".to_string(),
            "assignee".to_string(),
            "created".to_string(),
            "updated".to_string(),
            "project".to_string(),
            "description".to_string(),
        ]);
    
    match client.search_issues(jql_query, search_params).await {
        Ok(result) => {
            println!("    JIRAから {} 件のIssueを取得", result.issues.len());
            
            if !result.issues.is_empty() {
                // Issueを保存
                let saved_count = store.save_issues(&result.issues).await?;
                println!("    {} 件のIssueをDuckDBに保存", saved_count);
                
                // 統計情報を表示
                let stats = store.get_stats().await?;
                print_database_stats(&stats);
                
                // 3. DuckDBの高度なフィルタリング機能デモ
                demonstrate_advanced_filtering(&store).await?;
                
                // 4. DuckDBの強力なソート機能デモ
                demonstrate_advanced_sorting(&store).await?;
                
                // 5. DuckDBの分析機能デモ
                demonstrate_analytics_features(&store).await?;
                
                // 6. フィルター設定の永続化デモ
                demonstrate_filter_persistence(&mut store).await?;
                
            } else {
                println!("    [INFO] 取得できるIssueがありませんでした");
                
                // デモ用データを作成
                println!("    DuckDB機能デモ用のサンプルデータを作成します...");
                create_comprehensive_demo_data(&mut store).await?;
                
                demonstrate_advanced_filtering(&store).await?;
                demonstrate_advanced_sorting(&store).await?;
                demonstrate_analytics_features(&store).await?;
                demonstrate_filter_persistence(&mut store).await?;
            }
        }
        Err(e) => {
            println!("    [ERROR] JIRA検索エラー: {}", e);
            println!("    DuckDB機能デモ用のサンプルデータを作成します...");
            
            create_comprehensive_demo_data(&mut store).await?;
            
            demonstrate_advanced_filtering(&store).await?;
            demonstrate_advanced_sorting(&store).await?;
            demonstrate_analytics_features(&store).await?;
            demonstrate_filter_persistence(&mut store).await?;
        }
    }

    // 7. DuckDBストアの最適化デモ
    demonstrate_optimization(&mut store).await?;

    println!("\nDuckDBストアサンプル完了!");
    println!("\n高度な使用例:");
    println!("   // ファイルベースのDuckDBストア");
    println!("   let store = DuckDBStore::new(\\\"/path/to/jira_data.duckdb\\\")?;");
    println!();
    println!("   // インメモリDuckDBストア（テスト用）");
    println!("   let store = DuckDBStore::new_in_memory()?;");
    println!();
    println!("   // 複雑なSQLライクフィルタ");
    println!("   let filter = IssueFilter::new()");
    println!("       .project_keys(vec![\\\"PROJ1\\\".to_string(), \\\"PROJ2\\\".to_string()])");
    println!("       .statuses(vec![\\\"In Progress\\\".to_string()])");
    println!("       .created_range(DateRange::last_days(7))");
    println!("       .summary_contains(\\\"urgent\\\".to_string())");
    println!("       .sort_order(SortOrder::CreatedDesc)");
    println!("       .limit(50);");
    println!();
    println!("その他のサンプル:");
    println!("   cargo run --example basic_usage");
    println!("   cargo run --example persistence_example");
    println!("   cargo run --example sync_example");
    
    Ok(())
}

/// データベース統計情報を表示
fn print_database_stats(stats: &jira_api::StorageStats) {
    println!("\n    [DuckDB統計情報]");
    println!("    総Issue数: {} 件", stats.total_issues);
    println!("    最終更新: {}", stats.last_updated.format("%Y-%m-%d %H:%M:%S"));
    println!("    インデックス数: {}", stats.index_count);
    
    if !stats.issues_by_project.is_empty() {
        println!("    プロジェクト別分析:");
        let mut project_stats: Vec<_> = stats.issues_by_project.iter().collect();
        project_stats.sort_by(|a, b| b.1.cmp(a.1)); // 件数降順
        
        for (project, count) in project_stats.iter().take(5) {
            let percentage = (**count as f64 / stats.total_issues as f64) * 100.0;
            println!("      {}: {} 件 ({:.1}%)", project, count, percentage);
        }
        
        if project_stats.len() > 5 {
            println!("      ... 他 {} プロジェクト", project_stats.len() - 5);
        }
    }
    
    if !stats.issues_by_status.is_empty() {
        println!("    ステータス別分析:");
        let mut status_stats: Vec<_> = stats.issues_by_status.iter().collect();
        status_stats.sort_by(|a, b| b.1.cmp(a.1)); // 件数降順
        
        for (status, count) in status_stats.iter().take(5) {
            let percentage = (**count as f64 / stats.total_issues as f64) * 100.0;
            println!("      {}: {} 件 ({:.1}%)", status, count, percentage);
        }
    }
}

/// 高度なフィルタリング機能のデモ
async fn demonstrate_advanced_filtering(store: &DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[3] DuckDBの高度なフィルタリング機能");
    
    // 全Issue取得
    let all_issues = store.load_all_issues().await?;
    println!("    総Issue数: {} 件", all_issues.len());
    
    if all_issues.is_empty() {
        return Ok(());
    }
    
    // 複合条件でのフィルタリング
    let projects: std::collections::HashSet<String> = all_issues.iter()
        .filter_map(|i| i.fields.project.as_ref().map(|p| p.key.clone()))
        .collect();
    
    if let Some(project_key) = projects.iter().next() {
        let complex_filter = IssueFilter::new()
            .project_keys(vec![project_key.clone()])
            .statuses(vec!["Open".to_string(), "In Progress".to_string()])
            .created_range(DateRange::last_days(30))
            .limit(10);
        
        let complex_results = store.load_issues(&complex_filter).await?;
        println!("    複合条件フィルター (プロジェクト: {}, ステータス: Open/In Progress, 過去30日): {} 件", 
                 project_key, complex_results.len());
        
        for issue in complex_results.iter().take(3) {
            println!("      {} - {} [{}] ({})", 
                issue.key, 
                issue.fields.summary.chars().take(50).collect::<String>(),
                issue.fields.status.name,
                issue.fields.created.format("%Y-%m-%d")
            );
        }
    }
    
    // テキスト検索（大文字小文字を区別しない）
    let text_search_filter = IssueFilter::new()
        .summary_contains("test".to_string())
        .limit(5);
    
    let text_results = store.load_issues(&text_search_filter).await?;
    println!("    テキスト検索 (サマリーに'test'を含む): {} 件", text_results.len());
    
    // 優先度別フィルタリング
    let priority_filter = IssueFilter::new()
        .priorities(vec!["High".to_string(), "Critical".to_string()])
        .limit(5);
    
    let priority_results = store.load_issues(&priority_filter).await?;
    println!("    高優先度Issue (High/Critical): {} 件", priority_results.len());
    
    // 時間範囲フィルタリング（過去7日間）
    let recent_filter = IssueFilter::new()
        .created_range(DateRange::last_days(7))
        .sort_order(SortOrder::CreatedDesc)
        .limit(5);
    
    let recent_results = store.load_issues(&recent_filter).await?;
    println!("    過去7日間のIssue: {} 件", recent_results.len());
    
    Ok(())
}

/// 高度なソート機能のデモ
async fn demonstrate_advanced_sorting(store: &DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[4] DuckDBの強力なソート機能");
    
    let sort_demos = vec![
        (SortOrder::CreatedDesc, "最新作成順", "新しく作成されたIssueから表示"),
        (SortOrder::UpdatedDesc, "最新更新順", "最近更新されたIssueから表示"),
        (SortOrder::PriorityDesc, "優先度順", "高優先度から低優先度の順"),
        (SortOrder::KeyAsc, "キー昇順", "Issue番号の若い順"),
    ];
    
    for (sort_order, title, description) in sort_demos {
        let filter = IssueFilter::new()
            .sort_order(sort_order)
            .limit(3);
        
        let sorted_issues = store.load_issues(&filter).await?;
        
        println!("    {} ({}):", title, description);
        for (i, issue) in sorted_issues.iter().enumerate() {
            let priority = issue.fields.priority.as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("None");
            
            println!("      {}. {} - {} [優先度: {}, 作成: {}]", 
                i + 1,
                issue.key,
                issue.fields.summary.chars().take(40).collect::<String>(),
                priority,
                issue.fields.created.format("%m/%d")
            );
        }
    }
    
    Ok(())
}

/// DuckDBの分析機能デモ
async fn demonstrate_analytics_features(store: &DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[5] DuckDBの分析機能");
    
    // 統計情報による分析
    let stats = store.get_stats().await?;
    
    println!("    データ分析サマリー:");
    println!("      • 総Issue数: {} 件", stats.total_issues);
    println!("      • プロジェクト数: {} 個", stats.issues_by_project.len());
    println!("      • ステータス種類: {} 種類", stats.issues_by_status.len());
    println!("      • Issue種別: {} 種類", stats.issues_by_type.len());
    
    // プロジェクト別分析
    if !stats.issues_by_project.is_empty() {
        println!("\n    プロジェクト別Issue分布:");
        let mut projects: Vec<_> = stats.issues_by_project.iter().collect();
        projects.sort_by(|a, b| b.1.cmp(a.1));
        
        for (project, count) in projects.iter().take(5) {
            let percentage = (**count as f64 / stats.total_issues as f64) * 100.0;
            let bar = "█".repeat((percentage / 5.0).ceil() as usize);
            println!("      {}: {} 件 ({:.1}%) {}", project, count, percentage, bar);
        }
    }
    
    // ステータス別分析
    if !stats.issues_by_status.is_empty() {
        println!("\n    ステータス別Issue分布:");
        let mut statuses: Vec<_> = stats.issues_by_status.iter().collect();
        statuses.sort_by(|a, b| b.1.cmp(a.1));
        
        for (status, count) in statuses.iter().take(3) {
            let percentage = (**count as f64 / stats.total_issues as f64) * 100.0;
            println!("      {}: {} 件 ({:.1}%)", status, count, percentage);
        }
    }
    
    // 時系列分析のデモ
    println!("\n    時系列分析:");
    
    // 最近1週間のIssue作成傾向
    let recent_filter = IssueFilter::new()
        .created_range(DateRange::last_days(7))
        .sort_order(SortOrder::CreatedDesc);
    
    let recent_count = store.count_issues(&recent_filter).await?;
    
    // 過去1ヶ月のIssue作成傾向
    let monthly_filter = IssueFilter::new()
        .created_range(DateRange::last_days(30))
        .sort_order(SortOrder::CreatedDesc);
    
    let monthly_count = store.count_issues(&monthly_filter).await?;
    
    println!("      • 過去7日間の新規Issue: {} 件", recent_count);
    println!("      • 過去30日間の新規Issue: {} 件", monthly_count);
    
    if monthly_count > 0 {
        let weekly_rate = recent_count as f64 / 7.0;
        let monthly_rate = monthly_count as f64 / 30.0;
        println!("      • 週平均作成率: {:.1} 件/日", weekly_rate);
        println!("      • 月平均作成率: {:.1} 件/日", monthly_rate);
    }
    
    Ok(())
}

/// フィルター設定の永続化デモ
async fn demonstrate_filter_persistence(store: &mut DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[6] フィルター設定の永続化");
    
    // 複雑なフィルター設定を作成
    let complex_filter = IssueFilter::new()
        .statuses(vec!["Open".to_string(), "In Progress".to_string()])
        .created_range(DateRange::last_days(14))
        .sort_order(SortOrder::UpdatedDesc)
        .limit(25);
    
    let mut filter_config = jira_api::FilterConfig::new(
        "active_recent_issues".to_string(),
        "アクティブな最近のIssue".to_string(),
        complex_filter,
    ).description("過去2週間に作成され、現在もアクティブなIssueを更新順で表示".to_string());
    
    // フィルター設定を保存
    store.save_filter_config(&filter_config).await?;
    println!("    フィルター設定 '{}' を保存", filter_config.name);
    
    // 使用回数を増加
    filter_config.increment_usage();
    filter_config.increment_usage();
    store.save_filter_config(&filter_config).await?;
    
    // 保存されたフィルター設定を読み込み
    let loaded_config = store.load_filter_config().await?;
    
    if let Some(config) = loaded_config {
        println!("    保存されたフィルター設定を復元:");
        println!("      ID: {}", config.id);
        println!("      名前: {}", config.name);
        println!("      説明: {}", config.description.unwrap_or("なし".to_string()));
        println!("      使用回数: {} 回", config.usage_count);
        println!("      作成日: {}", config.created_at.format("%Y-%m-%d %H:%M"));
        
        // フィルターを適用してデータを取得
        let filtered_results = store.load_issues(&config.filter).await?;
        println!("      フィルター適用結果: {} 件", filtered_results.len());
        
        if !filtered_results.is_empty() {
            println!("      結果プレビュー:");
            for issue in filtered_results.iter().take(3) {
                println!("        {} - {} [{}]", 
                    issue.key,
                    issue.fields.summary.chars().take(40).collect::<String>(),
                    issue.fields.status.name
                );
            }
        }
    }
    
    Ok(())
}

/// DuckDBストアの最適化デモ
async fn demonstrate_optimization(store: &mut DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[7] DuckDBストアの最適化");
    
    println!("    最適化前の統計:");
    let stats_before = store.get_stats().await?;
    println!("      • Issue数: {} 件", stats_before.total_issues);
    println!("      • インデックス数: {}", stats_before.index_count);
    
    // データベース最適化実行
    println!("    DuckDBストレージの最適化を実行中...");
    store.optimize().await?;
    
    println!("    最適化後の統計:");
    let stats_after = store.get_stats().await?;
    println!("      • Issue数: {} 件", stats_after.total_issues);
    println!("      • インデックス数: {}", stats_after.index_count);
    println!("      • 最適化完了時刻: {}", stats_after.last_updated.format("%H:%M:%S"));
    
    // パフォーマンステスト
    println!("\n    パフォーマンステスト:");
    let start_time = std::time::Instant::now();
    
    let perf_filter = IssueFilter::new()
        .sort_order(SortOrder::CreatedDesc)
        .limit(100);
    
    let perf_results = store.load_issues(&perf_filter).await?;
    let duration = start_time.elapsed();
    
    println!("      • 100件取得時間: {:.2}ms", duration.as_millis());
    println!("      • 取得レコード数: {} 件", perf_results.len());
    println!("      • 処理速度: {:.0} 件/秒", 
             perf_results.len() as f64 / duration.as_secs_f64());
    
    Ok(())
}

/// 包括的なデモデータの作成
async fn create_comprehensive_demo_data(store: &mut DuckDBStore) -> Result<(), Box<dyn std::error::Error>> {
    use jira_api::{Issue, IssueFields, Status, StatusCategory, IssueType, Project, User, Priority};
    use chrono::{Utc, Duration};
    
    // より豊富なデモ用のIssueデータを作成
    let mut demo_issues = Vec::new();
    
    let projects = vec![
        ("WEBUI", "Web UI Project"),
        ("MOBILE", "Mobile App Project"), 
        ("API", "Backend API Project"),
        ("INFRA", "Infrastructure Project"),
    ];
    
    let statuses = vec![
        ("Open", "新規"),
        ("In Progress", "進行中"), 
        ("Code Review", "レビュー中"),
        ("Testing", "テスト中"),
        ("Done", "完了"),
    ];
    
    let priorities = vec!["Critical", "High", "Medium", "Low"];
    let issue_types = vec!["Bug", "Task", "Story", "Epic"];
    
    for i in 1..=50 {
        let project = &projects[i % projects.len()];
        let status_info = &statuses[i % statuses.len()];
        let priority = priorities[i % priorities.len()];
        let issue_type = issue_types[i % issue_types.len()];
        
        let status_category = StatusCategory {
            id: 1,
            key: "new".to_string(),
            name: "新規".to_string(),
            color_name: "blue-gray".to_string(),
            self_url: Some("http://example.com".to_string()),
        };
        
        let status = Status {
            id: (i % statuses.len() + 1).to_string(),
            name: status_info.0.to_string(),
            description: None,
            icon_url: None,
            status_category,
            self_url: "http://example.com".to_string(),
        };
        
        let issue_type_obj = IssueType {
            id: (i % issue_types.len() + 1).to_string(),
            name: issue_type.to_string(),
            description: None,
            icon_url: None,
            subtask: Some(false),
            self_url: "http://example.com".to_string(),
        };
        
        let project_obj = Project {
            id: (i % projects.len() + 1).to_string(),
            key: project.0.to_string(),
            name: project.1.to_string(),
            project_type_key: Some("software".to_string()),
            description: None,
            lead: None,
            url: None,
            simplified: None,
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
        };
        
        let reporter = User {
            account_id: format!("user_{}", i % 5 + 1),
            display_name: match i % 5 {
                0 => "Alice Smith",
                1 => "Bob Johnson", 
                2 => "Carol Davis",
                3 => "David Wilson",
                _ => "Eve Brown",
            }.to_string(),
            email_address: Some(format!("user{}@example.com", i % 5 + 1)),
            self_url: "http://example.com".to_string(),
            avatar_urls: None,
            active: Some(true),
            time_zone: None,
            account_type: None,
        };
        
        let assignee = if i % 3 == 0 { None } else { Some(reporter.clone()) };
        
        let priority_obj = Priority {
            id: (i % priorities.len() + 1).to_string(),
            name: priority.to_string(),
            description: None,
            icon_url: None,
            status_color: None,
            self_url: "http://example.com".to_string(),
        };
        
        // 様々な作成日時でリアルなデータを作成
        let created_days_ago = (i % 60) as i64; // 過去60日間にランダム分散
        let created = Utc::now() - Duration::days(created_days_ago);
        let updated = created + Duration::hours((i % 24) as i64);
        
        let summary = match issue_type {
            "Bug" => format!("Fix {} issue in {} module", priority.to_lowercase(), project.0.to_lowercase()),
            "Task" => format!("Implement {} feature for {}", priority.to_lowercase(), project.1),
            "Story" => format!("As a user, I want {} functionality", priority.to_lowercase()),
            "Epic" => format!("{} enhancement project for {}", priority, project.1),
            _ => format!("Demo issue {} - {}", i, issue_type),
        };
        
        let description = Some(format!(
            "This is a {} priority {} for the {} project. \
            Created for demonstration purposes with realistic test data. \
            Issue contains relevant details and follows standard formatting.",
            priority.to_lowercase(), issue_type.to_lowercase(), project.1
        ));
        
        let fields = IssueFields {
            summary,
            description,
            status,
            priority: Some(priority_obj),
            issue_type: issue_type_obj,
            assignee,
            reporter,
            created,
            updated,
            resolution_date: None,
            project: Some(project_obj),
            custom_fields: HashMap::new(),
        };
        
        let issue = Issue {
            id: i.to_string(),
            key: format!("{}-{}", project.0, i),
            fields,
            self_url: "http://example.com".to_string(),
            changelog: None,
        };
        
        demo_issues.push(issue);
    }
    
    let saved_count = store.save_issues(&demo_issues).await?;
    println!("    {} 件の包括的デモデータを作成しました", saved_count);
    
    Ok(())
}