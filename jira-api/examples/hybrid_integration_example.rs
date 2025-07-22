use chrono::{Duration, Utc};
/// ハイブリッド統合テストのサンプル
///
/// このサンプルは、モックデータと実際のJIRA APIの両方に対応し、
/// 環境に応じて動作モードを切り替えることができます。
///
/// モックモードでの実行方法:
/// ```
/// cargo run --example hybrid_integration_example
/// ```
///
/// 実APIモードでの実行方法:
/// ```
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// export USE_REAL_JIRA_API=true
/// cargo run --example hybrid_integration_example
/// ```
use dotenv::dotenv;
use jira_api::{
    Auth, DuckDBStore, Issue, IssueFields, IssueFilter, IssueType, JiraClient, JiraConfig,
    JsonStore, PersistenceStore, Priority, Project, SearchParams, SortOrder, Status,
    StatusCategory, SyncConfig, SyncService, User,
};
use std::collections::HashMap;
use tempfile::TempDir;

/// 実際のAPIを使用するかどうかを判定
fn is_using_real_api() -> bool {
    std::env::var("USE_REAL_JIRA_API")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false)
}

/// モック用のIssueデータを生成
fn create_mock_issues(count: usize) -> Vec<Issue> {
    let mut issues = Vec::new();

    for i in 1..=count {
        let status_category = StatusCategory {
            id: 1,
            key: "done".to_string(),
            name: "Done".to_string(),
            color_name: "green".to_string(),
            self_url: Some("https://mock.example.com/status/1".to_string()),
        };

        let status = Status {
            id: (i % 3 + 1).to_string(),
            name: match i % 3 {
                0 => "Done",
                1 => "In Progress",
                _ => "Open",
            }
            .to_string(),
            description: Some(format!("Status for issue {}", i)),
            icon_url: None,
            status_category,
            self_url: format!("https://mock.example.com/status/{}", i % 3 + 1),
        };

        let issue_type = IssueType {
            id: (i % 2 + 1).to_string(),
            name: if i % 2 == 0 { "Bug" } else { "Story" }.to_string(),
            description: Some(format!("Issue type for {}", i)),
            icon_url: None,
            subtask: Some(false),
            self_url: format!("https://mock.example.com/type/{}", i % 2 + 1),
        };

        let project = Project {
            id: "10000".to_string(),
            key: "HYBRID".to_string(),
            name: "Hybrid Test Project".to_string(),
            project_type_key: Some("software".to_string()),
            description: Some("Project for hybrid testing".to_string()),
            lead: None,
            url: None,
            simplified: None,
            self_url: "https://mock.example.com/project/HYBRID".to_string(),
            avatar_urls: None,
        };

        let reporter = User {
            account_id: format!("hybrid-user-{}", i % 3),
            display_name: format!("Hybrid User {}", i % 3),
            email_address: Some(format!("hybrid{}@example.com", i % 3)),
            self_url: format!("https://mock.example.com/user/{}", i % 3),
            avatar_urls: None,
            active: Some(true),
            time_zone: Some("UTC".to_string()),
            account_type: Some("atlassian".to_string()),
        };

        let priority = Priority {
            id: (i % 3 + 1).to_string(),
            name: match i % 3 {
                0 => "High",
                1 => "Medium",
                _ => "Low",
            }
            .to_string(),
            description: Some(format!(
                "{} priority issue",
                match i % 3 {
                    0 => "High",
                    1 => "Medium",
                    _ => "Low",
                }
            )),
            icon_url: None,
            status_color: None,
            self_url: format!("https://mock.example.com/priority/{}", i % 3 + 1),
        };

        let fields = IssueFields {
            summary: format!(
                "Hybrid test issue {} - {}",
                i,
                match i % 3 {
                    0 => "Critical bug",
                    1 => "Feature request",
                    _ => "General task",
                }
            ),
            description: Some(serde_json::Value::String(format!(
                "This is hybrid test issue number {} created for demonstrating \
                the ability to switch between mock and real JIRA API modes seamlessly.",
                i
            ))),
            status,
            priority: Some(priority),
            issue_type,
            assignee: None,
            reporter,
            created: Utc::now() - Duration::days((i % 30) as i64),
            updated: Utc::now() - Duration::hours((i % 24) as i64),
            resolution_date: if i % 4 == 0 {
                Some(Utc::now() - Duration::hours((i % 12) as i64))
            } else {
                None
            },
            project: Some(project),
            custom_fields: {
                let mut fields = HashMap::new();
                fields.insert(
                    "customfield_10001".to_string(),
                    format!("Hybrid value {}", i).into(),
                );
                fields.insert("customfield_10002".to_string(), (i % 5).to_string().into());
                fields
            },
        };

        let issue = Issue {
            id: (30000 + i).to_string(),
            key: format!("HYBRID-{}", i),
            fields,
            self_url: format!("https://mock.example.com/issue/{}", 30000 + i),
            changelog: None,
        };

        issues.push(issue);
    }

    issues
}

/// 実際のJIRA APIを使用した統合テスト
async fn run_real_api_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Running integration test with REAL JIRA API");

    // 1. クライアント初期化
    let config = JiraConfig::from_env().map_err(
        |_| "Environment variables not set. Please check JIRA_URL, JIRA_USER, JIRA_API_TOKEN",
    )?;
    let client = JiraClient::new(config)?;

    println!("✅ JIRA client initialized successfully");

    // 2. プロジェクト一覧取得
    let projects = client.get_projects().await?;
    println!("✅ Retrieved {} projects from JIRA", projects.len());

    if let Some(first_project) = projects.first() {
        println!(
            "   First project: {} ({})",
            first_project.name, first_project.key
        );

        // 3. プロジェクト固有のIssue検索
        let jql = format!("project = {} ORDER BY created DESC", first_project.key);
        let params = SearchParams::new().max_results(10);
        // fieldsパラメータを指定しない場合、JIRAのデフォルトフィールドが返される

        let search_result = client.search_issues(&jql, params).await?;
        println!(
            "✅ Found {} issues in project {}",
            search_result.total, first_project.key
        );

        // 4. 最初の数件を表示
        for (i, issue) in search_result.issues.iter().take(3).enumerate() {
            let priority = issue
                .fields
                .priority
                .as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("None");

            println!(
                "   {}. {} - {} [{}] (Priority: {})",
                i + 1,
                issue.key,
                issue.fields.summary.chars().take(50).collect::<String>(),
                issue.fields.status.name,
                priority
            );
        }

        // 5. データ永続化のテスト
        if !search_result.issues.is_empty() {
            let temp_dir = TempDir::new()?;
            let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
            json_store.initialize().await?;

            let saved_count = json_store.save_issues(&search_result.issues).await?;
            println!("✅ Saved {} real issues to JSON store", saved_count);

            let loaded_issues = json_store.load_all_issues().await?;
            println!("✅ Loaded {} issues from JSON store", loaded_issues.len());
            assert_eq!(saved_count, loaded_issues.len());
        }
    }

    // 6. その他のAPIエンドポイントのテスト
    let priorities = client.get_priorities().await?;
    println!("✅ Retrieved {} priorities", priorities.len());

    let issue_types = client.get_issue_types().await?;
    println!("✅ Retrieved {} issue types", issue_types.len());

    Ok(())
}

/// モックデータを使用した統合テスト
async fn run_mock_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Running integration test with MOCK data");

    // 1. モッククライアント初期化
    let config = JiraConfig::new(
        "https://mock-jira.example.com".to_string(),
        Auth::Basic {
            username: "hybrid@example.com".to_string(),
            api_token: "mock-token-12345".to_string(),
        },
    )?;
    let _client = JiraClient::new(config)?;

    println!("✅ Mock JIRA client initialized successfully");

    // 2. モックデータ生成
    let mock_issues = create_mock_issues(25);
    println!("✅ Generated {} mock issues", mock_issues.len());

    // 3. データ永続化の統合テスト
    let temp_dir = TempDir::new()?;

    // JSONストアのテスト
    let mut json_store = JsonStore::new(temp_dir.path().join("json")).with_compression(true);
    json_store.initialize().await?;

    let json_saved = json_store.save_issues(&mock_issues).await?;
    println!("✅ Saved {} mock issues to JSON store", json_saved);

    // DuckDBストアのテスト
    let mut duckdb_store = DuckDBStore::new_in_memory()?;
    duckdb_store.initialize().await?;

    let duckdb_saved = duckdb_store.save_issues(&mock_issues).await?;
    println!("✅ Saved {} mock issues to DuckDB store", duckdb_saved);

    // 4. フィルタリングのテスト
    let high_priority_filter = IssueFilter::new()
        .statuses(vec!["Open".to_string(), "In Progress".to_string()])
        .sort_order(SortOrder::CreatedDesc)
        .limit(5);

    let json_filtered = json_store.load_issues(&high_priority_filter).await?;
    let duckdb_filtered = duckdb_store.load_issues(&high_priority_filter).await?;

    println!(
        "✅ JSON store filtered results: {} issues",
        json_filtered.len()
    );
    println!(
        "✅ DuckDB store filtered results: {} issues",
        duckdb_filtered.len()
    );

    // 結果の整合性確認
    assert_eq!(json_filtered.len(), duckdb_filtered.len());

    // 5. 同期機能のテスト
    let sync_config = SyncConfig::new()
        .target_projects(vec!["HYBRID".to_string()])
        .interval_minutes(30)
        .max_history_count(50);

    let sync_service = SyncService::new(sync_config);

    // 重複除外のテスト
    let mut with_duplicates = mock_issues.clone();
    with_duplicates.extend(mock_issues.iter().take(5).cloned());

    let deduplicated = sync_service.deduplicate_issues(with_duplicates);
    println!(
        "✅ Deduplication: {} -> {} issues",
        mock_issues.len() + 5,
        deduplicated.len()
    );
    assert_eq!(deduplicated.len(), mock_issues.len());

    // 6. 統計情報の確認
    let json_stats = json_store.get_stats().await?;
    let duckdb_stats = duckdb_store.get_stats().await?;

    println!(
        "✅ JSON store stats: {} issues, {} projects",
        json_stats.total_issues,
        json_stats.issues_by_project.len()
    );
    println!(
        "✅ DuckDB store stats: {} issues, {} projects",
        duckdb_stats.total_issues,
        duckdb_stats.issues_by_project.len()
    );

    // 統計の整合性確認
    assert_eq!(json_stats.total_issues, duckdb_stats.total_issues);
    assert_eq!(
        json_stats.issues_by_project.len(),
        duckdb_stats.issues_by_project.len()
    );

    // 7. 個別Issueの詳細表示
    println!("\n📝 Sample mock issues:");
    for (i, issue) in mock_issues.iter().take(3).enumerate() {
        let priority = issue
            .fields
            .priority
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("None");

        println!(
            "   {}. {} - {} [{}] (Priority: {})",
            i + 1,
            issue.key,
            issue.fields.summary.chars().take(60).collect::<String>(),
            issue.fields.status.name,
            priority
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!("🚀 Hybrid Integration Example");
    println!("==============================");

    // 実行モードの判定と表示
    if is_using_real_api() {
        println!("🔗 Mode: REAL JIRA API");
        println!("   Connecting to actual JIRA instance...");

        match run_real_api_integration().await {
            Ok(()) => {
                println!("\n🎉 Real API integration test completed successfully!");
            }
            Err(e) => {
                eprintln!("\n❌ Real API integration test failed: {}", e);
                eprintln!("   Please check your environment variables:");
                eprintln!("   - JIRA_URL");
                eprintln!("   - JIRA_USER");
                eprintln!("   - JIRA_API_TOKEN");
                return Err(e);
            }
        }
    } else {
        println!("🧪 Mode: MOCK DATA");
        println!("   Using generated mock data for testing...");

        match run_mock_integration().await {
            Ok(()) => {
                println!("\n🎉 Mock integration test completed successfully!");
            }
            Err(e) => {
                eprintln!("\n❌ Mock integration test failed: {}", e);
                return Err(e);
            }
        }
    }

    println!("\n💡 Tips:");
    println!("   - Set USE_REAL_JIRA_API=true to test with real JIRA API");
    println!("   - Remove USE_REAL_JIRA_API or set it to false to use mock data");
    println!("   - This example demonstrates the flexibility of the library");
    println!("     to work in different environments and testing scenarios");

    Ok(())
}
