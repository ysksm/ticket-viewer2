# jira-api

[![Crates.io](https://img.shields.io/crates/v/jira-api.svg)](https://crates.io/crates/jira-api)
[![Documentation](https://docs.rs/jira-api/badge.svg)](https://docs.rs/jira-api)
[![License](https://img.shields.io/crates/l/jira-api.svg)](LICENSE)
[![Build Status](https://github.com/your-username/jira-api/workflows/CI/badge.svg)](https://github.com/your-username/jira-api/actions)

**JIRA REST API v3 クライアントライブラリ for Rust**

このライブラリは、JIRA REST API v3の主要なエンドポイントをサポートし、データ同期、永続化、変更履歴管理などの高度な機能を提供するRustクライアントです。

## ✨ 特徴

- 🔐 **柔軟な認証**: Basic認証とBearer認証をサポート
- 🔍 **強力な検索**: JQLクエリによる高度なIssue検索
- 📊 **メタデータ取得**: プロジェクト、優先度、Issue種別、フィールド情報の取得
- 👥 **ユーザー検索**: ユーザーアカウントの検索機能
- 🔄 **同期機能**: 増分データ同期、時間ベースフィルタリング
- 💾 **データ永続化**: JSON（gzip圧縮対応）およびDuckDB形式での保存
- 📈 **変更履歴**: Issue変更履歴の取得と詳細管理
- ⚙️ **設定管理**: 認証情報とフィルター条件の永続化
- 🚀 **非同期**: 完全な非同期処理対応
- 🛡️ **型安全**: Rustの型システムによる安全なAPI操作

## 🛠️ サポートAPIエンドポイント

- `/rest/api/3/search` - Issue検索
- `/rest/api/3/project` - プロジェクト一覧
- `/rest/api/3/priority` - 優先度一覧
- `/rest/api/3/issuetype` - Issue種別一覧
- `/rest/api/3/field` - フィールド一覧
- `/rest/api/3/statuscategory` - ステータスカテゴリ一覧
- `/rest/api/3/users/search` - ユーザー検索

## 📦 インストール

`Cargo.toml`に追加：

```toml
[dependencies]
jira-api = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
dotenv = "0.15"
```

## 🚀 クイックスタート

### 1. 環境設定

`.env`ファイルを作成：

```env
JIRA_URL=https://your-instance.atlassian.net
JIRA_USER=your-email@example.com
JIRA_API_TOKEN=your-api-token
```

### 2. 基本的な使用例

```rust
use jira_api::{JiraConfig, JiraClient, SearchParams};
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 環境変数を読み込み
    dotenv().ok();
    
    // 設定を作成
    let config = JiraConfig::from_env()?;
    let client = JiraClient::new(config)?;
    
    // プロジェクト一覧を取得
    let projects = client.get_projects().await?;
    println!("📋 Found {} projects", projects.len());
    
    // Issue検索
    let result = client.search_issues(
        "project = MYPROJ ORDER BY created DESC",
        SearchParams::new().max_results(10),
    ).await?;
    
    println!("🎫 Found {} issues", result.total);
    for issue in result.issues {
        println!("  {} - {}", issue.key, issue.fields.summary);
    }
    
    Ok(())
}
```

## 📚 主要機能

### Issue検索

```rust
use jira_api::{SearchParams};

// 基本検索
let result = client.search_issues(
    "project = MYPROJ AND status != Done",
    SearchParams::new()
        .max_results(50)
        .fields(vec!["summary".to_string(), "status".to_string()])
        .expand(vec!["changelog".to_string()]),
).await?;
```

### データ永続化

```rust
use jira_api::{JsonStore, DuckDBStore, PersistenceStore};

// JSONストア（圧縮対応）
let mut json_store = JsonStore::new(temp_dir.path()).with_compression(true);
json_store.initialize().await?;
json_store.save_issues(&issues).await?;

// DuckDBストア（SQL対応）
let mut duckdb_store = DuckDBStore::new_in_memory()?;
duckdb_store.initialize().await?;
duckdb_store.save_issues(&issues).await?;
```

### 同期機能

```rust
use jira_api::{SyncService, SyncConfig, TimeBasedFilter};

// 同期設定
let config = SyncConfig::new()
    .interval_minutes(30)
    .max_history_count(10);

let sync_service = SyncService::new(config);

// 全体同期
let result = sync_service.sync_full(&client).await?;
println!("✅ Synced {} issues", result.synced_issues_count);

// 時間ベースフィルタ
let filter = TimeBasedFilter::last_hours(24);
if let Some(jql) = filter.to_jql_time_condition() {
    println!("Generated JQL: {}", jql);
}
```

### 変更履歴

```rust
use jira_api::{HistoryFilter, HistorySortOrder};

let history_filter = HistoryFilter::new()
    .issue_keys(vec!["PROJ-123".to_string()])
    .since(chrono::Utc::now() - chrono::Duration::days(30))
    .sort_order(HistorySortOrder::NewestFirst);

let history = duckdb_store.get_issue_history(&history_filter).await?;
println!("📊 Found {} history records", history.len());
```

## 📖 ドキュメント

- **[API ドキュメント](https://docs.rs/jira-api)** - 完全なAPIリファレンス
- **[使用例](docs/usage_examples.md)** - 詳細な使用例とサンプルコード
- **[設計仕様](docs/2.spec.md)** - アーキテクチャと設計方針
- **[タスクリスト](docs/3.task_list.md)** - 開発進捗と計画

### サンプルコード

`examples/`ディレクトリに豊富なサンプルを用意：

```bash
# 基本的な使用方法
cargo run --example basic_usage

# 検索機能
cargo run --example search_example

# データ永続化
cargo run --example persistence_example

# 同期機能
cargo run --example sync_example

# 変更履歴
cargo run --example history_example

# ハイブリッド統合テスト
cargo run --example hybrid_integration_example
```

## 🧪 テスト

```bash
# 全テスト実行
cargo test

# 統合テスト
cargo test --test integration_tests

# パフォーマンステスト
cargo test --test performance_tests

# エラーシナリオテスト  
cargo test --test error_scenario_tests
```

## 🔧 開発

### 必要な環境

- Rust 1.70.0 以上
- JIRA API トークン（テスト用）

### 開発用コマンド

```bash
# プロジェクトをビルド
cargo build

# 全テスト実行
cargo test

# サンプル実行
cargo run --example basic_usage

# ドキュメント生成
cargo doc --open

# フォーマット
cargo fmt

# Lint
cargo clippy

# 継続的テスト実行（cargo-watch使用）
cargo watch -x test
```

### JIRAインスタンスの設定

1. JIRA管理画面で APIトークンを生成
2. 環境変数またはConfigでURL、ユーザー、トークンを設定
3. 必要に応じてプロジェクトキーやフィルターを調整

## 🏗️ アーキテクチャ

```
jira-api/
├── src/
│   ├── client.rs          # JIRA API クライアント
│   ├── models/            # データモデル定義
│   ├── sync.rs            # 同期機能
│   ├── time_filter.rs     # 時間ベースフィルター
│   ├── persistence.rs     # データ永続化抽象化
│   ├── json_store.rs      # JSON ストア実装
│   ├── duckdb_store.rs    # DuckDB ストア実装
│   ├── history.rs         # 変更履歴管理
│   ├── config_store.rs    # 設定管理
│   ├── memory.rs          # メモリ管理
│   └── error.rs           # エラー定義
├── examples/              # サンプルコード
├── tests/                 # テストスイート
└── docs/                  # ドキュメント
```

## 🤝 コントリビューション

コントリビューションを歓迎します！以下の手順でお願いします：

1. このリポジトリをフォーク
2. フィーチャーブランチを作成 (`git checkout -b feature/amazing-feature`)
3. 変更をコミット (`git commit -m 'Add some amazing feature'`)
4. ブランチにプッシュ (`git push origin feature/amazing-feature`)
5. プルリクエストを作成

### 開発ガイドライン

- TDD（Test-Driven Development）アプローチを採用
- 全ての新機能にはテストが必要
- コードフォーマットは `cargo fmt` で統一
- `cargo clippy` でのlintチェックをパス
- コミットメッセージは明確で説明的に

## 📄 ライセンス

このプロジェクトは [MIT License](LICENSE) の下でライセンスされています。

## 🙏 謝辞

- [Atlassian JIRA REST API](https://developer.atlassian.com/server/jira/platform/rest-apis/) のドキュメント
- Rustコミュニティの素晴らしいライブラリ群
- コントリビューターの皆様

## 📞 サポート

- 🐛 **バグ報告**: [GitHub Issues](https://github.com/your-username/jira-api/issues)
- 💡 **機能リクエスト**: [GitHub Issues](https://github.com/your-username/jira-api/issues)
- 📖 **ドキュメント**: [docs.rs](https://docs.rs/jira-api)
- 💬 **ディスカッション**: [GitHub Discussions](https://github.com/your-username/jira-api/discussions)

---

**Made with ❤️ for the Rust community**