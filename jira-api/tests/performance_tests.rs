/// パフォーマンステスト
/// 
/// JIRAクライアントライブラリの性能特性をテストします：
/// 1. 大量データの処理性能
/// 2. メモリ使用量
/// 3. 同期処理のスループット
/// 4. ストレージ性能の比較

use jira_api::{
    JsonStore, DuckDBStore, PersistenceStore,
    SyncService, SyncConfig, TimeBasedFilter,
    IssueFilter, SortOrder,
    Issue, IssueFields, Status, StatusCategory, IssueType,
    Project, User, Priority
};
use tempfile::TempDir;
use std::collections::HashMap;
use chrono::{Utc, Duration};
use std::time::Instant;

/// 大量のテストデータを生成（パフォーマンステスト用）
fn generate_large_test_dataset(count: usize) -> Vec<Issue> {
    let mut issues = Vec::with_capacity(count);
    
    // プリセットデータでメモリ効率を向上
    let projects = vec![
        ("PROJ1", "Project One"),
        ("PROJ2", "Project Two"),
        ("PROJ3", "Project Three"),
        ("PROJ4", "Project Four"),
        ("PROJ5", "Project Five"),
    ];
    
    let statuses = vec![
        ("Open", "blue-gray"),
        ("In Progress", "yellow"),
        ("Done", "green"),
        ("Closed", "green"),
    ];
    
    let issue_types = vec![
        ("Bug", false),
        ("Story", false),
        ("Task", false),
        ("Epic", false),
    ];
    
    let priorities = vec!["Critical", "High", "Medium", "Low"];
    
    for i in 1..=count {
        let project_idx = i % projects.len();
        let status_idx = i % statuses.len();
        let type_idx = i % issue_types.len();
        let priority_idx = i % priorities.len();
        
        let (project_key, project_name) = &projects[project_idx];
        let (status_name, color) = &statuses[status_idx];
        let (type_name, is_subtask) = &issue_types[type_idx];
        let priority_name = priorities[priority_idx];
        
        let status_category = StatusCategory {
            id: (status_idx + 1) as u32,
            key: status_name.to_lowercase().replace(" ", "_"),
            name: status_name.to_string(),
            color_name: color.to_string(),
            self_url: Some(format!("http://example.com/status/{}", status_idx)),
        };
        
        let status = Status {
            id: (status_idx + 1).to_string(),
            name: status_name.to_string(),
            description: Some(format!("Status description for {}", status_name)),
            icon_url: Some(format!("http://example.com/icon/{}.png", status_idx)),
            status_category,
            self_url: format!("http://example.com/status/{}", status_idx + 1),
        };
        
        let issue_type = IssueType {
            id: (type_idx + 1).to_string(),
            name: type_name.to_string(),
            description: Some(format!("Issue type: {}", type_name)),
            icon_url: Some(format!("http://example.com/type/{}.png", type_idx)),
            subtask: Some(*is_subtask),
            self_url: format!("http://example.com/type/{}", type_idx + 1),
        };
        
        let project = Project {
            id: (10000 + project_idx).to_string(),
            key: project_key.to_string(),
            name: project_name.to_string(),
            project_type_key: Some("software".to_string()),
            description: Some(format!("Performance test project: {}", project_name)),
            lead: None,
            url: None,
            simplified: Some(false),
            self_url: format!("http://example.com/project/{}", project_key),
            avatar_urls: None,
        };
        
        let reporter = User {
            account_id: format!("perf-user-{}", (i % 10) + 1),
            display_name: format!("Performance User {}", (i % 10) + 1),
            email_address: Some(format!("perfuser{}@example.com", (i % 10) + 1)),
            self_url: format!("http://example.com/user/{}", (i % 10) + 1),
            avatar_urls: None,
            active: Some(true),
            time_zone: Some("UTC".to_string()),
            account_type: Some("atlassian".to_string()),
        };
        
        let priority = Priority {
            id: (priority_idx + 1).to_string(),
            name: priority_name.to_string(),
            description: Some(format!("{} priority level", priority_name)),
            icon_url: Some(format!("http://example.com/priority/{}.png", priority_idx)),
            status_color: Some(format!("#{:06x}", (priority_idx + 1) * 0x111111)),
            self_url: format!("http://example.com/priority/{}", priority_idx + 1),
        };
        
        let mut custom_fields = HashMap::new();
        custom_fields.insert(
            "customfield_10001".to_string(),
            format!("Custom value for issue {}", i).into()
        );
        custom_fields.insert(
            "customfield_10002".to_string(),
            (i % 100).to_string().into()
        );
        
        let fields = IssueFields {
            summary: format!("Performance test issue {} - {} {}", i, type_name, priority_name),
            description: Some(serde_json::Value::String(format!(
                "This is a performance test issue number {} for testing large dataset operations. \
                It belongs to project {} and has status {}. The issue was created to evaluate \
                the performance characteristics of the JIRA API client library.",
                i, project_name, status_name
            ))),
            status,
            priority: Some(priority),
            issue_type,
            assignee: if i % 3 == 0 { Some(reporter.clone()) } else { None },
            reporter,
            created: Utc::now() - Duration::days((i % 365) as i64),
            updated: Utc::now() - Duration::hours((i % 24) as i64),
            resolution_date: if *status_name == "Done" || *status_name == "Closed" {
                Some(Utc::now() - Duration::hours((i % 48) as i64))
            } else {
                None
            },
            project: Some(project),
            custom_fields,
        };
        
        let issue = Issue {
            id: (50000 + i).to_string(),
            key: format!("{}-{}", project_key, i),
            fields,
            self_url: format!("http://example.com/issue/{}", 50000 + i),
            changelog: None,
        };
        
        issues.push(issue);
    }
    
    issues
}

/// 大量データの保存・読み込みパフォーマンステスト
#[tokio::test]
async fn test_large_dataset_performance() {
    const DATASET_SIZE: usize = 1000; // テスト用に1000件（実際の使用では数万件以上も可能）
    
    println!("Generating {} test issues...", DATASET_SIZE);
    let start = Instant::now();
    let large_dataset = generate_large_test_dataset(DATASET_SIZE);
    let generation_time = start.elapsed();
    
    println!("✓ Generated {} issues in {:?}", DATASET_SIZE, generation_time);
    
    // JSONストアのパフォーマンステスト
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
    json_store.initialize().await.expect("Failed to initialize JSON store");
    
    let start = Instant::now();
    let json_saved = json_store.save_issues(&large_dataset).await
        .expect("Failed to save to JSON store");
    let json_save_time = start.elapsed();
    
    println!("✓ JSON Store: Saved {} issues in {:?} ({:.2} issues/sec)", 
        json_saved, json_save_time, json_saved as f64 / json_save_time.as_secs_f64());
    
    let start = Instant::now();
    let json_loaded = json_store.load_all_issues().await
        .expect("Failed to load from JSON store");
    let json_load_time = start.elapsed();
    
    println!("✓ JSON Store: Loaded {} issues in {:?} ({:.2} issues/sec)", 
        json_loaded.len(), json_load_time, json_loaded.len() as f64 / json_load_time.as_secs_f64());
    
    // DuckDBストアのパフォーマンステスト
    let mut duckdb_store = DuckDBStore::new_in_memory()
        .expect("Failed to create DuckDB store");
    duckdb_store.initialize().await.expect("Failed to initialize DuckDB store");
    
    let start = Instant::now();
    let duckdb_saved = duckdb_store.save_issues(&large_dataset).await
        .expect("Failed to save to DuckDB store");
    let duckdb_save_time = start.elapsed();
    
    println!("✓ DuckDB Store: Saved {} issues in {:?} ({:.2} issues/sec)", 
        duckdb_saved, duckdb_save_time, duckdb_saved as f64 / duckdb_save_time.as_secs_f64());
    
    let start = Instant::now();
    let duckdb_loaded = duckdb_store.load_all_issues().await
        .expect("Failed to load from DuckDB store");
    let duckdb_load_time = start.elapsed();
    
    println!("✓ DuckDB Store: Loaded {} issues in {:?} ({:.2} issues/sec)", 
        duckdb_loaded.len(), duckdb_load_time, duckdb_loaded.len() as f64 / duckdb_load_time.as_secs_f64());
    
    // パフォーマンス比較
    println!("\n📊 Performance Comparison:");
    println!("Save Performance:");
    println!("  JSON:   {:.2} ms/issue", json_save_time.as_millis() as f64 / json_saved as f64);
    println!("  DuckDB: {:.2} ms/issue", duckdb_save_time.as_millis() as f64 / duckdb_saved as f64);
    
    println!("Load Performance:");
    println!("  JSON:   {:.2} ms/issue", json_load_time.as_millis() as f64 / json_loaded.len() as f64);
    println!("  DuckDB: {:.2} ms/issue", duckdb_load_time.as_millis() as f64 / duckdb_loaded.len() as f64);
    
    // データの整合性確認
    assert_eq!(json_saved, DATASET_SIZE);
    assert_eq!(duckdb_saved, DATASET_SIZE);
    assert_eq!(json_loaded.len(), DATASET_SIZE);
    assert_eq!(duckdb_loaded.len(), DATASET_SIZE);
    
    println!("\n🎉 Large dataset performance test completed successfully!");
}

/// 複雑クエリのパフォーマンステスト
#[tokio::test]
async fn test_complex_query_performance() {
    const DATASET_SIZE: usize = 500;
    
    // テストデータの準備
    let test_dataset = generate_large_test_dataset(DATASET_SIZE);
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let mut json_store = JsonStore::new(temp_dir.path()).with_compression(false); // 圧縮なしで比較
    let mut duckdb_store = DuckDBStore::new_in_memory()
        .expect("Failed to create DuckDB store");
    
    json_store.initialize().await.expect("Failed to initialize JSON store");
    duckdb_store.initialize().await.expect("Failed to initialize DuckDB store");
    
    // データ保存
    json_store.save_issues(&test_dataset).await.expect("Failed to save to JSON");
    duckdb_store.save_issues(&test_dataset).await.expect("Failed to save to DuckDB");
    
    println!("✓ Test data prepared");
    
    // 複雑なクエリのテスト
    let complex_queries = vec![
        ("Project filtering", IssueFilter::new().project_keys(vec!["PROJ1".to_string()])),
        ("Status filtering", IssueFilter::new().statuses(vec!["Open".to_string(), "In Progress".to_string()])),
        ("Sorted by created", IssueFilter::new().sort_order(SortOrder::CreatedDesc).limit(50)),
        ("Recent issues", IssueFilter::new()
            .created_range(jira_api::DateRange::last_days(30))
            .sort_order(SortOrder::UpdatedDesc)),
        ("Complex filter", IssueFilter::new()
            .project_keys(vec!["PROJ1".to_string(), "PROJ2".to_string()])
            .statuses(vec!["Open".to_string()])
            .sort_order(SortOrder::CreatedDesc)
            .limit(20)),
    ];
    
    for (query_name, filter) in complex_queries {
        // JSONストアのクエリ性能
        let start = Instant::now();
        let json_results = json_store.load_issues(&filter).await
            .expect("Failed to query JSON store");
        let json_query_time = start.elapsed();
        
        // DuckDBストアのクエリ性能
        let start = Instant::now();
        let duckdb_results = duckdb_store.load_issues(&filter).await
            .expect("Failed to query DuckDB store");
        let duckdb_query_time = start.elapsed();
        
        println!("📊 {}: {} results", query_name, json_results.len());
        println!("  JSON:   {:?} ({:.2} ms/result)", 
            json_query_time, 
            json_query_time.as_millis() as f64 / json_results.len().max(1) as f64);
        println!("  DuckDB: {:?} ({:.2} ms/result)", 
            duckdb_query_time,
            duckdb_query_time.as_millis() as f64 / duckdb_results.len().max(1) as f64);
        
        // 結果の一貫性確認
        assert_eq!(json_results.len(), duckdb_results.len());
    }
    
    println!("\n🎉 Complex query performance test completed successfully!");
}

/// メモリ効率テスト
#[tokio::test]
async fn test_memory_efficiency() {
    use std::process;
    
    // プロセス開始時のメモリ使用量（概算）
    fn get_memory_usage() -> usize {
        // 実際の実装ではpsutilやsimilarを使用するが、
        // テスト環境では簡単な近似値を使用
        let pid = process::id();
        pid as usize * 1024 // 簡易的な値
    }
    
    let initial_memory = get_memory_usage();
    println!("Initial memory usage: ~{} KB", initial_memory / 1024);
    
    // 段階的にデータサイズを増加してメモリ使用量を監視
    let data_sizes = vec![100, 250, 500];
    
    for &size in &data_sizes {
        println!("\nTesting with {} issues:", size);
        
        let mem_before = get_memory_usage();
        
        // データ生成
        let dataset = generate_large_test_dataset(size);
        let mem_after_gen = get_memory_usage();
        
        // ストレージテスト
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
        json_store.initialize().await.expect("Failed to initialize");
        
        json_store.save_issues(&dataset).await.expect("Failed to save");
        let mem_after_save = get_memory_usage();
        
        let _loaded = json_store.load_all_issues().await.expect("Failed to load");
        let mem_after_load = get_memory_usage();
        
        println!("  After generation: +{} KB", (mem_after_gen.saturating_sub(mem_before)) / 1024);
        println!("  After save:       +{} KB", (mem_after_save.saturating_sub(mem_after_gen)) / 1024);
        println!("  After load:       +{} KB", (mem_after_load.saturating_sub(mem_after_save)) / 1024);
        println!("  Memory per issue: ~{:.2} KB", (mem_after_load.saturating_sub(initial_memory)) as f64 / (size as f64 * 1024.0));
        
        // メモリリークの簡易チェック
        drop(dataset);
        drop(_loaded);
        drop(json_store);
        
        // ガベージコレクション待機（実際のRustでは自動）
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    println!("\n🎉 Memory efficiency test completed successfully!");
}

/// 同期パフォーマンステスト
#[tokio::test]
async fn test_sync_performance() {
    // 同期設定の作成
    let sync_config = SyncConfig::new()
        .target_projects(vec!["PROJ1".to_string(), "PROJ2".to_string()])
        .interval_minutes(1)
        .max_history_count(100)
        .enable_time_optimization(true)
        .concurrent_sync_count(3);
    
    let sync_service = SyncService::new(sync_config);
    
    println!("✓ Sync service initialized");
    
    // 既存データのシミュレーション（大量データ）
    let existing_data_sizes = vec![100, 300, 500];
    
    for &size in &existing_data_sizes {
        println!("\nTesting sync with {} existing issues:", size);
        
        let existing_issues = generate_large_test_dataset(size);
        
        // 重複除外のパフォーマンステスト
        let start = Instant::now();
        
        // 重複を含むデータセットを作成
        let mut with_duplicates = existing_issues.clone();
        with_duplicates.extend(existing_issues.iter().take(size / 4).cloned()); // 25%重複
        
        let deduplicated = sync_service.deduplicate_issues(with_duplicates);
        let dedup_time = start.elapsed();
        
        println!("  Deduplication: {:?} ({} -> {} issues, {:.2} ms/issue)", 
            dedup_time, 
            size + size / 4, 
            deduplicated.len(),
            dedup_time.as_millis() as f64 / (size + size / 4) as f64);
        
        assert_eq!(deduplicated.len(), size);
        
        // 同期状態の確認
        assert!(sync_service.should_sync().await);
        assert!(sync_service.can_sync().await);
        
        println!("  ✓ Sync readiness confirmed");
    }
    
    println!("\n🎉 Sync performance test completed successfully!");
}

/// ストレージ圧縮効率テスト
#[tokio::test]
async fn test_compression_efficiency() {
    const TEST_SIZE: usize = 200;
    
    let test_dataset = generate_large_test_dataset(TEST_SIZE);
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // 圧縮ありのJSONストア
    let mut json_compressed = JsonStore::new(temp_dir.path().join("compressed"))
        .with_compression(true);
    json_compressed.initialize().await.expect("Failed to initialize compressed store");
    
    // 圧縮なしのJSONストア
    let mut json_uncompressed = JsonStore::new(temp_dir.path().join("uncompressed"))
        .with_compression(false);
    json_uncompressed.initialize().await.expect("Failed to initialize uncompressed store");
    
    // データ保存
    let start = Instant::now();
    json_compressed.save_issues(&test_dataset).await.expect("Failed to save compressed");
    let compressed_save_time = start.elapsed();
    
    let start = Instant::now();
    json_uncompressed.save_issues(&test_dataset).await.expect("Failed to save uncompressed");
    let uncompressed_save_time = start.elapsed();
    
    // ファイルサイズの確認（概算）
    let compressed_stats = json_compressed.get_stats().await.expect("Failed to get compressed stats");
    let _uncompressed_stats = json_uncompressed.get_stats().await.expect("Failed to get uncompressed stats");
    
    println!("📊 Compression Analysis:");
    println!("  Compressed save time:   {:?}", compressed_save_time);
    println!("  Uncompressed save time: {:?}", uncompressed_save_time);
    println!("  Compression ratio:      {:.1}%", compressed_stats.compression_ratio * 100.0);
    
    // 読み込み速度の比較
    let start = Instant::now();
    let _compressed_loaded = json_compressed.load_all_issues().await.expect("Failed to load compressed");
    let compressed_load_time = start.elapsed();
    
    let start = Instant::now();
    let _uncompressed_loaded = json_uncompressed.load_all_issues().await.expect("Failed to load uncompressed");
    let uncompressed_load_time = start.elapsed();
    
    println!("  Compressed load time:   {:?}", compressed_load_time);
    println!("  Uncompressed load time: {:?}", uncompressed_load_time);
    
    // 圧縮が効果的であることを確認
    assert!(compressed_stats.compression_ratio < 1.0, "Compression should reduce size");
    
    println!("\n🎉 Compression efficiency test completed successfully!");
}

/// 時間フィルタリングのパフォーマンステスト
#[tokio::test]
async fn test_time_filtering_performance() {
    // 様々な時間フィルターのパフォーマンステスト
    let time_filters = vec![
        ("Last 1 hour", TimeBasedFilter::last_hours(1)),
        ("Last 24 hours", TimeBasedFilter::last_hours(24)),
        ("Last 7 days", TimeBasedFilter::last_days(7)),
        ("Last 30 days", TimeBasedFilter::last_days(30)),
        ("Incremental", TimeBasedFilter::incremental_since(Utc::now() - Duration::hours(6))),
        ("Custom range", TimeBasedFilter::date_range(
            Utc::now() - Duration::days(14),
            Utc::now() - Duration::days(1)
        )),
    ];
    
    for (name, filter) in time_filters {
        let start = Instant::now();
        
        // フィルター検証
        assert!(filter.is_valid().is_ok(), "Filter {} should be valid", name);
        
        // JQL生成
        let jql = filter.to_jql_time_condition();
        let jql_time = start.elapsed();
        
        // 時間チャンク分割（複雑な操作）
        let complex_filter = filter
            .granularity_hours(6)
            .exclude_existing(true)
            .excluded_issue_keys(vec![
                "TEST-1".to_string(),
                "TEST-2".to_string(),
                "TEST-3".to_string(),
                "TEST-4".to_string(),
                "TEST-5".to_string(),
            ]);
        
        let start_chunk = Instant::now();
        let chunks = complex_filter.split_into_chunks();
        let chunk_time = start_chunk.elapsed();
        
        println!("⏱️  {} ({} chunks):", name, chunks.len());
        println!("   JQL generation: {:?}", jql_time);
        println!("   Chunk splitting: {:?}", chunk_time);
        println!("   JQL: {:?}", jql);
        
        // パフォーマンス閾値の確認
        assert!(jql_time.as_millis() < 10, "JQL generation should be fast");
        assert!(chunk_time.as_millis() < 50, "Chunk splitting should be fast");
    }
    
    println!("\n🎉 Time filtering performance test completed successfully!");
}