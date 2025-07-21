# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

このプロジェクトはJIRA APIとやり取りするためのクライアントを提供するRustライブラリ（`jira-api`）です。特定のJIRA REST APIエンドポイントをラップすることで、JIRAのデータ操作を扱いやすくすることを目的としています。

## 対応するJIRA APIエンドポイント

このライブラリは以下の特定のJIRA APIエンドポイントをサポートするよう設計されています：
- `/rest/api/3/search`
- `/rest/api/3/project`
- `/rest/api/3/priority`
- `/rest/api/3/issuetype`
- `/rest/api/3/field`
- `/rest/api/3/statuscategory`
- `/rest/api/3/users/search`

## 主要なアーキテクチャコンポーネント

- **`src/lib.rs`**: メインライブラリのエントリーポイント、パブリックAPIの再エクスポート
- **`src/client.rs`**: JIRA APIとの相互作用のための`JiraClient`、`JiraConfig`、`Auth`タイプを含む
- **`src/error.rs`**: API、認証、データ処理エラーに対する`thiserror`を使用した包括的なエラーハンドリング
- **`examples/basic_usage.rs`**: 環境変数を使用した設定セットアップのデモンストレーション

## 認証

ライブラリは`Auth`列挙型を通じて2つの認証方法をサポートします：
- ユーザー名とAPIトークンによるBasic認証
- Bearerトークン認証

設定は環境変数を通じて処理されます：
- `JIRA_URL`: JIRAインスタンスのベースURL
- `JIRA_USER`: Basic認証用のユーザー名
- `JIRA_API_TOKEN`: Basic認証用のAPIトークン

## 開発用コマンド

```bash
# プロジェクトをビルド
cargo build

# テストを実行
cargo test

# 基本使用例を実行
cargo run --example basic_usage

# cargo-watchでテストを実行（インストール済みの場合）
cargo watch -x test

# リントの問題をチェック
cargo clippy

# コードをフォーマット
cargo fmt
```

## 独自機能（要求仕様書より）

1. **差分データ取得**: 完全なデータセットを取得するために増分データの取得をサポート
2. **データ永続化**: JSON（gzip圧縮）またはDuckDB形式でのデータ保存をサポート
3. **設定の永続化**: 認証情報とフィルター条件の保存機能
4. **時間ベースフィルタリング**: JIRAの時間（hour）粒度の時間フィルタリングを、既に取得済みのチケットの除外と共に処理

## 依存関係

主要な依存関係は以下の通りです：
- `reqwest`: JSONとTLSサポート付きHTTPクライアント
- `serde`/`serde_json`: シリアライゼーション
- `tokio`: 非同期ランタイム
- `thiserror`: エラーハンドリング
- `dotenv`: 環境変数の読み込み
- `url`: URL解析と検証