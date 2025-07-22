# 使用例

このドキュメントでは、`jira-api`ライブラリの様々な使用例を示します。

## 目次

1. [基本的な使用方法](#基本的な使用方法)
2. [認証設定](#認証設定)
3. [Issue検索](#issue検索)
4. [メタデータ取得](#メタデータ取得)
5. [データ永続化](#データ永続化)
6. [同期機能](#同期機能)
7. [変更履歴](#変更履歴)
8. [設定管理](#設定管理)
9. [高度な使用例](#高度な使用例)

## 基本的な使用方法

### 最小限の例

```rust
use jira_api::{JiraConfig, JiraClient, Auth};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 設定作成
    let config = JiraConfig::new(
        "https://your-instance.atlassian.net".to_string(),
        Auth::Basic {
            username: "your-email@example.com".to_string(),
            api_token: "your-api-token".to_string(),
        },
    )?;
    
    // クライアント初期化
    let client = JiraClient::new(config)?;
    
    // プロジェクト一覧取得
    let projects = client.get_projects().await?;
    println!("Found {} projects", projects.len());
    
    Ok(())
}
```

### 環境変数を使用した設定

```bash
export JIRA_URL=https://your-instance.atlassian.net
export JIRA_USER=your-email@example.com
export JIRA_API_TOKEN=your-api-token
```

```rust
use jira_api::{JiraConfig, JiraClient};
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // .envファイルから環境変数を読み込み
    dotenv::dotenv().ok();
    
    // 環境変数から設定を作成
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    
    let projects = client.get_projects().await?;
    println!("Found {} projects", projects.len());
    
    Ok(())
}
```

## 認証設定

### Basic認証

```rust
use jira_api::{JiraConfig, Auth};

let config = JiraConfig::new(
    "https://your-instance.atlassian.net".to_string(),
    Auth::Basic {
        username: "your-email@example.com".to_string(),
        api_token: "your-api-token".to_string(),
    },
)?;
```

### Bearer認証

```rust
use jira_api::{JiraConfig, Auth};

let config = JiraConfig::new(
    "https://your-instance.atlassian.net".to_string(),
    Auth::Bearer {
        token: "your-bearer-token".to_string(),
    },
)?;
```

## Issue検索

### 基本的なJQL検索

```rust
use jira_api::{JiraClient, SearchParams};

let client = JiraClient::new(config)?;

// 基本検索
let result = client.search_issues(
    "project = MYPROJ ORDER BY created DESC",
    SearchParams::new().max_results(50),
).await?;

println!("Found {} issues", result.total);
for issue in result.issues {
    println!("{}: {}", issue.key, issue.fields.summary);
}
```

### 高度なSearchParams

```rust
use jira_api::{SearchParams};

let params = SearchParams::new()
    .max_results(100)
    .start_at(50)
    .fields(vec![
        "summary".to_string(),
        "status".to_string(),
        "assignee".to_string(),
        "priority".to_string(),
    ])
    .expand(vec!["changelog".to_string()]);

let result = client.search_issues(
    "project = MYPROJ AND status != Done",
    params,
).await?;
```

### 特定の条件での検索

```rust
// 最近1週間の更新されたIssue
let jql = "updated >= -1w ORDER BY updated DESC";

// 特定の担当者のIssue
let jql = "assignee = 'john.doe@example.com' AND status != Done";

// 優先度がHighまたはCriticalのIssue
let jql = "priority in (High, Critical) ORDER BY priority DESC, created DESC";

// 複数プロジェクトの検索
let jql = "project in (PROJ1, PROJ2, PROJ3) AND status = 'In Progress'";

let result = client.search_issues(jql, SearchParams::new()).await?;
```

## メタデータ取得

### プロジェクト情報

```rust
// 全プロジェクト取得
let projects = client.get_projects().await?;
for project in projects {
    println!("Project: {} ({}) - {}", 
             project.name, project.key, 
             project.description.unwrap_or_default());
}
```

### 優先度とIssue種別

```rust
// 優先度一覧
let priorities = client.get_priorities().await?;
for priority in priorities {
    println!("Priority: {} - {}", 
             priority.name, 
             priority.description.unwrap_or_default());
}

// Issue種別一覧
let issue_types = client.get_issue_types().await?;
for issue_type in issue_types {
    println!("Issue Type: {} - {}", 
             issue_type.name, 
             issue_type.description.unwrap_or_default());
}
```

### フィールドとステータスカテゴリ

```rust
// カスタムフィールド情報
let fields = client.get_fields().await?;
for field in fields {
    if field.name.contains("customfield") {
        println!("Custom field: {} - {}", field.id, field.name);
    }
}

// ステータスカテゴリ
let categories = client.get_status_categories().await?;
for category in categories {
    println!("Status Category: {} - {}", category.name, category.key);
}
```

### ユーザー検索

```rust
// ユーザー検索
let users = client.search_users("john", 10).await?;
for user in users {
    println!("User: {} ({})", 
             user.display_name, 
             user.email_address.unwrap_or_default());
}
```

## データ永続化

### JSONストア

```rust
use jira_api::{JsonStore, PersistenceStore, IssueFilter};
use tempfile::TempDir;

let temp_dir = TempDir::new()?;
let mut json_store = JsonStore::new(temp_dir.path())
    .with_compression(true); // gzip圧縮を有効化

json_store.initialize().await?;

// Issueデータを保存
let search_result = client.search_issues(
    "project = MYPROJ",
    SearchParams::new().max_results(1000),
).await?;

let saved_count = json_store.save_issues(&search_result.issues).await?;
println!("Saved {} issues", saved_count);

// データを読み込み
let filter = IssueFilter::new()
    .project_keys(vec!["MYPROJ".to_string()])
    .statuses(vec!["Open".to_string(), "In Progress".to_string()])
    .limit(100);

let issues = json_store.load_issues(&filter).await?;
println!("Loaded {} filtered issues", issues.len());

// 統計情報
let stats = json_store.get_stats().await?;
println!("Total issues: {}, Projects: {}", 
         stats.total_issues, 
         stats.issues_by_project.len());
```

### DuckDBストア

```rust
use jira_api::{DuckDBStore, PersistenceStore, SortOrder};

// メモリ内データベース
let mut duckdb_store = DuckDBStore::new_in_memory()?;
// またはファイル
// let mut duckdb_store = DuckDBStore::new("/path/to/database.db")?;

duckdb_store.initialize().await?;

// データ保存
let saved_count = duckdb_store.save_issues(&search_result.issues).await?;
println!("Saved {} issues to DuckDB", saved_count);

// 高度なフィルタリング
let filter = IssueFilter::new()
    .project_keys(vec!["PROJ1".to_string(), "PROJ2".to_string()])
    .priorities(vec!["High".to_string(), "Critical".to_string()])
    .created_range(jira_api::DateRange::new(
        chrono::Utc::now() - chrono::Duration::days(30),
        chrono::Utc::now(),
    ))
    .sort_order(SortOrder::UpdatedDesc)
    .limit(500);

let issues = duckdb_store.load_issues(&filter).await?;
println!("Found {} recent high-priority issues", issues.len());
```

## 同期機能

### 基本的な同期

```rust
use jira_api::{SyncService, SyncConfig};

let sync_config = SyncConfig::new()
    .interval_minutes(30)
    .max_history_count(10);

let sync_service = SyncService::new(sync_config);

// 全体同期
if sync_service.should_sync().await {
    let result = sync_service.sync_full(&client).await?;
    println!("Synced {} issues in {:.2} seconds", 
             result.synced_issues_count, 
             result.duration_seconds());
}
```

### 増分同期

```rust
// 既存のIssueを取得
let existing_issues = json_store.load_all_issues().await?;

// 増分同期実行
let result = sync_service.sync_incremental(&client, &existing_issues).await?;

println!("Incremental sync results:");
println!("  New issues: {}", result.new_issues_count);
println!("  Updated issues: {}", result.updated_issues_count);
println!("  Total synced: {}", result.synced_issues_count);
```

### 時間ベースフィルタリング

```rust
use jira_api::{TimeBasedFilter};

// 最近24時間のIssue
let filter = TimeBasedFilter::last_hours(24)
    .filter_by_created(true)
    .filter_by_updated(true);

// 特定期間のIssue
let start = chrono::Utc::now() - chrono::Duration::days(7);
let end = chrono::Utc::now();
let filter = TimeBasedFilter::date_range(start, end)
    .granularity_hours(1)
    .excluded_issue_keys(vec!["PROJ-100".to_string()]);

if let Some(jql) = filter.to_jql_time_condition() {
    println!("Generated JQL: {}", jql);
}

// 時間チャンクに分割
let chunks = filter.split_into_chunks();
println!("Split into {} time chunks", chunks.len());
```

### 同期統計と履歴

```rust
// 同期統計
let stats = sync_service.get_stats().await;
println!("Sync statistics:");
println!("  Total syncs: {}", stats.total_syncs);
println!("  Successful syncs: {}", stats.successful_syncs);
println!("  Success rate: {:.1}%", 
         if stats.total_syncs > 0 {
             (stats.successful_syncs as f64 / stats.total_syncs as f64) * 100.0
         } else {
             0.0
         });

// 同期履歴
let history = sync_service.sync_history().await;
for (i, result) in history.iter().enumerate() {
    println!("Sync {}: {} issues in {:.2}s ({})", 
             i + 1,
             result.synced_issues_count,
             result.duration_seconds(),
             if result.is_success { "Success" } else { "Failed" });
}
```

## 変更履歴

### 履歴データの取得

```rust
use jira_api::{HistoryFilter, HistorySortOrder, ChangeType};

// DuckDBストアで履歴を有効化
let mut duckdb_store = DuckDBStore::new_in_memory()?;
duckdb_store.initialize().await?;

// 変更履歴付きでIssueを検索
let params = SearchParams::new()
    .expand(vec!["changelog".to_string()]);

let result = client.search_issues("project = MYPROJ", params).await?;
let saved_count = duckdb_store.save_issues(&result.issues).await?;

// 履歴フィルタリング
let history_filter = HistoryFilter::new()
    .issue_keys(vec!["MYPROJ-123".to_string()])
    .change_types(vec![ChangeType::StatusChange, ChangeType::AssigneeChange])
    .since(chrono::Utc::now() - chrono::Duration::days(30))
    .sort_order(HistorySortOrder::NewestFirst)
    .limit(100);

let history = duckdb_store.get_issue_history(&history_filter).await?;
println!("Found {} history records", history.len());

for record in history {
    println!("{}: {} changed {} from {:?} to {:?}",
             record.changed_at.format("%Y-%m-%d %H:%M"),
             record.author.display_name,
             record.field_name,
             record.from_value,
             record.to_value);
}
```

### 履歴統計

```rust
let history_stats = duckdb_store.get_history_stats(
    Some(vec!["MYPROJ".to_string()]),
    Some(chrono::Utc::now() - chrono::Duration::days(90)),
    Some(chrono::Utc::now()),
).await?;

println!("History statistics:");
println!("  Total changes: {}", history_stats.total_changes);
println!("  Issues with history: {}", history_stats.issues_with_history);
println!("  Most active users: {:?}", history_stats.top_authors);
println!("  Common change types: {:?}", history_stats.change_type_counts);
```

## 設定管理

### 設定の保存と読み込み

```rust
use jira_api::{ConfigStore, FileConfigStore, AppConfig};

let mut config_store = FileConfigStore::new(&dirs::home_dir()
    .unwrap()
    .join(".jira-api"));

// 設定を作成
let app_config = AppConfig::new()
    .jira_url("https://your-instance.atlassian.net".to_string())
    .username("your-email@example.com".to_string())
    .default_projects(vec!["PROJ1".to_string(), "PROJ2".to_string()])
    .sync_interval_minutes(30);

// 設定を保存
config_store.save_config(&app_config).await?;
println!("Configuration saved");

// 設定を読み込み
let loaded_config = config_store.load_config().await?;
println!("Loaded configuration for: {}", 
         loaded_config.username.unwrap_or("Unknown".to_string()));
```

## 高度な使用例

### 複数ストアの統合使用

```rust
use jira_api::{JsonStore, DuckDBStore, PersistenceStore};

// 複数のストアを初期化
let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
let mut duckdb_store = DuckDBStore::new_in_memory()?;

json_store.initialize().await?;
duckdb_store.initialize().await?;

// 同一データを両方のストアに保存
let issues = client.search_issues("project = MYPROJ", SearchParams::new()).await?.issues;

let json_saved = json_store.save_issues(&issues).await?;
let duckdb_saved = duckdb_store.save_issues(&issues).await?;

println!("Saved to JSON: {}, DuckDB: {}", json_saved, duckdb_saved);

// 異なるクエリで性能比較
let start = std::time::Instant::now();
let json_results = json_store.load_issues(&IssueFilter::new().limit(1000)).await?;
let json_time = start.elapsed();

let start = std::time::Instant::now();
let duckdb_results = duckdb_store.load_issues(&IssueFilter::new().limit(1000)).await?;
let duckdb_time = start.elapsed();

println!("Performance comparison:");
println!("  JSON: {} issues in {:?}", json_results.len(), json_time);
println!("  DuckDB: {} issues in {:?}", duckdb_results.len(), duckdb_time);
```

### バッチ処理

```rust
use jira_api::{SearchParams, IssueFilter, SortOrder};

async fn batch_process_all_issues(
    client: &JiraClient,
    store: &mut dyn PersistenceStore,
    project_key: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let batch_size = 100;
    let mut start_at = 0;
    let mut total_processed = 0;

    loop {
        let params = SearchParams::new()
            .max_results(batch_size)
            .start_at(start_at);

        let jql = format!("project = {} ORDER BY created ASC", project_key);
        let result = client.search_issues(&jql, params).await?;

        if result.issues.is_empty() {
            break;
        }

        let saved = store.save_issues(&result.issues).await?;
        total_processed += saved;
        start_at += batch_size;

        println!("Processed batch: {} issues (total: {})", saved, total_processed);

        if result.issues.len() < batch_size {
            break; // 最後のバッチ
        }
    }

    Ok(total_processed)
}

// 使用例
let total = batch_process_all_issues(&client, &mut duckdb_store, "MYPROJ").await?;
println!("Total processed: {} issues", total);
```

### エラーハンドリング

```rust
use jira_api::{Error, JiraClient, SearchParams};

async fn robust_search(
    client: &JiraClient,
    jql: &str,
    max_retries: usize,
) -> Result<Vec<jira_api::Issue>, Error> {
    let mut retry_count = 0;

    loop {
        match client.search_issues(jql, SearchParams::new()).await {
            Ok(result) => return Ok(result.issues),
            Err(Error::Api(ref api_err)) if retry_count < max_retries => {
                retry_count += 1;
                println!("API error (retry {}/{}): {}", retry_count, max_retries, api_err);
                tokio::time::sleep(tokio::time::Duration::from_secs(2_u64.pow(retry_count as u32))).await;
            },
            Err(Error::Network(ref net_err)) if retry_count < max_retries => {
                retry_count += 1;
                println!("Network error (retry {}/{}): {}", retry_count, max_retries, net_err);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            },
            Err(e) => return Err(e),
        }
    }
}

// 使用例
match robust_search(&client, "project = MYPROJ", 3).await {
    Ok(issues) => println!("Successfully retrieved {} issues", issues.len()),
    Err(e) => eprintln!("Failed after retries: {}", e),
}
```

## まとめ

このライブラリは、JIRAとの統合を簡単かつ柔軟に行うための包括的なソリューションを提供します。基本的なAPI呼び出しから、高度なデータ永続化、同期機能まで、様々な用途に対応できます。

より詳細な情報は、各モジュールのAPIドキュメントとサンプルコード（`examples/`ディレクトリ）をご参照ください。