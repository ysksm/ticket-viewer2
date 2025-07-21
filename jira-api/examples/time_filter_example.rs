/// JIRA 時間ベースフィルタリングの使用例
/// 
/// TimeBasedFilterを使った様々な時間条件の設定と JQL クエリ生成を示します
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example time_filter_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, TimeBasedFilter, SearchParams};
use chrono::{Utc, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("[INFO] JIRA 時間ベースフィルタリングの使用例");
    println!("==========================================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. 基本的な時間フィルターの作成
    println!("\n[1] 基本的な時間フィルター");
    
    // 最近24時間
    let filter_24h = TimeBasedFilter::last_hours(24);
    println!("    最近24時間:");
    println!("    - 開始時刻: {}", 
        filter_24h.since.unwrap().format("%Y-%m-%d %H:%M")
    );
    println!("    - 終了時刻: {}", 
        filter_24h.until.unwrap().format("%Y-%m-%d %H:%M")
    );
    
    // 最近7日間
    let filter_7d = TimeBasedFilter::last_days(7);
    println!("\n    最近7日間:");
    println!("    - 開始時刻: {}", 
        filter_7d.since.unwrap().format("%Y-%m-%d %H:%M")
    );
    println!("    - 終了時刻: {}", 
        filter_7d.until.unwrap().format("%Y-%m-%d %H:%M")
    );

    // 2. カスタム期間フィルター
    println!("\n[2] カスタム期間フィルター");
    
    let start_date = Utc::now() - Duration::days(30);
    let end_date = Utc::now() - Duration::days(1);
    let custom_filter = TimeBasedFilter::date_range(start_date, end_date);
    
    println!("    30日前から1日前まで:");
    println!("    - 開始時刻: {}", 
        custom_filter.since.unwrap().format("%Y-%m-%d %H:%M")
    );
    println!("    - 終了時刻: {}", 
        custom_filter.until.unwrap().format("%Y-%m-%d %H:%M")
    );

    // 3. 増分同期用フィルター
    println!("\n[3] 増分同期用フィルター");
    
    let last_sync_time = Utc::now() - Duration::hours(2);
    let incremental_filter = TimeBasedFilter::incremental_since(last_sync_time);
    
    println!("    2時間前から現在まで（増分同期用）:");
    println!("    - 開始時刻: {}", 
        incremental_filter.since.unwrap().format("%Y-%m-%d %H:%M")
    );
    println!("    - 終了時刻: {}", 
        incremental_filter.until.unwrap().format("%Y-%m-%d %H:%M")
    );
    println!("    - 更新時刻でフィルタ: {}", incremental_filter.filter_by_updated);
    println!("    - 既存除外: {}", incremental_filter.exclude_existing);

    // 4. フィルター設定の詳細制御
    println!("\n[4] フィルター設定の詳細制御");
    
    let detailed_filter = TimeBasedFilter::new()
        .since(Utc::now() - Duration::days(7))
        .until(Utc::now())
        .granularity_hours(6)  // 6時間粒度
        .filter_by_created(true)
        .filter_by_updated(false)
        .exclude_existing(true)
        .excluded_issue_keys(vec![
            "TEST-123".to_string(),
            "DEMO-456".to_string(),
            "PROJECT-789".to_string()
        ]);
    
    println!("    詳細制御フィルター:");
    println!("    - 時間粒度: {} 時間", detailed_filter.granularity_hours);
    println!("    - 作成時刻でフィルタ: {}", detailed_filter.filter_by_created);
    println!("    - 更新時刻でフィルタ: {}", detailed_filter.filter_by_updated);
    println!("    - 除外するIssue数: {} 件", detailed_filter.excluded_issue_keys.len());
    
    for (i, key) in detailed_filter.excluded_issue_keys.iter().enumerate() {
        println!("      {}. {}", i + 1, key);
    }

    // 5. JQLクエリ生成
    println!("\n[5] JQLクエリ生成");
    
    let filters = vec![
        ("最近24時間", &filter_24h),
        ("最近7日間", &filter_7d),
        ("カスタム期間", &custom_filter),
        ("増分同期用", &incremental_filter),
        ("詳細制御", &detailed_filter),
    ];
    
    for (name, filter) in filters {
        if let Some(jql_condition) = filter.to_jql_time_condition() {
            println!("\n    {} のJQL条件:", name);
            println!("    {}", jql_condition);
        } else {
            println!("\n    {} のJQL条件: なし", name);
        }
    }

    // 6. 時間チャンクの分割
    println!("\n[6] 時間チャンクの分割");
    
    let chunked_filter = TimeBasedFilter::date_range(
        Utc::now() - Duration::days(3),
        Utc::now()
    ).granularity_hours(12); // 12時間単位で分割
    
    let chunks = chunked_filter.split_into_chunks();
    
    println!("    3日間を12時間粒度で分割:");
    println!("    - 総チャンク数: {} 個", chunks.len());
    
    for (i, chunk) in chunks.iter().enumerate() {
        println!("      チャンク {}: {} - {} ({:.1} 時間)", 
            i + 1,
            chunk.start.format("%Y-%m-%d %H:%M"),
            chunk.end.format("%Y-%m-%d %H:%M"),
            chunk.duration_hours()
        );
        
        // 各チャンクのJQL条件も表示
        let chunk_jql = chunk.to_jql_condition(true, true);
        println!("        JQL: {}", chunk_jql);
    }

    // 7. フィルター検証
    println!("\n[7] フィルター検証");
    
    let test_filters = vec![
        ("有効なフィルター", TimeBasedFilter::last_hours(24)),
        ("無効なフィルター（時刻逆転）", TimeBasedFilter::new()
            .since(Utc::now())
            .until(Utc::now() - Duration::hours(1))),
        ("無効なフィルター（時間粒度0）", TimeBasedFilter::new()
            .granularity_hours(0)),
        ("無効なフィルター（両方無効）", TimeBasedFilter::new()
            .filter_by_created(false)
            .filter_by_updated(false)),
    ];
    
    for (name, filter) in test_filters {
        match filter.is_valid() {
            Ok(_) => println!("    {} : [OK]", name),
            Err(e) => println!("    {} : [ERROR] {}", name, e),
        }
    }

    // 8. 実際のJIRA検索での使用例
    println!("\n[8] 実際のJIRA検索での使用");
    
    // 最近24時間の全プロジェクトの新規Issue検索
    let recent_filter = TimeBasedFilter::last_hours(24)
        .filter_by_created(true)
        .filter_by_updated(false); // 作成時刻のみでフィルタ
        
    if let Some(time_condition) = recent_filter.to_jql_time_condition() {
        // プロジェクトを指定したJQLクエリ
        let jql_query = format!("project = TEST AND ({})", time_condition);
        println!("    検索JQL: {}", jql_query);
        
        // 実際に検索を実行（TESTプロジェクトが存在する場合のみ）
        let search_params = SearchParams::new()
            .max_results(5)
            .fields(vec![
                "key".to_string(),
                "summary".to_string(),
                "created".to_string(),
                "status".to_string(),
                "reporter".to_string(),
                "issuetype".to_string(),
                "updated".to_string(),
            ]);
        
        match client.search_issues(&jql_query, search_params).await {
            Ok(result) => {
                println!("    [OK] 検索成功! 総件数: {} 件", result.total);
                
                if !result.issues.is_empty() {
                    println!("    最初の {} 件を表示:", 
                        std::cmp::min(3, result.issues.len())
                    );
                    
                    for (i, issue) in result.issues.iter().take(3).enumerate() {
                        println!("      {}. {} - {} [{}]", 
                            i + 1,
                            issue.key,
                            issue.fields.summary,
                            issue.fields.status.name
                        );
                        println!("         作成: {}", 
                            issue.fields.created.format("%Y-%m-%d %H:%M")
                        );
                    }
                } else {
                    println!("    該当するIssueが見つかりませんでした");
                }
            }
            Err(e) => {
                println!("    [INFO] 検索実行スキップ ({})", e);
                println!("    実際の使用時はプロジェクトキーを適切に設定してください");
            }
        }
    }

    // 9. パフォーマンス最適化のヒント
    println!("\n[9] パフォーマンス最適化のヒント");
    
    println!("    1. 時間粒度の調整:");
    println!("       - 大量データ: 24時間以上の粒度推奨");
    println!("       - 少量データ: 1時間粒度でも可能");
    
    println!("\n    2. フィールドフィルタリング:");
    println!("       - 作成時刻のみ: 新規Issue検出に最適");
    println!("       - 更新時刻のみ: 変更Issue検出に最適");
    println!("       - 両方: 包括的な検索（パフォーマンス低下あり）");
    
    println!("\n    3. 除外機能の活用:");
    println!("       - 既存Issue除外: 重複処理の回避");
    println!("       - 特定Issue除外: 処理済みIssueのスキップ");

    // 10. エラーハンドリングの実例
    println!("\n[10] エラーハンドリングの実例");
    
    // 不正な時間範囲でのJIRA検索
    let invalid_jql = "created >= '2024-12-31 23:59' AND created <= '2024-01-01 00:00'";
    
    let error_search_params = SearchParams::new().max_results(1);
    
    match client.search_issues(invalid_jql, error_search_params).await {
        Ok(result) => {
            println!("    [UNEXPECTED] 不正な時間範囲での検索が成功: {} 件", result.total);
        }
        Err(e) => {
            println!("    [EXPECTED] 不正な時間範囲での検索エラー: {}", e);
            println!("    → TimeBasedFilterの検証機能を使用して事前にエラーを防げます");
        }
    }

    println!("\n時間フィルタリングサンプル完了!");
    println!("\n高度な使用例:");
    println!("   // 業務時間のみのフィルター");
    println!("   let business_hours = TimeBasedFilter::new()");
    println!("       .since(today_9am)");
    println!("       .until(today_6pm)");
    println!("       .filter_by_created(true);");
    println!();
    println!("   // 週末を除外したフィルター");
    println!("   let weekday_only = TimeBasedFilter::last_days(7)");
    println!("       .granularity_hours(24)");
    println!("       // 実装では日付チェック機能の追加が必要");
    println!();
    println!("その他のサンプル:");
    println!("   cargo run --example basic_usage");
    println!("   cargo run --example search_example");
    println!("   cargo run --example project_example");
    println!("   cargo run --example sync_example");
    
    Ok(())
}