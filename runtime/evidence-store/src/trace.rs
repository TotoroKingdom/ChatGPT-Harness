//! Exact-identity read-only request trace. No reconstruction from timestamps.
use crate::metrics::QueryDeadline;
use crate::writer::{enum_from_db, optional_enum_from_db};
use crate::*;
use rusqlite::OptionalExtension;

impl EvidenceReader {
    pub fn trace_get(&self, params: &TraceGetParams) -> Result<TraceGetResult> {
        if [&params.runtime_id, &params.request_id]
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err(StoreError::InvalidEvidence(
                "invalid trace request identity".into(),
            ));
        }
        let _snapshot = self
            .connection
            .as_ref()
            .map(rusqlite::Connection::unchecked_transaction)
            .transpose()?;
        let mut result = TraceGetResult {
            runtime_id: params.runtime_id.clone(),
            request_id: params.request_id.clone(),
            trace_id: None,
            tool_name: String::new(),
            outcome: None,
            policy_decision: None,
            approval_state: None,
            executor_started: false,
            coverage: TraceCoverage::Unknown,
            availability: TraceAvailability::Unavailable,
            spans: Vec::new(),
            failure_stage: None,
            slowest_measured_span: None,
            store_status: self.status()?,
            policy_reason: None,
            approval_decision: None,
        };
        let Some(connection) = self.connection.as_ref() else {
            return Ok(result);
        };
        let _deadline = QueryDeadline::new(connection);
        let version: i64 =
            connection.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })?;
        let sql=format!("SELECT tool_name,outcome,policy_decision,approval_state,EXISTS(SELECT 1 FROM evidence_events x WHERE x.runtime_id=request_summary.runtime_id AND x.request_id=request_summary.request_id AND x.stage='execution_started'),evidence_coverage,terminal_state,{} FROM request_summary WHERE runtime_id=?1 AND request_id=?2",if version>=2 {"trace_id"} else {"NULL"});
        let summary = connection
            .query_row(&sql, [&params.runtime_id, &params.request_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .optional()?
            .ok_or(StoreError::NotFound)?;
        result.tool_name = summary.0;
        result.outcome = optional_enum_from_db(summary.1)?;
        result.policy_decision = optional_enum_from_db(summary.2)?;
        result.approval_state = Some(enum_from_db(summary.3)?);
        result.executor_started = summary.4;
        result.trace_id = summary.7;
        let context=connection.query_row("SELECT (SELECT policy_reason FROM evidence_events WHERE runtime_id=?1 AND request_id=?2 AND policy_reason IS NOT NULL ORDER BY occurred_at_utc DESC,event_id DESC LIMIT 1), (SELECT approval_decision FROM evidence_events WHERE runtime_id=?1 AND request_id=?2 AND approval_decision IS NOT NULL ORDER BY occurred_at_utc DESC,event_id DESC LIMIT 1)",[&params.runtime_id,&params.request_id],|r|Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?))).optional()?;
        if let Some((reason, decision)) = context {
            result.policy_reason = optional_enum_from_db(reason)?;
            result.approval_decision = optional_enum_from_db(decision)?;
        }
        if version < 2 {
            result.coverage = TraceCoverage::LegacyUntraced;
            result.availability = TraceAvailability::MigrationRequired;
            return Ok(result);
        }
        if result.trace_id.is_none() {
            if summary.5 == "UNKNOWN" {
                // Native capture can be lost under bounded capacity. A lost
                // trace is unknown evidence, never a fabricated legacy trace.
                result.coverage = TraceCoverage::Unknown;
                result.availability = TraceAvailability::Unavailable;
                return Ok(result);
            }
            result.coverage = TraceCoverage::LegacyUntraced;
            result.availability = TraceAvailability::LegacyUntraced;
            return Ok(result);
        }
        let mut statement=connection.prepare("SELECT span_id,trace_id,parent_span_id,runtime_id,request_id,span_sequence,span_name,started_at_utc,ended_at_utc,started_offset_ms,duration_ms,status,failure_category,coverage FROM request_spans WHERE runtime_id=?1 AND request_id=?2 ORDER BY span_sequence ASC LIMIT 17")?;
        let mut rows = statement.query([&params.runtime_id, &params.request_id])?;
        let mut overflow = false;
        while let Some(row) = rows.next()? {
            if result.spans.len() == 16 {
                overflow = true;
                break;
            }
            let status: TraceSpanStatus = enum_from_db(row.get(11)?)?;
            let coverage: EvidenceCoverage = enum_from_db(row.get(13)?)?;
            let open = matches!(
                status,
                TraceSpanStatus::Open | TraceSpanStatus::Incomplete | TraceSpanStatus::Unknown
            );
            let duration: Option<i64> = row.get(10)?;
            result.spans.push(RequestTraceSpan {
                span_id: row.get(0)?,
                trace_id: row.get(1)?,
                parent_span_id: row.get(2)?,
                runtime_id: row.get(3)?,
                request_id: row.get(4)?,
                span_sequence: row.get(5)?,
                span_name: enum_from_db(row.get(6)?)?,
                started_at_utc: row.get(7)?,
                ended_at_utc: if open { None } else { row.get(8)? },
                started_offset_ms: row.get(9)?,
                duration_ms: if open {
                    None
                } else {
                    duration.filter(|v| *v >= 0).map(|v| v as u64)
                },
                status,
                failure_category: optional_enum_from_db(row.get(12)?)?,
                coverage,
            });
        }
        result.availability = TraceAvailability::Available;
        result.coverage = if overflow
            || summary.5 == "UNKNOWN"
            || result.spans.is_empty()
            || result
                .spans
                .iter()
                .any(|s| s.coverage == EvidenceCoverage::Unknown)
        {
            TraceCoverage::Unknown
        } else if summary.5 == "INCOMPLETE"
            || matches!(summary.6.as_str(), "open" | "incomplete")
            || result.spans.iter().any(|s| {
                matches!(
                    s.status,
                    TraceSpanStatus::Open | TraceSpanStatus::Incomplete
                )
            })
        {
            TraceCoverage::Incomplete
        } else {
            TraceCoverage::Complete
        };
        result.failure_stage = result
            .spans
            .iter()
            .rev()
            .find(|s| {
                s.span_name != TraceSpanName::RequestWall
                    && s.span_name != TraceSpanName::Terminal
                    && matches!(
                        s.status,
                        TraceSpanStatus::Failed
                            | TraceSpanStatus::Denied
                            | TraceSpanStatus::Cancelled
                            | TraceSpanStatus::TimedOut
                    )
            })
            .map(|s| s.span_name);
        result.slowest_measured_span = result
            .spans
            .iter()
            .filter(|s| s.span_name != TraceSpanName::RequestWall && s.duration_ms.is_some())
            .max_by_key(|s| s.duration_ms)
            .or_else(|| {
                result
                    .spans
                    .iter()
                    .find(|s| s.span_name == TraceSpanName::RequestWall && s.duration_ms.is_some())
            })
            .map(|s| s.span_name);
        Ok(result)
    }
}
