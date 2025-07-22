use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// 時間ベースフィルタリングの設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedFilter {
    /// 開始時刻（この時刻以降に作成・更新されたIssueを取得）
    pub since: Option<DateTime<Utc>>,
    /// 終了時刻（この時刻以前に作成・更新されたIssueを取得）
    pub until: Option<DateTime<Utc>>,
    /// 時間粒度（時間単位）
    pub granularity_hours: u32,
    /// 作成時刻での絞り込み
    pub filter_by_created: bool,
    /// 更新時刻での絞り込み
    pub filter_by_updated: bool,
    /// 既存取得済みIssueの除外を行うかどうか
    pub exclude_existing: bool,
    /// 除外対象のIssueキー一覧
    pub excluded_issue_keys: Vec<String>,
}

impl TimeBasedFilter {
    /// 新しい時間ベースフィルターを作成
    pub fn new() -> Self {
        Self {
            since: None,
            until: None,
            granularity_hours: 1,
            filter_by_created: true,
            filter_by_updated: true,
            exclude_existing: true,
            excluded_issue_keys: Vec::new(),
        }
    }

    /// 開始時刻を設定
    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// 終了時刻を設定
    pub fn until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    /// 時間粒度を設定（時間単位）
    pub fn granularity_hours(mut self, hours: u32) -> Self {
        self.granularity_hours = hours;
        self
    }

    /// 作成時刻での絞り込みを設定
    pub fn filter_by_created(mut self, enabled: bool) -> Self {
        self.filter_by_created = enabled;
        self
    }

    /// 更新時刻での絞り込みを設定
    pub fn filter_by_updated(mut self, enabled: bool) -> Self {
        self.filter_by_updated = enabled;
        self
    }

    /// 既存Issue除外を設定
    pub fn exclude_existing(mut self, enabled: bool) -> Self {
        self.exclude_existing = enabled;
        self
    }

    /// 除外対象Issueキーを追加
    pub fn add_excluded_issue_key(mut self, issue_key: String) -> Self {
        self.excluded_issue_keys.push(issue_key);
        self
    }

    /// 除外対象Issueキー一覧を設定
    pub fn excluded_issue_keys(mut self, issue_keys: Vec<String>) -> Self {
        self.excluded_issue_keys = issue_keys;
        self
    }

    /// 最近N時間のフィルターを作成
    pub fn last_hours(hours: u32) -> Self {
        let now = Utc::now();
        let since = now - Duration::hours(hours as i64);

        Self::new().since(since).until(now).granularity_hours(1)
    }

    /// 最近N日のフィルターを作成
    pub fn last_days(days: u32) -> Self {
        let now = Utc::now();
        let since = now - Duration::days(days as i64);

        Self::new().since(since).until(now).granularity_hours(24)
    }

    /// 指定した日付範囲のフィルターを作成
    pub fn date_range(start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Self {
        Self::new()
            .since(start_date)
            .until(end_date)
            .granularity_hours(24)
    }

    /// 増分取得用のフィルターを作成（最後の同期時刻以降）
    pub fn incremental_since(last_sync_time: DateTime<Utc>) -> Self {
        let now = Utc::now();

        Self::new()
            .since(last_sync_time)
            .until(now)
            .filter_by_updated(true) // 更新時刻でのフィルタリングを有効化
            .exclude_existing(true)
            .granularity_hours(1)
    }

    /// JQLクエリの時間条件部分を生成
    pub fn to_jql_time_condition(&self) -> Option<String> {
        let mut conditions = Vec::new();

        // 作成時刻による条件
        if self.filter_by_created {
            if let Some(since) = &self.since {
                let since_str = format_jira_datetime(since);
                conditions.push(format!("created >= '{}'", since_str));
            }

            if let Some(until) = &self.until {
                let until_str = format_jira_datetime(until);
                conditions.push(format!("created <= '{}'", until_str));
            }
        }

        // 更新時刻による条件
        if self.filter_by_updated {
            if let Some(since) = &self.since {
                let since_str = format_jira_datetime(since);
                conditions.push(format!("updated >= '{}'", since_str));
            }

            if let Some(until) = &self.until {
                let until_str = format_jira_datetime(until);
                conditions.push(format!("updated <= '{}'", until_str));
            }
        }

        // 除外対象Issueキーによる条件
        if self.exclude_existing && !self.excluded_issue_keys.is_empty() {
            let keys_str = self
                .excluded_issue_keys
                .iter()
                .map(|k| format!("'{}'", k))
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("key NOT IN ({})", keys_str));
        }

        if conditions.is_empty() {
            None
        } else {
            // 作成時刻と更新時刻の条件がある場合はORで結合
            if self.filter_by_created && self.filter_by_updated {
                let created_conditions: Vec<_> = conditions
                    .iter()
                    .filter(|c| c.contains("created"))
                    .cloned()
                    .collect();
                let updated_conditions: Vec<_> = conditions
                    .iter()
                    .filter(|c| c.contains("updated"))
                    .cloned()
                    .collect();
                let other_conditions: Vec<_> = conditions
                    .iter()
                    .filter(|c| !c.contains("created") && !c.contains("updated"))
                    .cloned()
                    .collect();

                let mut result_conditions = Vec::new();

                if !created_conditions.is_empty() || !updated_conditions.is_empty() {
                    let time_condition = format!(
                        "({})",
                        [created_conditions, updated_conditions]
                            .concat()
                            .join(" OR ")
                    );
                    result_conditions.push(time_condition);
                }

                result_conditions.extend(other_conditions);
                Some(result_conditions.join(" AND "))
            } else {
                Some(conditions.join(" AND "))
            }
        }
    }

    /// フィルターが有効かどうかチェック
    pub fn is_valid(&self) -> Result<(), String> {
        // 開始時刻が終了時刻より後の場合はエラー
        if let (Some(since), Some(until)) = (&self.since, &self.until) {
            if since > until {
                return Err("開始時刻が終了時刻より後に設定されています".to_string());
            }
        }

        // 時間粒度が0の場合はエラー
        if self.granularity_hours == 0 {
            return Err("時間粒度は1以上である必要があります".to_string());
        }

        // 作成時刻・更新時刻両方が無効な場合はエラー
        if !self.filter_by_created && !self.filter_by_updated {
            return Err("作成時刻または更新時刻による絞り込みが必要です".to_string());
        }

        Ok(())
    }

    /// 時間範囲を時間粒度で分割
    pub fn split_into_chunks(&self) -> Vec<TimeChunk> {
        let mut chunks = Vec::new();

        let (start, end) = match (&self.since, &self.until) {
            (Some(s), Some(e)) => (*s, *e),
            (Some(s), None) => (*s, Utc::now()),
            (None, Some(e)) => (*e - Duration::days(30), *e), // デフォルト30日前から
            (None, None) => {
                let now = Utc::now();
                (now - Duration::days(30), now)
            }
        };

        let chunk_duration = Duration::hours(self.granularity_hours as i64);
        let mut current = start;

        while current < end {
            let chunk_end = std::cmp::min(current + chunk_duration, end);
            chunks.push(TimeChunk {
                start: current,
                end: chunk_end,
            });
            current = chunk_end;
        }

        chunks
    }
}

impl Default for TimeBasedFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// 時間チャンク（時間粒度で分割された時間範囲）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeChunk {
    /// チャンクの開始時刻
    pub start: DateTime<Utc>,
    /// チャンクの終了時刻
    pub end: DateTime<Utc>,
}

impl TimeChunk {
    /// 新しい時間チャンクを作成
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// チャンクの期間（秒）を取得
    pub fn duration_seconds(&self) -> i64 {
        (self.end - self.start).num_seconds()
    }

    /// チャンクの期間（時間）を取得
    pub fn duration_hours(&self) -> f64 {
        self.duration_seconds() as f64 / 3600.0
    }

    /// このチャンク用のJQL時間条件を生成
    pub fn to_jql_condition(&self, filter_by_created: bool, filter_by_updated: bool) -> String {
        let start_str = format_jira_datetime(&self.start);
        let end_str = format_jira_datetime(&self.end);

        let mut conditions = Vec::new();

        if filter_by_created {
            conditions.push(format!(
                "created >= '{}' AND created <= '{}'",
                start_str, end_str
            ));
        }

        if filter_by_updated {
            conditions.push(format!(
                "updated >= '{}' AND updated <= '{}'",
                start_str, end_str
            ));
        }

        if conditions.is_empty() {
            format!("created >= '{}' AND created <= '{}'", start_str, end_str)
        } else {
            format!("({})", conditions.join(" OR "))
        }
    }
}

/// DateTime<Utc>をJIRA用の日時文字列にフォーマット
fn format_jira_datetime(dt: &DateTime<Utc>) -> String {
    // JIRA APIでは "YYYY-MM-DD HH:mm" フォーマットを使用
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// JIRA用の日時文字列を`DateTime<Utc>`にパース
pub fn parse_jira_datetime(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    let naive_dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")?;
    Ok(naive_dt.and_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    #[test]
    fn test_time_based_filter_new() {
        // TimeBasedFilter::new()でデフォルト値が正しく設定されることをテスト
        let filter = TimeBasedFilter::new();

        assert!(filter.since.is_none());
        assert!(filter.until.is_none());
        assert_eq!(filter.granularity_hours, 1);
        assert_eq!(filter.filter_by_created, true);
        assert_eq!(filter.filter_by_updated, true);
        assert_eq!(filter.exclude_existing, true);
        assert!(filter.excluded_issue_keys.is_empty());
    }

    #[test]
    fn test_time_based_filter_builder_pattern() {
        // TimeBasedFilterのビルダーパターンが正しく動作することをテスト
        let since = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();

        let filter = TimeBasedFilter::new()
            .since(since)
            .until(until)
            .granularity_hours(24)
            .filter_by_created(false)
            .filter_by_updated(true)
            .exclude_existing(false)
            .add_excluded_issue_key("TEST-1".to_string())
            .add_excluded_issue_key("TEST-2".to_string());

        assert_eq!(filter.since, Some(since));
        assert_eq!(filter.until, Some(until));
        assert_eq!(filter.granularity_hours, 24);
        assert_eq!(filter.filter_by_created, false);
        assert_eq!(filter.filter_by_updated, true);
        assert_eq!(filter.exclude_existing, false);
        assert_eq!(filter.excluded_issue_keys, vec!["TEST-1", "TEST-2"]);
    }

    #[test]
    fn test_time_based_filter_last_hours() {
        // TimeBasedFilter::last_hours()が正しく動作することをテスト
        let filter = TimeBasedFilter::last_hours(24);

        assert!(filter.since.is_some());
        assert!(filter.until.is_some());
        assert_eq!(filter.granularity_hours, 1);

        let now = Utc::now();
        let since = filter.since.unwrap();
        let until = filter.until.unwrap();

        // 24時間前後の範囲内かチェック
        assert!((now - since).num_hours().abs() <= 24);
        assert!(until <= now);
    }

    #[test]
    fn test_time_based_filter_last_days() {
        // TimeBasedFilter::last_days()が正しく動作することをテスト
        let filter = TimeBasedFilter::last_days(7);

        assert!(filter.since.is_some());
        assert!(filter.until.is_some());
        assert_eq!(filter.granularity_hours, 24);

        let now = Utc::now();
        let since = filter.since.unwrap();

        // 7日前後の範囲内かチェック
        assert!((now - since).num_days().abs() <= 7);
    }

    #[test]
    fn test_time_based_filter_date_range() {
        // TimeBasedFilter::date_range()が正しく動作することをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();

        let filter = TimeBasedFilter::date_range(start, end);

        assert_eq!(filter.since, Some(start));
        assert_eq!(filter.until, Some(end));
        assert_eq!(filter.granularity_hours, 24);
    }

    #[test]
    fn test_time_based_filter_incremental_since() {
        // TimeBasedFilter::incremental_since()が正しく動作することをテスト
        let last_sync = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let filter = TimeBasedFilter::incremental_since(last_sync);

        assert_eq!(filter.since, Some(last_sync));
        assert!(filter.until.is_some());
        assert_eq!(filter.filter_by_updated, true);
        assert_eq!(filter.exclude_existing, true);
        assert_eq!(filter.granularity_hours, 1);
    }

    #[test]
    fn test_time_based_filter_validation() {
        // TimeBasedFilter::is_valid()が正しく動作することをテスト
        let valid_filter = TimeBasedFilter::new();
        assert!(valid_filter.is_valid().is_ok());

        // 開始時刻が終了時刻より後
        let invalid_time_filter = TimeBasedFilter::new()
            .since(Utc.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap())
            .until(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap());
        assert!(invalid_time_filter.is_valid().is_err());

        // 時間粒度が0
        let invalid_granularity_filter = TimeBasedFilter::new().granularity_hours(0);
        assert!(invalid_granularity_filter.is_valid().is_err());

        // 作成時刻・更新時刻両方が無効
        let invalid_filter_fields = TimeBasedFilter::new()
            .filter_by_created(false)
            .filter_by_updated(false);
        assert!(invalid_filter_fields.is_valid().is_err());
    }

    #[test]
    fn test_time_based_filter_to_jql_condition() {
        // TimeBasedFilter::to_jql_time_condition()が正しいJQLを生成することをテスト
        let since = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2024, 1, 31, 12, 0, 0).unwrap();

        let filter = TimeBasedFilter::new()
            .since(since)
            .until(until)
            .filter_by_created(true)
            .filter_by_updated(false)
            .exclude_existing(false);

        let jql = filter.to_jql_time_condition().unwrap();

        assert!(jql.contains("created >= '2024-01-01 12:00'"));
        assert!(jql.contains("created <= '2024-01-31 12:00'"));
        assert!(!jql.contains("updated"));
    }

    #[test]
    fn test_time_based_filter_to_jql_with_exclusion() {
        // 除外条件付きのJQL生成をテスト
        let filter = TimeBasedFilter::new()
            .excluded_issue_keys(vec!["TEST-1".to_string(), "TEST-2".to_string()])
            .exclude_existing(true);

        let jql = filter.to_jql_time_condition().unwrap();
        assert!(jql.contains("key NOT IN ('TEST-1', 'TEST-2')"));
    }

    #[test]
    fn test_time_chunk_new() {
        // TimeChunk::new()で正しく作成されることをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 1, 1, 0, 0).unwrap();

        let chunk = TimeChunk::new(start, end);

        assert_eq!(chunk.start, start);
        assert_eq!(chunk.end, end);
        assert_eq!(chunk.duration_hours(), 1.0);
        assert_eq!(chunk.duration_seconds(), 3600);
    }

    #[test]
    fn test_time_chunk_to_jql_condition() {
        // TimeChunk::to_jql_condition()が正しいJQLを生成することをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 1, 13, 0, 0).unwrap();

        let chunk = TimeChunk::new(start, end);

        // 作成時刻のみでフィルタ
        let jql_created = chunk.to_jql_condition(true, false);
        assert!(jql_created.contains("created >= '2024-01-01 12:00'"));
        assert!(jql_created.contains("created <= '2024-01-01 13:00'"));
        assert!(!jql_created.contains("updated"));

        // 作成時刻と更新時刻でフィルタ
        let jql_both = chunk.to_jql_condition(true, true);
        assert!(jql_both.contains("created"));
        assert!(jql_both.contains("updated"));
        assert!(jql_both.contains(" OR "));
    }

    #[test]
    fn test_filter_split_into_chunks() {
        // TimeBasedFilter::split_into_chunks()が正しく時間を分割することをテスト
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 1, 6, 0, 0).unwrap();

        let filter = TimeBasedFilter::new()
            .since(start)
            .until(end)
            .granularity_hours(2);

        let chunks = filter.split_into_chunks();

        assert_eq!(chunks.len(), 3); // 6時間 / 2時間粒度 = 3チャンク
        assert_eq!(chunks[0].start, start);
        assert_eq!(chunks[0].end, start + Duration::hours(2));
        assert_eq!(chunks[1].start, start + Duration::hours(2));
        assert_eq!(chunks[1].end, start + Duration::hours(4));
        assert_eq!(chunks[2].start, start + Duration::hours(4));
        assert_eq!(chunks[2].end, end);
    }

    #[test]
    fn test_format_and_parse_jira_datetime() {
        // JIRA日時フォーマットが正しく動作することをテスト
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap();
        let formatted = format_jira_datetime(&dt);

        assert_eq!(formatted, "2024-01-15 14:30");

        let parsed = parse_jira_datetime(&formatted).unwrap();

        // 秒の精度は切り捨てられるので、分まで一致すればOK
        assert_eq!(parsed.year(), dt.year());
        assert_eq!(parsed.month(), dt.month());
        assert_eq!(parsed.day(), dt.day());
        assert_eq!(parsed.hour(), dt.hour());
        assert_eq!(parsed.minute(), dt.minute());
    }
}
