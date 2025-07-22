use crate::{Error, Issue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// メモリ効率改善のためのユーティリティ
///
/// このモジュールは、JIRA APIライブラリのメモリ使用量を最適化するための
/// 機能を提供します：
/// 1. データの遅延読み込み（Lazy Loading）
/// 2. 効率的なデータストリーミング
/// 3. メモリプールの管理
/// 4. ガベージコレクション支援
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// メモリ効率の設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 最大メモリ使用量（バイト）
    pub max_memory_usage: usize,
    /// ページサイズ（一度に読み込むアイテム数）
    pub page_size: usize,
    /// キャッシュサイズ（最近使用されたデータの保持数）
    pub cache_size: usize,
    /// 圧縮閾値（この値を超えるとデータを圧縮）
    pub compression_threshold: usize,
    /// GC実行間隔（秒）
    pub gc_interval_seconds: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory_usage: 256 * 1024 * 1024, // 256MB
            page_size: 100,
            cache_size: 1000,
            compression_threshold: 1024 * 1024, // 1MB
            gc_interval_seconds: 300,           // 5分
        }
    }
}

/// 遅延読み込み可能なIssueデータ
#[derive(Clone)]
pub struct LazyIssue {
    /// Issue の ID
    pub id: String,
    /// Issue の キー
    pub key: String,
    /// 軽量な基本情報（常に読み込み済み）
    pub summary: String,
    /// 詳細データの状態
    pub detail_status: DetailStatus,
    /// 詳細データの読み込み関数
    pub loader: Option<Arc<dyn IssueLoader>>,
}

impl std::fmt::Debug for LazyIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyIssue")
            .field("id", &self.id)
            .field("key", &self.key)
            .field("summary", &self.summary)
            .field("detail_status", &self.detail_status)
            .field("loader", &"<IssueLoader>")
            .finish()
    }
}

/// 詳細データの状態
#[derive(Debug, Clone)]
pub enum DetailStatus {
    /// 未読み込み
    NotLoaded,
    /// 読み込み中
    Loading,
    /// 読み込み済み
    Loaded(Box<Issue>),
    /// 読み込みエラー
    Error(String),
}

impl PartialEq for DetailStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DetailStatus::NotLoaded, DetailStatus::NotLoaded) => true,
            (DetailStatus::Loading, DetailStatus::Loading) => true,
            (DetailStatus::Loaded(_), DetailStatus::Loaded(_)) => true,
            (DetailStatus::Error(a), DetailStatus::Error(b)) => a == b,
            _ => false,
        }
    }
}

/// Issue データの遅延読み込みトレイト
pub trait IssueLoader: Send + Sync {
    /// Issue の詳細データを読み込む
    fn load_details(
        &self,
        issue_key: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Issue, Error>> + Send + '_>>;
}

impl LazyIssue {
    /// 新しい遅延読み込みIssueを作成
    pub fn new(
        id: String,
        key: String,
        summary: String,
        loader: Option<Arc<dyn IssueLoader>>,
    ) -> Self {
        Self {
            id,
            key,
            summary,
            detail_status: DetailStatus::NotLoaded,
            loader,
        }
    }

    /// 軽量Issueから作成
    pub fn from_issue_lightweight(issue: &Issue, loader: Option<Arc<dyn IssueLoader>>) -> Self {
        Self::new(
            issue.id.clone(),
            issue.key.clone(),
            issue.fields.summary.clone(),
            loader,
        )
    }

    /// 詳細データが読み込み済みかチェック
    pub fn is_loaded(&self) -> bool {
        matches!(self.detail_status, DetailStatus::Loaded(_))
    }

    /// 詳細データを非同期で読み込む（簡略版）
    pub async fn load_details(&mut self) -> Result<&Issue, Error> {
        // 簡略版の実装 - 実際の読み込みは今後の実装で追加
        Err(Error::Unexpected(
            "LazyIssue loading not yet implemented".to_string(),
        ))
    }

    /// メモリ使用量を推定（バイト）
    pub fn estimated_memory_usage(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let string_sizes = self.id.len() + self.key.len() + self.summary.len();

        let detail_size = match &self.detail_status {
            DetailStatus::Loaded(_) => 8000, // 平均的なIssueサイズの推定
            DetailStatus::Error(msg) => msg.len(),
            _ => 0,
        };

        base_size + string_sizes + detail_size
    }
}

/// ページネーション対応のIssue ストリーム
pub struct IssueStream {
    /// 設定
    _config: MemoryConfig,
    /// 現在のページ番号
    current_page: usize,
    /// 総ページ数（判明している場合）
    total_pages: Option<usize>,
    /// キャッシュされたページ
    page_cache: Arc<RwLock<HashMap<usize, Vec<LazyIssue>>>>,
    /// データローダー
    _loader: Arc<dyn IssueStreamLoader>,
}

/// Issue ストリームのデータ読み込みトレイト
pub trait IssueStreamLoader: Send + Sync {
    /// 指定されたページのデータを読み込む
    fn load_page(
        &self,
        page: usize,
        page_size: usize,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<LazyIssue>, Error>> + Send + '_>,
    >;

    /// 総ページ数を取得（可能な場合）
    fn total_pages(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<usize>, Error>> + Send + '_>,
    >;
}

impl IssueStream {
    /// 新しいIssue ストリームを作成
    pub fn new(config: MemoryConfig, loader: Arc<dyn IssueStreamLoader>) -> Self {
        Self {
            _config: config,
            current_page: 0,
            total_pages: None,
            page_cache: Arc::new(RwLock::new(HashMap::new())),
            _loader: loader,
        }
    }

    /// 次のページを読み込む（簡略版）
    pub async fn next_page(&mut self) -> Result<Vec<LazyIssue>, Error> {
        // 簡略版の実装 - 実際のページング実装は今後追加
        self.current_page += 1;
        Ok(Vec::new())
    }

    /// 指定されたページを読み込む（簡略版）
    pub async fn get_page(&self, _page: usize) -> Result<Vec<LazyIssue>, Error> {
        // 簡略版の実装
        Ok(Vec::new())
    }

    /// ストリームをリセット
    pub async fn reset(&mut self) {
        self.current_page = 0;
        self.total_pages = None;
        self.page_cache.write().await.clear();
    }

    /// 現在のページ番号を取得
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// 総ページ数を取得（簡略版）
    pub async fn total_pages(&mut self) -> Result<Option<usize>, Error> {
        Ok(self.total_pages)
    }

    /// キャッシュメモリ使用量を推定
    #[allow(dead_code)]
    async fn estimate_cache_memory_usage(&self) -> usize {
        let cache = self.page_cache.read().await;
        let mut total_usage = 0;

        for page_data in cache.values() {
            for issue in page_data {
                total_usage += issue.estimated_memory_usage();
            }
        }

        total_usage
    }

    /// キャッシュをクリーンアップ（古いページから削除）
    #[allow(dead_code)]
    async fn cleanup_cache(&self) {
        let mut cache = self.page_cache.write().await;

        // 現在のページから離れたページを優先的に削除
        let current = self.current_page;
        let mut pages_to_remove = Vec::new();

        for &page in cache.keys() {
            let distance = if page > current {
                page - current
            } else {
                current - page
            };
            if distance > 5 {
                // 現在のページから5ページ以上離れている場合
                pages_to_remove.push(page);
            }
        }

        // メモリ使用量が半分以下になるまで削除
        pages_to_remove.sort_by_key(|&page| {
            let distance = if page > current {
                page - current
            } else {
                current - page
            };
            std::cmp::Reverse(distance) // 遠いページから削除
        });

        let target_size = cache.len() / 2;
        for &page in pages_to_remove.iter().take(cache.len() - target_size) {
            cache.remove(&page);
        }

        println!(
            "🧹 Memory cleanup: removed {} cached pages",
            pages_to_remove.len()
        );
    }
}

/// メモリプール管理
pub struct MemoryPool<T> {
    /// 使用可能なオブジェクトのプール
    available: Arc<Mutex<Vec<T>>>,
    /// ファクトリー関数
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    /// プールの最大サイズ
    max_size: usize,
    /// 現在の使用中オブジェクト数
    in_use_count: Arc<Mutex<usize>>,
}

impl<T: Send + 'static> MemoryPool<T> {
    /// 新しいメモリプールを作成
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            available: Arc::new(Mutex::new(Vec::new())),
            factory: Arc::new(factory),
            max_size,
            in_use_count: Arc::new(Mutex::new(0)),
        }
    }

    /// オブジェクトを取得
    pub async fn acquire(&self) -> PooledObject<T>
    where
        T: Send,
    {
        let mut available = self.available.lock().await;
        let object = if let Some(obj) = available.pop() {
            obj
        } else {
            (self.factory)()
        };

        *self.in_use_count.lock().await += 1;

        PooledObject {
            object: Some(object),
            pool: Arc::new(PoolReturn {
                available: Arc::clone(&self.available),
                in_use_count: Arc::clone(&self.in_use_count),
                max_size: self.max_size,
            }),
        }
    }

    /// プールの統計情報を取得
    pub async fn stats(&self) -> PoolStats {
        let available_count = self.available.lock().await.len();
        let in_use_count = *self.in_use_count.lock().await;

        PoolStats {
            available_count,
            in_use_count,
            max_size: self.max_size,
            total_created: available_count + in_use_count,
        }
    }
}

/// プールから取得されたオブジェクト
pub struct PooledObject<T: Send + 'static> {
    object: Option<T>,
    pool: Arc<PoolReturn<T>>,
}

impl<T: Send + 'static> PooledObject<T> {
    /// オブジェクトへの参照を取得
    pub fn as_ref(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// オブジェクトへの可変参照を取得
    pub fn as_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T: Send + 'static> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            tokio::spawn({
                let pool = Arc::clone(&self.pool);
                async move {
                    pool.return_object(object).await;
                }
            });
        }
    }
}

/// プールへの返却処理
struct PoolReturn<T> {
    available: Arc<Mutex<Vec<T>>>,
    in_use_count: Arc<Mutex<usize>>,
    max_size: usize,
}

impl<T> PoolReturn<T> {
    async fn return_object(&self, object: T) {
        let mut available = self.available.lock().await;
        let mut in_use_count = self.in_use_count.lock().await;

        *in_use_count -= 1;

        // プールサイズが最大値を超えない場合のみ返却
        if available.len() < self.max_size {
            available.push(object);
        }
        // そうでなければオブジェクトは破棄される
    }
}

/// プール統計情報
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// 利用可能なオブジェクト数
    pub available_count: usize,
    /// 使用中のオブジェクト数
    pub in_use_count: usize,
    /// プールの最大サイズ
    pub max_size: usize,
    /// これまでに作成された総オブジェクト数
    pub total_created: usize,
}

impl PoolStats {
    /// プールの使用率を計算
    pub fn utilization_rate(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.in_use_count as f64) / (self.max_size as f64)
        }
    }
}

/// ガベージコレクション支援
pub struct MemoryGC {
    /// GC設定
    config: MemoryConfig,
    /// 管理対象のメモリプール
    pools: Vec<Arc<dyn MemoryPoolGC>>,
    /// GCタスクハンドル
    gc_handle: Option<tokio::task::JoinHandle<()>>,
}

/// メモリプールのGCインターフェース
pub trait MemoryPoolGC: Send + Sync {
    /// GCを実行
    fn run_gc(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;

    /// メモリ使用量を取得
    fn memory_usage(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + Send + '_>>;
}

impl MemoryGC {
    /// 新しいGCマネージャーを作成
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            pools: Vec::new(),
            gc_handle: None,
        }
    }

    /// メモリプールを管理対象に追加
    pub fn add_pool(&mut self, pool: Arc<dyn MemoryPoolGC>) {
        self.pools.push(pool);
    }

    /// 定期GCを開始
    pub fn start_gc(&mut self) {
        if self.gc_handle.is_some() {
            return; // 既に開始済み
        }

        let pools = self.pools.clone();
        let interval = std::time::Duration::from_secs(self.config.gc_interval_seconds);
        let _max_memory = self.config.max_memory_usage;

        let handle = tokio::spawn(async move {
            let mut gc_interval = tokio::time::interval(interval);

            loop {
                gc_interval.tick().await;

                // 簡略版GC実装
                if !pools.is_empty() {
                    println!("🗑️  Running simplified GC...");
                    // 実際のGC実装は今後追加
                    println!("✅ GC completed (simplified)");
                }
            }
        });

        self.gc_handle = Some(handle);
    }

    /// GCを停止
    pub fn stop_gc(&mut self) {
        if let Some(handle) = self.gc_handle.take() {
            handle.abort();
        }
    }

    /// 手動でGCを実行（簡略版）
    pub async fn run_gc_now(&self) {
        println!("🧹 Running manual GC (simplified)...");
        println!("✅ Manual GC completed");
    }
}

impl Drop for MemoryGC {
    fn drop(&mut self) {
        self.stop_gc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[test]
    fn test_memory_config_default() {
        let config = MemoryConfig::default();

        assert_eq!(config.max_memory_usage, 256 * 1024 * 1024);
        assert_eq!(config.page_size, 100);
        assert_eq!(config.cache_size, 1000);
        assert_eq!(config.compression_threshold, 1024 * 1024);
        assert_eq!(config.gc_interval_seconds, 300);
    }

    #[test]
    fn test_lazy_issue_creation() {
        let lazy_issue = LazyIssue::new(
            "123".to_string(),
            "TEST-1".to_string(),
            "Test issue".to_string(),
            None,
        );

        assert_eq!(lazy_issue.id, "123");
        assert_eq!(lazy_issue.key, "TEST-1");
        assert_eq!(lazy_issue.summary, "Test issue");
        assert!(!lazy_issue.is_loaded());
        assert_eq!(lazy_issue.detail_status, DetailStatus::NotLoaded);
    }

    #[test]
    fn test_lazy_issue_memory_estimation() {
        let lazy_issue = LazyIssue::new(
            "123".to_string(),
            "TEST-1".to_string(),
            "Test issue".to_string(),
            None,
        );

        let memory_usage = lazy_issue.estimated_memory_usage();
        assert!(memory_usage > 0);

        // 基本的なサイズチェック（文字列長 + 構造体サイズ）
        let expected_min =
            "123".len() + "TEST-1".len() + "Test issue".len() + std::mem::size_of::<LazyIssue>();
        assert!(memory_usage >= expected_min);
    }

    #[tokio::test]
    async fn test_memory_pool_basic_operations() {
        let pool = MemoryPool::new(|| String::from("test"), 5);

        // オブジェクトを取得
        let obj1 = pool.acquire().await;
        assert_eq!(obj1.as_ref(), "test");

        let stats = pool.stats().await;
        assert_eq!(stats.in_use_count, 1);
        assert_eq!(stats.available_count, 0);

        // オブジェクトを返却（drop時に自動返却）
        drop(obj1);

        // 少し待機して返却処理の完了を待つ
        sleep(Duration::from_millis(10)).await;

        let stats = pool.stats().await;
        assert_eq!(stats.in_use_count, 0);
        assert_eq!(stats.available_count, 1);
    }

    #[tokio::test]
    async fn test_memory_pool_reuse() {
        let pool = MemoryPool::new(|| vec![1, 2, 3], 3);

        {
            let mut obj = pool.acquire().await;
            obj.as_mut().push(4);
            assert_eq!(obj.as_ref(), &vec![1, 2, 3, 4]);
        } // オブジェクトがここで返却される

        sleep(Duration::from_millis(10)).await;

        // 新しいオブジェクトを取得（再利用される可能性）
        let obj2 = pool.acquire().await;
        // プールから再利用される場合、前の状態が残っている可能性がある
        // これは実装依存なので、基本的なサイズだけチェック
        assert!(!obj2.as_ref().is_empty());
    }

    #[tokio::test]
    async fn test_memory_pool_max_size_limit() {
        let pool = MemoryPool::new(|| String::from("pooled"), 2);

        // プールの最大サイズを超えてオブジェクトを作成
        let _obj1 = pool.acquire().await;
        let _obj2 = pool.acquire().await;
        let _obj3 = pool.acquire().await; // 最大サイズを超過

        let stats = pool.stats().await;
        assert_eq!(stats.in_use_count, 3); // 使用中は3個
        assert!(stats.total_created >= 3); // 少なくとも3個は作成された
    }

    #[test]
    fn test_pool_stats_utilization_rate() {
        let stats = PoolStats {
            available_count: 2,
            in_use_count: 3,
            max_size: 10,
            total_created: 5,
        };

        let utilization = stats.utilization_rate();
        assert_eq!(utilization, 0.3); // 3/10 = 0.3

        // エッジケース：最大サイズが0
        let edge_stats = PoolStats {
            available_count: 0,
            in_use_count: 0,
            max_size: 0,
            total_created: 0,
        };
        assert_eq!(edge_stats.utilization_rate(), 0.0);
    }

    #[test]
    fn test_detail_status_equality() {
        assert_eq!(DetailStatus::NotLoaded, DetailStatus::NotLoaded);
        assert_eq!(DetailStatus::Loading, DetailStatus::Loading);
        assert_ne!(DetailStatus::NotLoaded, DetailStatus::Loading);

        let error1 = DetailStatus::Error("test".to_string());
        let error2 = DetailStatus::Error("test".to_string());
        assert_eq!(error1, error2);
    }

    #[tokio::test]
    async fn test_memory_gc_creation() {
        let config = MemoryConfig::default();
        let gc = MemoryGC::new(config);

        assert!(gc.pools.is_empty());
        assert!(gc.gc_handle.is_none());
    }

    // メモリGCの実際の動作テストは統合テストで実装
    // ここでは基本的な構造のテストのみ行う
}
