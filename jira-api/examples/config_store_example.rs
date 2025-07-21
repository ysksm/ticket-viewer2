use jira_api::{
    JiraConfig, Auth, ConfigStore, FileConfigStore, AppConfig,
    IssueFilter, FilterConfig, SortOrder
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== 設定ストア使用例 ===\n");

    // 1. 設定ストアの初期化
    println!("1. 設定ストアの初期化");
    let mut config_store = FileConfigStore::default_config_dir()?;
    config_store.initialize().await?;
    println!("設定ストアを初期化しました");

    // 2. JIRA設定の保存と読み込み
    println!("\n2. JIRA設定の管理");
    
    // JIRA設定を作成
    let jira_config = JiraConfig::new(
        "https://example.atlassian.net".to_string(),
        Auth::Basic {
            username: "user@example.com".to_string(),
            api_token: "your-api-token".to_string(),
        }
    )?;
    
    // 設定を保存
    config_store.save_jira_config(&jira_config).await?;
    println!("JIRA設定を保存しました");
    
    // 設定を読み込み
    let loaded_config = config_store.load_jira_config().await?;
    if let Some(config) = loaded_config {
        println!("読み込んだJIRA設定:");
        println!("   - Base URL: {}", config.base_url);
        match config.auth {
            Auth::Basic { username, .. } => {
                println!("   - 認証: Basic (ユーザー: {})", username);
            }
            Auth::Bearer { .. } => {
                println!("   - 認証: Bearer");
            }
        }
    }

    // 3. フィルター設定の管理
    println!("\n3. フィルター設定の管理");
    
    // 複数のフィルター設定を作成
    let filters = vec![
        create_bug_filter(),
        create_my_issues_filter(),
        create_recent_issues_filter(),
    ];
    
    // フィルター設定を保存
    for filter in &filters {
        config_store.save_filter_config(filter).await?;
        println!("フィルター '{}' を保存しました", filter.name);
    }
    
    // フィルター設定一覧を取得
    let saved_filters = config_store.list_filter_configs().await?;
    println!("\n保存済みフィルター一覧:");
    for filter in &saved_filters {
        println!("   - {}: {} (使用回数: {}回)", 
            filter.id, filter.name, filter.usage_count);
        if let Some(desc) = &filter.description {
            println!("     説明: {}", desc);
        }
    }
    
    // 特定のフィルターを読み込み
    let bug_filter = config_store.load_filter_config("bug_filter").await?;
    if let Some(filter) = bug_filter {
        println!("\nバグフィルターの詳細:");
        println!("   - プロジェクト: {:?}", filter.filter.project_keys);
        println!("   - ステータス: {:?}", filter.filter.statuses);
        println!("   - 課題タイプ: {:?}", filter.filter.issue_types);
    }

    // 4. アプリケーション設定の管理
    println!("\n4. アプリケーション設定の管理");
    
    // アプリケーション設定を作成・カスタマイズ
    let mut app_config = AppConfig::new();
    app_config.set_debug_mode(true);
    app_config.set_custom_setting("theme".to_string(), "dark".to_string());
    app_config.set_custom_setting("language".to_string(), "ja".to_string());
    app_config.set_custom_setting("notifications".to_string(), "enabled".to_string());
    
    // アプリケーション設定を保存
    config_store.save_app_config(&app_config).await?;
    println!("アプリケーション設定を保存しました");
    
    // アプリケーション設定を読み込み
    let loaded_app_config = config_store.load_app_config().await?;
    if let Some(config) = loaded_app_config {
        println!("アプリケーション設定:");
        println!("   - アプリ名: {}", config.app_name);
        println!("   - バージョン: {}", config.version);
        println!("   - デバッグモード: {}", config.debug_mode);
        println!("   - ログレベル: {}", config.log_level);
        println!("   - カスタム設定:");
        for (key, value) in &config.custom_settings {
            println!("     {}: {}", key, value);
        }
        println!("   - 最終更新: {}", config.last_updated.format("%Y-%m-%d %H:%M:%S"));
    }

    // 5. フィルター使用回数の更新
    println!("\n5. フィルター使用統計の更新");
    
    // フィルターを「使用」して使用回数を増加
    if let Some(mut filter) = config_store.load_filter_config("my_issues").await? {
        println!("'{}' の使用前: {}回", filter.name, filter.usage_count);
        
        // 使用回数を増加
        filter.increment_usage();
        config_store.save_filter_config(&filter).await?;
        
        println!("'{}' の使用後: {}回", filter.name, filter.usage_count);
    }

    // 6. フィルターの削除
    println!("\n6. フィルター設定の削除");
    
    let deleted = config_store.delete_filter_config("recent_issues").await?;
    if deleted {
        println!("'recent_issues' フィルターを削除しました");
    }
    
    // 削除後のフィルター一覧を確認
    let remaining_filters = config_store.list_filter_configs().await?;
    println!("残りのフィルター: {}個", remaining_filters.len());
    for filter in &remaining_filters {
        println!("   - {}", filter.name);
    }

    // 7. 設定の部分更新例
    println!("\n7. 設定の部分更新");
    
    // アプリケーション設定を更新
    if let Some(mut config) = config_store.load_app_config().await? {
        config.set_custom_setting("max_results".to_string(), "200".to_string());
        config.set_debug_mode(false);
        config_store.save_app_config(&config).await?;
        println!("アプリケーション設定を更新しました");
    }

    println!("\n=== 設定ストア使用例完了 ===");
    println!("ヒント: 設定ファイルは以下の場所に保存されています");
    println!("   - Linux: ~/.config/jira-api/");
    println!("   - macOS: ~/Library/Application Support/jira-api/");
    println!("   - Windows: %APPDATA%\\jira-api\\");
    
    Ok(())
}

/// バグ専用フィルターを作成
fn create_bug_filter() -> FilterConfig {
    let filter = IssueFilter::new()
        .issue_types(vec!["Bug".to_string()])
        .statuses(vec!["Open".to_string(), "In Progress".to_string()])
        .sort_order(SortOrder::PriorityDesc);
    
    FilterConfig::new(
        "bug_filter".to_string(),
        "バグフィルター".to_string(),
        filter,
    ).description("優先度順で並んだ未解決のバグ一覧".to_string())
}

/// 自分の課題フィルターを作成
fn create_my_issues_filter() -> FilterConfig {
    let filter = IssueFilter::new()
        .assignees(vec!["currentUser()".to_string()])
        .statuses(vec!["In Progress".to_string(), "To Do".to_string()])
        .sort_order(SortOrder::UpdatedDesc);
    
    FilterConfig::new(
        "my_issues".to_string(),
        "担当課題".to_string(),
        filter,
    ).description("現在のユーザーにアサインされた未完了課題".to_string())
}

/// 最近の課題フィルターを作成  
fn create_recent_issues_filter() -> FilterConfig {
    let filter = IssueFilter::new()
        .sort_order(SortOrder::CreatedDesc)
        .limit(50);
    
    FilterConfig::new(
        "recent_issues".to_string(),
        "最近の課題".to_string(),
        filter,
    ).description("作成日時順で並んだ最新50件の課題".to_string())
}