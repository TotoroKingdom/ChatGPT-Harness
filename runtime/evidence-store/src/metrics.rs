//! Bounded, read-only projections of retained canonical request evidence.
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use std::time::{Duration, Instant};

use crate::{
    DurationMetric, EvidenceReader, MetricsCounts, MetricsCoverage, MetricsQueryParams,
    MetricsQueryResult, MetricsSampleStatus,
};
use crate::{Result, StoreError};

pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_METRICS_RANGE_DAYS: i64 = 30;
const MAX_METRICS_TOOL_FILTER_BYTES: usize = 128;
const MIN_P95_SAMPLES: usize = 20;

/// SQLite also checks the deadline while executing a long step, not only
/// between returned rows. Dropping this guard removes the callback.
pub(crate) struct QueryDeadline<'a>(&'a Connection);
impl<'a> QueryDeadline<'a> {
    pub(crate) fn new(connection: &'a Connection) -> Self {
        let start = Instant::now();
        connection.progress_handler(1000, Some(move || start.elapsed() >= QUERY_TIMEOUT));
        Self(connection)
    }
}
impl Drop for QueryDeadline<'_> {
    fn drop(&mut self) {
        self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

fn normalize_range(from: &str, to: &str) -> Result<(String, String)> {
    let parse = |value: &str| {
        if value.len() > 64 {
            return Err(StoreError::InvalidEvidence(
                "invalid metrics UTC range".into(),
            ));
        }
        DateTime::parse_from_rfc3339(value)
            .map(|v| v.with_timezone(&Utc))
            .map_err(|_| StoreError::InvalidEvidence("invalid metrics UTC range".into()))
    };
    let (from, to) = (parse(from)?, parse(to)?);
    if from >= to || to - from > chrono::Duration::days(MAX_METRICS_RANGE_DAYS) {
        return Err(StoreError::InvalidEvidence(
            "metrics range must be within 30 days".into(),
        ));
    }
    // Canonical evidence UTC values have millisecond precision. Ceil both
    // bounds to that grid so submillisecond input preserves [from, to)
    // membership instead of silently widening/truncating the requested range.
    let bound = |value: DateTime<Utc>| -> Result<String> {
        let remainder = value.timestamp_subsec_nanos() % 1_000_000;
        let value = if remainder == 0 {
            value
        } else {
            value
                .checked_add_signed(chrono::Duration::nanoseconds(i64::from(
                    1_000_000 - remainder,
                )))
                .ok_or_else(|| StoreError::InvalidEvidence("invalid metrics UTC range".into()))?
        };
        Ok(value.to_rfc3339_opts(SecondsFormat::Millis, true))
    };
    Ok((bound(from)?, bound(to)?))
}

fn sample_status(n: u64) -> MetricsSampleStatus {
    match n {
        0 => MetricsSampleStatus::NoData,
        1..=19 => MetricsSampleStatus::Insufficient,
        _ => MetricsSampleStatus::Sufficient,
    }
}

fn duration_metric(mut samples: Vec<u64>) -> DurationMetric {
    let n = samples.len();
    let average_ms = (n > 0).then(|| samples.iter().map(|v| *v as f64).sum::<f64>() / n as f64);
    let p95_ms = if n >= MIN_P95_SAMPLES {
        samples.sort_unstable();
        Some(samples[(95 * n).div_ceil(100) - 1])
    } else {
        None
    };
    DurationMetric {
        sample_count: n as u64,
        average_ms,
        p95_ms,
        sample_status: sample_status(n as u64),
    }
}

impl EvidenceReader {
    pub fn metrics_query(&self, params: &MetricsQueryParams) -> Result<MetricsQueryResult> {
        let (from, to) = normalize_range(&params.from_utc, &params.to_utc)?;
        if params
            .tool
            .as_ref()
            .is_some_and(|v| v.len() > MAX_METRICS_TOOL_FILTER_BYTES || v.is_empty())
        {
            return Err(StoreError::InvalidEvidence(
                "invalid metrics Tool filter".into(),
            ));
        }
        let _snapshot = self
            .connection
            .as_ref()
            .map(Connection::unchecked_transaction)
            .transpose()?;
        let status = self.status()?;
        let mut result = MetricsQueryResult {
            counts: MetricsCounts::default(),
            execution: DurationMetric::default(),
            approval_wait: DurationMetric::default(),
            request_wall: DurationMetric::default(),
            coverage: MetricsCoverage {
                complete_request_count: 0,
                incomplete_request_count: 0,
                unknown_request_count: 0,
                legacy_untraced_request_count: 0,
                execution_duration_eligible_count: 0,
                approval_wait_duration_eligible_count: 0,
                request_wall_duration_eligible_count: 0,
                last_commit_at_utc: status.last_commit_at_utc.clone(),
                drop_count: status.drop_count,
                coverage_unknown_since_utc: status.coverage_unknown_since_utc.clone(),
                store_status: status,
            },
        };
        let Some(connection) = self.connection.as_ref() else {
            return Ok(result);
        };
        let _deadline = QueryDeadline::new(connection);
        let version: i64 =
            connection.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })?;
        if version < 2 {
            return Err(StoreError::IncompatibleSchema {
                found: version as u32,
            });
        }
        // One bounded relational query. Only complete measured spans contribute;
        // old request_summary.duration_ms deliberately never enters this query.
        let sql = format!("SELECT q.terminal_state,q.outcome,EXISTS(SELECT 1 FROM evidence_events x WHERE x.runtime_id=q.runtime_id AND x.request_id=q.request_id AND x.stage='execution_started'),q.evidence_coverage,q.trace_id,
            EXISTS(SELECT 1 FROM evidence_events e WHERE e.runtime_id=q.runtime_id AND e.request_id=q.request_id AND e.stage='request_received'),
            MAX(CASE WHEN s.span_name='execution' AND s.ended_at_utc IS NOT NULL AND s.coverage='COMPLETE' AND s.status NOT IN ('open','incomplete','unknown') THEN s.duration_ms END),
            MAX(CASE WHEN s.span_name='approval_wait' AND s.ended_at_utc IS NOT NULL AND s.coverage='COMPLETE' AND s.status NOT IN ('open','incomplete','unknown') THEN s.duration_ms END),
            MAX(CASE WHEN s.span_name='request_wall' AND s.ended_at_utc IS NOT NULL AND s.coverage='COMPLETE' AND s.status NOT IN ('open','incomplete','unknown') THEN s.duration_ms END)
            FROM request_summary q LEFT JOIN request_spans s ON s.runtime_id=q.runtime_id AND s.request_id=q.request_id
            WHERE q.first_seen_at_utc>=?1 AND q.first_seen_at_utc<?2 {}
            GROUP BY q.runtime_id,q.request_id",if params.tool.is_some(){"AND q.tool_name=?3"}else{""});
        let mut statement = connection.prepare(&sql)?;
        let mut values = vec![
            rusqlite::types::Value::Text(from),
            rusqlite::types::Value::Text(to),
        ];
        if let Some(tool) = &params.tool {
            values.push(rusqlite::types::Value::Text(tool.clone()));
        }
        let mut rows = statement.query(rusqlite::params_from_iter(values))?;
        let (mut execution, mut approval, mut wall) = (Vec::new(), Vec::new(), Vec::new());
        while let Some(row) = rows.next()? {
            let terminal: String = row.get(0)?;
            let outcome: Option<String> = row.get(1)?;
            let started: bool = row.get(2)?;
            let coverage: String = row.get(3)?;
            let trace: Option<String> = row.get(4)?;
            let received: bool = row.get(5)?;
            let counts = &mut result.counts;
            counts.received_calls += u64::from(received);
            counts.execution_attempts += u64::from(started);
            if trace.is_none() {
                counts.legacy_untraced_calls += 1;
                result.coverage.legacy_untraced_request_count += 1;
            }
            let mut unknown = coverage == "UNKNOWN" || !received || terminal == "unknown";
            let incomplete =
                coverage == "INCOMPLETE" || matches!(terminal.as_str(), "open" | "incomplete");
            if unknown {
                result.coverage.unknown_request_count += 1;
                counts.unknown_calls += 1;
            } else if incomplete {
                result.coverage.incomplete_request_count += 1;
                counts.incomplete_calls += 1;
            } else {
                result.coverage.complete_request_count += 1;
                match (terminal.as_str(), outcome.as_deref(), started) {
                    ("succeeded", Some("success"), true) => counts.successful_calls += 1,
                    ("failed", Some("failed"), true) => counts.failed_calls += 1,
                    ("denied", _, _) => counts.denied_calls += 1,
                    ("cancelled", _, true) => counts.cancelled_calls += 1,
                    ("timed_out", _, true) => counts.timed_out_calls += 1,
                    ("approval_expired", _, _) => counts.approval_expired_calls += 1,
                    ("approval_cancelled", _, _) => counts.approval_cancelled_calls += 1,
                    ("failed", _, false) => counts.pre_execution_failed_calls += 1,
                    _ => {
                        unknown = true;
                        counts.unknown_calls += 1;
                        result.coverage.complete_request_count -= 1;
                        result.coverage.unknown_request_count += 1;
                    }
                }
            }
            if !unknown && !incomplete {
                let ordinary_execution = started
                    && matches!(
                        (terminal.as_str(), outcome.as_deref()),
                        ("succeeded", Some("success")) | ("failed", Some("failed"))
                    );
                if ordinary_execution {
                    if let Some(ms) = row.get::<_, Option<i64>>(6)?.filter(|ms| *ms >= 0) {
                        execution.push(ms as u64);
                    }
                }
                if let Some(ms) = row.get::<_, Option<i64>>(8)?.filter(|ms| *ms >= 0) {
                    wall.push(ms as u64);
                }
            }
            // A completed approval wait remains measurable even if a later
            // executor crashes. It carries its own independently closed evidence.
            if let Some(ms) = row.get::<_, Option<i64>>(7)?.filter(|ms| *ms >= 0) {
                approval.push(ms as u64);
            }
        }
        result.counts.completed_execution_denominator =
            result.counts.successful_calls + result.counts.failed_calls;
        result.counts.execution_success_numerator = result.counts.successful_calls;
        let n = result.counts.completed_execution_denominator;
        result.counts.execution_success_rate =
            (n > 0).then(|| result.counts.successful_calls as f64 / n as f64);
        result.counts.execution_success_sample_status = sample_status(n);
        result.execution = duration_metric(execution);
        result.approval_wait = duration_metric(approval);
        result.request_wall = duration_metric(wall);
        result.coverage.execution_duration_eligible_count = result.execution.sample_count;
        result.coverage.approval_wait_duration_eligible_count = result.approval_wait.sample_count;
        result.coverage.request_wall_duration_eligible_count = result.request_wall.sample_count;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_rank_boundaries_and_unordered_samples() {
        for n in [0, 1, 19, 20, 21] {
            let result = duration_metric((1..=n).rev().collect());
            assert_eq!(result.sample_count, n);
            assert_eq!(
                result.p95_ms,
                match n {
                    20 => Some(19),
                    21 => Some(20),
                    _ => None,
                }
            );
            assert_eq!(result.sample_status, sample_status(n));
        }
    }
    #[test]
    fn bounded_range_and_timezone_normalization() {
        assert!(normalize_range("bad", "bad").is_err());
        assert!(normalize_range("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").is_err());
        assert!(normalize_range("2026-10-01T00:00:00Z", "2026-10-01T00:00:00Z").is_err());
        assert_eq!(
            normalize_range("2026-10-01T08:00:00+08:00", "2026-10-02T00:00:00Z")
                .unwrap()
                .0,
            "2026-10-01T00:00:00.000Z"
        );
        let (from, to) =
            normalize_range("2026-10-01T00:00:00.0001Z", "2026-10-01T00:00:00.0009Z").unwrap();
        assert_eq!(from, "2026-10-01T00:00:00.001Z");
        assert_eq!(from, to);
    }
}
