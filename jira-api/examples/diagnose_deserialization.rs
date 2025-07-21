use jira_api::{JiraConfig, JiraClient};
use dotenv::dotenv;
use std::error::Error;
use std::env;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    println!("デシリアライゼーション診断ツール");
    
    // JIRAクライアント初期化
    let config = JiraConfig::from_env()?;
    let _client = JiraClient::new(config)?;
    
    let project_key = env::var("JIRA_PROJECT_KEY").unwrap_or_else(|_| "TEST".to_string());
    
    // まず生のHTTPクライアントでレスポンスを取得
    println!("\n=== 生のHTTPリクエストでJSONレスポンス取得 ===");
    let auth_header = format!("{}:{}", 
        env::var("JIRA_USER").unwrap(), 
        env::var("JIRA_API_TOKEN").unwrap()
    );
    let encoded_auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_header.as_bytes());
    
    let http_client = reqwest::Client::new();
    let search_url = format!("{}/rest/api/3/search", env::var("JIRA_URL")?);
    
    let search_body = serde_json::json!({
        "jql": format!("project = {} ORDER BY updated DESC", project_key),
        "maxResults": 1
    });
    
    let response = http_client
        .post(&search_url)
        .header("Authorization", format!("Basic {}", encoded_auth))
        .header("Content-Type", "application/json")
        .json(&search_body)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await?;
        println!("❌ HTTPエラー: {}", error_text);
        return Ok(());
    }
    
    let response_text = response.text().await?;
    println!("✅ レスポンス取得成功: {} bytes", response_text.len());
    
    // JSONとしてパース
    let json_value: Value = serde_json::from_str(&response_text)?;
    println!("✅ JSON パース成功");
    
    // SearchResult構造の段階的テスト
    println!("\n=== SearchResult構造の段階的テスト ===");
    
    // ステップ1: 基本フィールドのテスト
    println!("ステップ1: 基本フィールド");
    let start_at = json_value.get("startAt").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let max_results = json_value.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let total = json_value.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    println!("  startAt: {}, maxResults: {}, total: {}", start_at, max_results, total);
    
    // ステップ2: issuesフィールドの存在確認
    println!("ステップ2: issues配列");
    if let Some(issues_array) = json_value.get("issues").and_then(|v| v.as_array()) {
        println!("  issues配列: {} 件", issues_array.len());
        
        if let Some(first_issue) = issues_array.first() {
            println!("\n=== 最初のIssueの詳細分析 ===");
            
            // Issue基本フィールド
            let id = first_issue.get("id").and_then(|v| v.as_str()).unwrap_or("不明");
            let key = first_issue.get("key").and_then(|v| v.as_str()).unwrap_or("不明");
            let self_url = first_issue.get("self").and_then(|v| v.as_str()).unwrap_or("不明");
            println!("Issue基本情報: id={}, key={}, self={}", id, key, self_url);
            
            // fieldsオブジェクトの分析
            if let Some(fields) = first_issue.get("fields") {
                println!("\nfieldsオブジェクト分析:");
                analyze_fields_object(fields);
            } else {
                println!("❌ fieldsオブジェクトが見つかりません");
            }
        }
    } else {
        println!("❌ issues配列が見つかりません");
    }
    
    // ステップ3: SearchResult全体のデシリアライゼーション試行
    println!("\n=== SearchResult全体のデシリアライゼーション試行 ===");
    match serde_json::from_value::<jira_api::models::SearchResult>(json_value.clone()) {
        Ok(_) => println!("✅ SearchResult デシリアライゼーション成功"),
        Err(e) => {
            println!("❌ SearchResult デシリアライゼーション失敗: {}", e);
            
            // エラー位置の特定
            let error_msg = e.to_string();
            if let Some(line_col) = extract_line_column(&error_msg) {
                let (line, column) = line_col;
                println!("エラー位置: line {}, column {}", line, column);
                
                // エラー位置周辺のテキストを表示
                show_error_context(&response_text, column);
            }
        }
    }
    
    Ok(())
}

fn analyze_fields_object(fields: &Value) {
    if let Some(fields_obj) = fields.as_object() {
        for (key, value) in fields_obj {
            let type_info = match value {
                Value::String(s) => format!("String (\"{}...\")", if s.len() > 20 { &s[..20] } else { s }),
                Value::Number(_) => "Number".to_string(),
                Value::Bool(_) => "Boolean".to_string(),
                Value::Array(_) => "Array".to_string(),
                Value::Object(_) => "Object".to_string(),
                Value::Null => "null".to_string(),
            };
            println!("  {}: {}", key, type_info);
            
            // 特に問題のありそうなフィールドの詳細分析
            if key == "description" || key == "summary" || key == "reporter" || key == "assignee" {
                match value {
                    Value::Object(obj) => {
                        println!("    オブジェクト詳細:");
                        for (sub_key, sub_value) in obj {
                            let sub_type = match sub_value {
                                Value::String(_) => "String",
                                Value::Number(_) => "Number",
                                Value::Bool(_) => "Boolean",
                                Value::Array(_) => "Array",
                                Value::Object(_) => "Object",
                                Value::Null => "null",
                            };
                            println!("      {}: {}", sub_key, sub_type);
                        }
                    },
                    Value::String(s) => {
                        println!("    文字列内容: \"{}\"", if s.len() > 100 { &s[..100] } else { s });
                    },
                    _ => {}
                }
            }
        }
    }
}

fn extract_line_column(error_msg: &str) -> Option<(usize, usize)> {
    // "at line X column Y" パターンを検索
    if let Some(line_start) = error_msg.find("line ") {
        if let Some(line_end) = error_msg[line_start + 5..].find(' ') {
            let line_str = &error_msg[line_start + 5..line_start + 5 + line_end];
            if let Ok(line) = line_str.parse::<usize>() {
                if let Some(col_start) = error_msg.find("column ") {
                    let col_str = &error_msg[col_start + 7..];
                    if let Some(col_end) = col_str.find(' ').or(Some(col_str.len())) {
                        if let Ok(column) = col_str[..col_end].parse::<usize>() {
                            return Some((line, column));
                        }
                    }
                }
            }
        }
    }
    None
}

fn show_error_context(text: &str, column: usize) {
    if column > text.len() {
        println!("エラー位置がテキスト範囲を超えています");
        return;
    }
    
    let start = column.saturating_sub(50);
    let end = (column + 50).min(text.len());
    let context = &text[start..end];
    
    println!("エラー位置周辺のテキスト:");
    println!("位置: {}", column);
    println!("コンテキスト: ...{}...", context);
    
    // エラー位置をマーク
    let marker_pos = column - start;
    let marker = " ".repeat(marker_pos) + "^";
    println!("              ...{}...", marker);
}