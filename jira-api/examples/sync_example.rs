/// JIRA 同期機能の使用例
/// 
/// 時間ベースフィルタリング、増分同期、同期結果統計の使い方を示します
/// 
/// 実行前に環境変数を設定してください：
/// export JIRA_URL=https://your-instance.atlassian.net
/// export JIRA_USER=your-email@example.com
/// export JIRA_API_TOKEN=your-api-token
/// 
/// 実行方法：
/// cargo run --example sync_example

use dotenv::dotenv;
use jira_api::{JiraClient, JiraConfig, SyncService, SyncConfig, TimeBasedFilter};
use chrono::{Utc, Duration};
use std::thread::sleep;
use std::time::Duration as StdDuration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    println!("[INFO] JIRA 同期機能の使用例");
    println!("===========================");

    // 設定をロード
    let config = JiraConfig::from_env()
        .map_err(|_| "環境変数が設定されていません。README.mdを参照してください。")?;
    
    let client = JiraClient::new(config)?;
    println!("[OK] JIRAクライアント準備完了");

    // 1. 同期サービスの基本設定
    println!("\n[1] 同期サービスの設定");
    
    let sync_config = SyncConfig::new()
        .interval_minutes(30)  // 30分間隔
        .max_history_count(10) // 最大10回の履歴保持
        .enable_time_optimization(true)
        .concurrent_sync_count(2);
        
    let sync_service = SyncService::new(sync_config);
    
    println!("    設定:");
    println!("    - 同期間隔: {} 分", sync_service.config().interval_minutes);
    println!("    - 最大履歴数: {} 回", sync_service.config().max_history_count);
    println!("    - 時間最適化: {}", sync_service.config().enable_time_optimization);
    println!("    - 並行処理数: {} 個", sync_service.config().concurrent_sync_count);

    // 2. 初回同期実行
    println!("\n[2] 初回同期実行");
    
    if sync_service.should_sync().await {
        println!("    同期を開始中...");
        
        match sync_service.sync_full(&client).await {
            Ok(result) => {
                println!("    [OK] 初回同期完了!");
                print_sync_result(&result);
            }
            Err(e) => {
                println!("    [ERROR] 同期エラー: {}", e);
            }
        }
    } else {
        println!("    [WARNING] 同期は必要ありません");
    }

    // 3. 時間ベースフィルタリングのデモ
    println!("\n[3] 時間ベースフィルタリング");
    
    // 最近24時間のフィルター
    let filter_24h = TimeBasedFilter::last_hours(24);
    println!("    最近24時間のフィルター:");
    if let Some(jql_condition) = filter_24h.to_jql_time_condition() {
        println!("    JQL条件: {}", jql_condition);
    }
    
    // 最近7日間のフィルター
    let filter_7d = TimeBasedFilter::last_days(7);
    println!("\n    最近7日間のフィルター:");
    if let Some(jql_condition) = filter_7d.to_jql_time_condition() {
        println!("    JQL条件: {}", jql_condition);
    }
    
    // 特定期間のフィルター
    let start_date = Utc::now() - Duration::days(30);
    let end_date = Utc::now() - Duration::days(1);
    let filter_range = TimeBasedFilter::date_range(start_date, end_date);
    
    println!("\n    特定期間のフィルター:");
    println!("    期間: {} から {}", 
        start_date.format("%Y-%m-%d %H:%M"),
        end_date.format("%Y-%m-%d %H:%M")
    );
    if let Some(jql_condition) = filter_range.to_jql_time_condition() {
        println!("    JQL条件: {}", jql_condition);
    }
    
    // 4. フィルターの時間チャンクデモ
    println!("\n[4] 時間チャンクの分割");
    
    let chunks = filter_range.split_into_chunks();
    println!("    時間チャンク数: {} 個", chunks.len());
    
    for (i, chunk) in chunks.iter().take(5).enumerate() {
        println!("    チャンク {}: {} - {} ({:.1} 時間)", 
            i + 1,
            chunk.start.format("%Y-%m-%d %H:%M"),
            chunk.end.format("%Y-%m-%d %H:%M"),
            chunk.duration_hours()
        );
    }
    
    if chunks.len() > 5 {
        println!("    ... 他 {} 個のチャンク", chunks.len() - 5);
    }

    // 5. 同期状態管理のデモ
    println!("\n[5] 同期状態管理");
    
    println!("    現在の状態: {:?}", sync_service.current_state().await);
    println!("    同期可能: {}", sync_service.can_sync().await);
    println!("    同期必要: {}", sync_service.should_sync().await);
    
    if let Some(last_sync) = sync_service.last_successful_sync().await {
        let elapsed = Utc::now() - last_sync;
        println!("    最後の成功同期: {} 分前", elapsed.num_minutes());
    } else {
        println!("    最後の成功同期: なし");
    }

    // 6. 同期統計の表示
    println!("\n[6] 同期統計");
    
    let stats = sync_service.get_stats().await;
    println!("    総同期回数: {} 回", stats.total_syncs);
    println!("    成功同期回数: {} 回", stats.successful_syncs);
    println!("    成功率: {:.1}%", 
        if stats.total_syncs > 0 {
            (stats.successful_syncs as f64 / stats.total_syncs as f64) * 100.0
        } else {
            0.0
        }
    );
    println!("    総同期Issue数: {} 件", stats.total_issues_synced);
    println!("    平均同期時間: {:.2} 秒", stats.average_duration_seconds);

    // 7. 履歴表示
    println!("\n[7] 同期履歴");
    
    let history = sync_service.sync_history().await;
    if history.is_empty() {
        println!("    履歴なし");
    } else {
        println!("    履歴数: {} 件", history.len());
        
        for (i, result) in history.iter().enumerate() {
            println!("\n    履歴 {}:", i + 1);
            println!("      開始時刻: {}", result.start_time.format("%Y-%m-%d %H:%M:%S"));
            println!("      終了時刻: {}", result.end_time.format("%Y-%m-%d %H:%M:%S"));
            println!("      処理時間: {:.2} 秒", result.duration_seconds());
            println!("      成功: {}", if result.is_success { "はい" } else { "いいえ" });
            println!("      同期Issue数: {} 件", result.synced_issues_count);
            println!("      新規Issue数: {} 件", result.new_issues_count);
            println!("      更新Issue数: {} 件", result.updated_issues_count);
            println!("      エラー数: {} 件", result.error_count);
            
            if !result.error_messages.is_empty() {
                println!("      エラーメッセージ:");
                for (j, error) in result.error_messages.iter().take(3).enumerate() {
                    println!("        {}. {}", j + 1, error);
                }
                if result.error_messages.len() > 3 {
                    println!("        ... 他 {} 個のエラー", result.error_messages.len() - 3);
                }
            }
            
            if !result.project_stats.is_empty() {
                println!("      プロジェクト別統計:");
                for (project, project_stat) in result.project_stats.iter().take(5) {
                    println!("        {}: {} 件 (新規: {}, 更新: {}, エラー: {})",
                        project,
                        project_stat.synced_count,
                        project_stat.new_count,
                        project_stat.updated_count,
                        project_stat.error_count
                    );
                }
                if result.project_stats.len() > 5 {
                    println!("        ... 他 {} プロジェクト", result.project_stats.len() - 5);
                }
            }
        }
    }

    // 8. 増分同期の実演（テスト用の空配列で）
    println!("\n[8] 増分同期の実演");
    
    println!("    3秒待機中... (実際の使用では同期間隔に応じて待機)");
    sleep(StdDuration::from_secs(3));
    
    // 空のIssue配列で増分同期をテスト（実際は既存のIssueを渡す）
    let existing_issues = vec![];
    
    match sync_service.sync_incremental(&client, &existing_issues).await {
        Ok(result) => {
            println!("    [OK] 増分同期完了!");
            print_sync_result(&result);
        }
        Err(e) => {
            println!("    [ERROR] 増分同期エラー: {}", e);
        }
    }

    // 9. エラーハンドリングのデモ
    println!("\n[9] エラーハンドリング");
    
    // エラー状態をシミュレート
    if sync_service.current_state().await.is_error() {
        println!("    エラー状態から復旧中...");
        sync_service.recover_from_error().await;
        println!("    [OK] エラーから復旧しました");
    } else {
        println!("    現在エラー状態ではありません");
    }

    // 10. カスタム同期設定の例
    println!("\n[10] カスタム同期設定の例");
    
    let custom_config = SyncConfig::new()
        .interval_minutes(15)  // 15分間隔
        .max_history_count(20) // 20回の履歴
        .enable_time_optimization(true)
        .concurrent_sync_count(5)
        .target_projects(vec!["PROJECT1".to_string(), "PROJECT2".to_string()])
        .excluded_fields(vec!["description".to_string(), "comment".to_string()]);
        
    let custom_sync_service = SyncService::new(custom_config);
    
    println!("    カスタム設定:");
    println!("    - 同期間隔: {} 分", custom_sync_service.config().interval_minutes);
    println!("    - 対象プロジェクト: {:?}", custom_sync_service.config().target_projects);
    println!("    - 除外フィールド: {:?}", custom_sync_service.config().excluded_fields);

    // 11. 時間フィルターの検証
    println!("\n[11] 時間フィルターの検証");
    
    // 有効なフィルター
    let valid_filter = TimeBasedFilter::last_hours(24)
        .filter_by_created(true)
        .filter_by_updated(true);
    
    match valid_filter.is_valid() {
        Ok(_) => println!("    有効なフィルター設定: [OK]"),
        Err(e) => println!("    フィルター設定エラー: {}", e),
    }
    
    // 無効なフィルター（開始時刻が終了時刻より後）
    let invalid_filter = TimeBasedFilter::new()
        .since(Utc::now())
        .until(Utc::now() - Duration::hours(1));
        
    match invalid_filter.is_valid() {
        Ok(_) => println!("    無効なフィルターが有効と判定されました: [ERROR]"),
        Err(e) => println!("    期待通りの無効フィルター検出: {}", e),
    }

    // 12. 全フィールド取得での同期（除外フィールドなし = 全取得）
    println!("\n[12] 全フィールド取得での同期");
    
    let all_fields_config = SyncConfig::new()
        .interval_minutes(60)
        .enable_time_optimization(false) // 全フィールド取得時は時間最適化無効
        .concurrent_sync_count(1) // 負荷軽減のため1つずつ
        .excluded_fields(vec![]); // 除外フィールドなし = 全フィールド取得
        
    let all_fields_sync = SyncService::new(all_fields_config);
    
    println!("    全フィールド同期設定:");
    println!("    - 除外フィールド: {:?} (空 = 全フィールド取得)", all_fields_sync.config().excluded_fields);
    println!("    - 時間最適化: {}", all_fields_sync.config().enable_time_optimization);
    
    // テスト同期（実際には大量のデータが取得される）
    println!("    注記: SearchParamsでfields=*allを指定することで全カスタムフィールドが取得されます");
    println!("    注記: 実際の同期では大量のデータ転送が発生する可能性があります");

    // 13. 変更履歴付き同期（expand=changelog）
    println!("\n[13] 変更履歴付き同期");
    
    let changelog_config = SyncConfig::new()
        .interval_minutes(30)
        .enable_time_optimization(true)
        .concurrent_sync_count(1) // changelogは重いため1つずつ
        .excluded_fields(vec![
            "description".to_string(),
            "comment".to_string()
        ]); // 重いフィールドを除外して基本フィールド + changelogを取得
        
    let changelog_sync = SyncService::new(changelog_config);
    
    println!("    変更履歴同期設定:");
    println!("    - 除外フィールド: {:?}", changelog_sync.config().excluded_fields);
    println!("    - 並行処理数: {} (changelogは重い処理のため)", changelog_sync.config().concurrent_sync_count);
    
    println!("    注記: SearchParamsでexpand=changelogを指定することで変更履歴が取得されます");
    println!("    注記: ステータス変更、担当者変更、フィールド更新等の詳細履歴が含まれます");

    // 14. フル機能同期（全フィールド + 変更履歴）
    println!("\n[14] フル機能同期（全フィールド + 変更履歴）");
    
    let full_config = SyncConfig::new()
        .interval_minutes(120) // 2時間間隔（重い処理のため）
        .enable_time_optimization(true)
        .concurrent_sync_count(1) // 負荷軽減
        .excluded_fields(vec![]) // フィールド除外なし = 全フィールド取得
        .target_projects(vec!["CRITICAL".to_string()]) // 重要プロジェクトのみ
        .max_history_count(5); // 履歴も少なめに
        
    let full_sync = SyncService::new(full_config);
    
    println!("    フル機能同期設定:");
    println!("    - 除外フィールド: {:?} (空 = 全フィールド取得)", full_sync.config().excluded_fields);
    println!("    - 対象プロジェクト: {:?}", full_sync.config().target_projects);
    println!("    - 同期間隔: {} 分（重い処理のため長め）", full_sync.config().interval_minutes);
    println!("    - 最大履歴数: {} (メモリ使用量を抑制)", full_sync.config().max_history_count);
    
    println!("    注記: SearchParamsで fields=*all, expand=changelog を指定");
    println!("    注記: 全フィールド + 全変更履歴 = 最も詳細だが最も重い処理");
    println!("    注記: 本番環境では対象プロジェクトを限定することを推奨");

    // 15. パフォーマンス比較シミュレーション
    println!("\n[15] パフォーマンス比較（シミュレーション）");
    
    // 基本同期（多くのフィールドを除外）
    let basic_config = SyncConfig::new()
        .excluded_fields(vec![
            "description".to_string(),
            "comment".to_string(), 
            "attachment".to_string(),
            "worklog".to_string()
        ]);
    let basic_sync = SyncService::new(basic_config);
    
    // 全フィールド同期（除外なし）
    let all_sync = SyncService::new(
        SyncConfig::new().excluded_fields(vec![])
    );
    
    // 軽量同期（重いフィールドのみ除外）
    let lightweight_sync = SyncService::new(
        SyncConfig::new()
            .excluded_fields(vec!["description".to_string(), "comment".to_string()])
    );
    
    println!("    同期設定の比較:");
    println!("    基本同期 (重いフィールドを除外):");
    println!("      - 除外フィールド: {:?}", basic_sync.config().excluded_fields);
    println!("      - 予想データ転送量: 小");
    println!("      - 予想処理時間: 高速");
    println!("      - 適用場面: ダッシュボード、一覧表示");
    
    println!("    軽量同期 (最低限の除外):");
    println!("      - 除外フィールド: {:?}", lightweight_sync.config().excluded_fields);
    println!("      - 予想データ転送量: 中");
    println!("      - 予想処理時間: 中程度");
    println!("      - 適用場面: 詳細表示、レポート");
    
    println!("    全フィールド同期 (除外なし):");
    println!("      - 除外フィールド: {:?}", all_sync.config().excluded_fields);
    println!("      - 予想データ転送量: 大");
    println!("      - 予想処理時間: 低速");
    println!("      - 適用場面: 完全バックアップ、詳細分析");
    
    println!("    注記: SearchParamsで fields=*all, expand=changelog を追加指定可能");

    // 16. 実際の同期テスト（軽量版）
    println!("\n[16] 軽量同期テスト");
    
    let test_config = SyncConfig::new()
        .interval_minutes(5)
        .max_history_count(3)
        .excluded_fields(vec![
            "description".to_string(),
            "comment".to_string(),
            "attachment".to_string(),
            "worklog".to_string(),
            "subtasks".to_string()
        ]) // 重いフィールドを除外して軽量化
        .target_projects(vec![]); // 全プロジェクト対象（制限なし）
        
    let test_sync = SyncService::new(test_config);
    
    if test_sync.should_sync().await {
        println!("    軽量同期を実行中...");
        match test_sync.sync_full(&client).await {
            Ok(result) => {
                println!("    [OK] 軽量同期完了!");
                println!("      同期Issue数: {} 件", result.synced_issues_count);
                println!("      処理時間: {:.2} 秒", result.duration_seconds());
                
                // 取得したデータの特徴を表示
                if result.synced_issues_count > 0 {
                    println!("    データ特徴:");
                    println!("      - 基本フィールドのみ取得");
                    println!("      - 変更履歴は含まれません");
                    println!("      - カスタムフィールドは含まれません");
                }
            }
            Err(e) => {
                println!("    [ERROR] 軽量同期エラー: {}", e);
                println!("    注記: 実際のJIRA APIへの接続が必要です");
            }
        }
    }

    println!("\n同期機能サンプル完了!");
    println!("\n機能別サンプル:");
    println!("   cargo run --example basic_usage        # 基本的な使用方法");
    println!("   cargo run --example search_example     # 検索機能（*all, changelog含む）");
    println!("   cargo run --example project_example    # プロジェクト情報取得");
    println!("   cargo run --example config_store_example # 設定管理");
    
    Ok(())
}

/// 同期結果の詳細を出力するヘルパー関数
fn print_sync_result(result: &jira_api::SyncResult) {
    println!("      開始時刻: {}", result.start_time.format("%Y-%m-%d %H:%M:%S"));
    println!("      終了時刻: {}", result.end_time.format("%Y-%m-%d %H:%M:%S"));
    println!("      処理時間: {:.2} 秒", result.duration_seconds());
    println!("      成功: {}", if result.is_success { "はい" } else { "いいえ" });
    println!("      同期Issue数: {} 件", result.synced_issues_count);
    println!("      新規Issue数: {} 件", result.new_issues_count);
    println!("      更新Issue数: {} 件", result.updated_issues_count);
    println!("      削除Issue数: {} 件", result.deleted_issues_count);
    println!("      エラー数: {} 件", result.error_count);
    
    if !result.error_messages.is_empty() {
        println!("      エラーメッセージ:");
        for (i, error) in result.error_messages.iter().take(3).enumerate() {
            println!("        {}. {}", i + 1, error);
        }
        if result.error_messages.len() > 3 {
            println!("        ... 他 {} 個のエラー", result.error_messages.len() - 3);
        }
    }
    
    if !result.project_stats.is_empty() {
        println!("      プロジェクト別統計:");
        for (project, stats) in result.project_stats.iter().take(5) {
            println!("        {}: {} 件 (新規: {}, 更新: {}, エラー: {})",
                project,
                stats.synced_count,
                stats.new_count,
                stats.updated_count,
                stats.error_count
            );
        }
        if result.project_stats.len() > 5 {
            println!("        ... 他 {} プロジェクト", result.project_stats.len() - 5);
        }
    }
}