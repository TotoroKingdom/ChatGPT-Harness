//! Run explicitly with `cargo test -p chatgpt-harness-evidence-store --release
//! --test m08_performance -- --ignored --nocapture` on Windows.
use chatgpt_harness_evidence_store::{EvidenceReader, EvidenceWriter, MetricsQueryParams};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::time::Instant;

#[test]
#[ignore = "controlled 50k-request development benchmark"]
fn metrics_50k_requests_twenty_warm_queries() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("metrics.sqlite3");
    drop(EvidenceWriter::open(&database).unwrap());
    let mut connection = Connection::open(&database).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON; UPDATE schema_migrations SET applied_at_utc='2026-10-01T00:00:00.000Z'; INSERT INTO runtime_sessions(runtime_id,started_at_utc,runtime_version,config_revision) VALUES('bench','2026-10-01T00:00:00.000Z','test','test');").unwrap();
    let tx = connection.transaction().unwrap();
    {
        let mut summary = tx.prepare("INSERT INTO request_summary(runtime_id,request_id,tool_name,first_seen_at_utc,last_seen_at_utc,terminal_state,outcome,approval_state,execution_started,duration_ms,evidence_coverage,trace_id) VALUES('bench',?1,?2,?3,?3,'succeeded','success','allow_once',1,100,'COMPLETE',?1)").unwrap();
        let mut event = tx.prepare("INSERT INTO evidence_events(event_id,runtime_id,request_id,tool_name,occurred_at_utc,recorded_at_utc,stage,lifecycle_state,workspace_validation,outcome,coverage) VALUES(?1,'bench',?2,?3,?4,?4,?5,'completed','passed','success','COMPLETE')").unwrap();
        let mut span = tx.prepare("INSERT INTO request_spans(span_id,trace_id,parent_span_id,runtime_id,request_id,span_sequence,span_name,started_at_utc,ended_at_utc,started_offset_ms,duration_ms,status,coverage) VALUES(?1,?2,NULL,'bench',?2,?3,?4,?5,?5,0,?6,'complete','COMPLETE')").unwrap();
        for i in 0..50_000 {
            let id = format!("r{i:05}");
            let tool = if (i / 30) % 2 == 0 {
                "read_file"
            } else {
                "write"
            };
            let timestamp = format!("2026-10-{:02}T12:00:00.000Z", i % 30 + 1);
            summary.execute(params![id, tool, timestamp]).unwrap();
            for (n, stage) in [
                "request_received",
                "workspace_validated",
                "policy_evaluated",
                "execution_started",
                "execution_completed",
            ]
            .into_iter()
            .enumerate()
            {
                event
                    .execute(params![format!("{id}-e{n}"), id, tool, timestamp, stage])
                    .unwrap();
            }
            for (n, name) in ["request_wall", "approval_wait", "execution", "terminal"]
                .into_iter()
                .enumerate()
            {
                span.execute(params![
                    format!("{id}-s{n}"),
                    id,
                    n as i64,
                    name,
                    timestamp,
                    if name == "execution" {
                        i % 200 + 1
                    } else {
                        100
                    }
                ])
                .unwrap();
            }
        }
    }
    tx.commit().unwrap();
    for (table, expected) in [
        ("request_summary", 50_000),
        ("evidence_events", 250_000),
        ("request_spans", 200_000),
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, expected);
    }
    let mut plan_statement=connection.prepare("EXPLAIN QUERY PLAN SELECT q.runtime_id,q.request_id,MAX(s.duration_ms),EXISTS(SELECT 1 FROM evidence_events e WHERE e.runtime_id=q.runtime_id AND e.request_id=q.request_id AND e.stage='execution_started') FROM request_summary q LEFT JOIN request_spans s ON s.runtime_id=q.runtime_id AND s.request_id=q.request_id WHERE q.tool_name='read_file' AND q.first_seen_at_utc >= '2026-10-01' AND q.first_seen_at_utc < '2026-10-31' GROUP BY q.runtime_id,q.request_id").unwrap();
    let plan = plan_statement
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(plan
        .iter()
        .any(|p| p.contains("idx_request_summary_tool_first_seen")));
    println!("query_plan={plan:?}");
    drop(plan_statement);
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let fixture_hash = format!("{:x}", Sha256::digest(std::fs::read(&database).unwrap()));
    println!("fixture_sha256={fixture_hash}");
    drop(connection);
    let reader = EvidenceReader::open(&database).unwrap();
    assert!(reader.connection_is_query_only().unwrap());
    let variants: Vec<_> = (0..6)
        .map(|i| MetricsQueryParams {
            from_utc: format!("2026-10-{:02}T00:00:00.000Z", [30, 24, 1][i % 3]),
            to_utc: "2026-10-31T00:00:00.000Z".into(),
            tool: if i < 3 {
                None
            } else {
                Some("read_file".into())
            },
        })
        .collect();
    for q in &variants {
        reader.metrics_query(q).unwrap();
    }
    let mut timings = Vec::new();
    for i in 0..20 {
        let start = Instant::now();
        reader.metrics_query(&variants[i % variants.len()]).unwrap();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "query={i} variant={} elapsed_ms={ms:.3}",
            i % variants.len()
        );
        timings.push(ms);
    }
    timings.sort_by(f64::total_cmp);
    println!("fixture=deterministic-v1 summaries=50000 events=250000 spans=200000 p95_ms={:.3} timeouts=0 os={} arch={}",timings[18],std::env::consts::OS,std::env::consts::ARCH);
    assert!(
        timings[18] <= 750.0,
        "query P95 exceeds frozen 750ms budget"
    );
}
