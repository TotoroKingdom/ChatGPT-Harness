# M00 SQLite Ownership POC Report

> Scope: T3 local development POC only.
> This is not the M03 production SQLite History implementation.

## 1. Candidate and isolation

- Repository commit at T3 implementation start: `7f2a482b1babfdb08b5e80cabfa430d3d0227dac`.
- POC path: `poc/v4-m00-evidence-store/`.
- The POC manifest declares its own empty Cargo `[workspace]`; it is not a member of the root production workspace.
- Every test creates a `tempfile::TempDir`; the POC never uses AppData, the user Workspace, or a production Runtime launch path.
- Rust/Cargo: `1.98.0` / `1.98.0`.

## 2. Implementation boundary

The POC models three explicit ownership markers:

1. `WriterStore`: one Runtime-like owner marker, one SQLite writer connection, WAL and foreign keys enabled, versioned migration.
2. `ReadOnlyReader`: SQLite read-only connection; it has no write/migration/clear API and includes a verification helper that confirms SQLite rejects writes.
3. `OfflineMaintenance`: only acquires after the writer marker disappears; it performs bounded clear/checkpoint maintenance and releases its own marker.

The POC stores a minimal `schema_migrations`, `evidence_events`, `request_summary` and `evidence_health` shape. The schema is disposable and must not be copied as M03 production DDL without a new SPEC.

Migration checksums are explicit (`m00-schema-v1`, `m00-schema-v2`). A v1 database is upgraded to v2 by adding the bounded `coverage` field and health record. Injected migration failure happens before commit and leaves the source database at v1.

## 3. Verification command and result

```text
cargo test --manifest-path poc/v4-m00-evidence-store/Cargo.toml
```

Result: **PASS** — 7 unit tests passed, 0 failed; doc-tests: 0 tests passed.

Resolved dependency evidence from the build:

- `rusqlite 0.40.2`, feature `bundled`;
- `libsqlite3-sys 0.38.2`;
- `serde_json 1.0.154`, `uuid 1.29.0`, `tempfile 3.27.0`.

## 4. Test matrix

| POC behavior | Evidence |
| --- | --- |
| WAL + writer/read-only reader | `writer_reader_wal_and_explicit_single_writer_ownership`; reader observes committed event and `journal_mode=wal`. |
| Deterministic single writer | Explicit `.m00-writer` marker rejects a second writer with `WriterAlreadyOwned`; no incidental `SQLITE_BUSY` timing is used as the ownership protocol. |
| Read-only boundary | `ReadOnlyReader::verify_read_only` confirms a write is rejected. |
| Atomic event + request summary | `transaction_rolls_back_event_and_summary_together`; injected pre-commit failure leaves both absent. |
| Event idempotency | `event_id_is_idempotent_and_request_identity_beats_close_timestamps`; duplicate event is not counted twice. |
| Stable request correlation | Same/near-identical observed timestamps are still queried by `(runtime_id, request_id)` and remain separate. |
| Crash/incomplete semantics | `incomplete_request_survives_writer_restart_without_invented_terminal_result`; only `INCOMPLETE` remains after writer drop. |
| Migration and checksum failure | `migration_failure_preserves_source_database_and_checksum_is_checked`; source file remains present and diagnosable. |
| Stopped-runtime maintenance | `offline_maintenance_requires_stopped_writer_and_reopens_coherently`; writer-active maintenance is rejected, stopped maintenance clears data, v2 reopens. |
| Observational failure / authorization separation | `persistence_failure_is_observational_and_history_has_no_authorization_constructor`; failed evidence persistence does not change an independent `DENY`, and historical DTO has no authorization conversion. |

## 5. Limitations and M03 handoff

- The marker is an explicit same-host POC ownership contract; M03 must choose and test the final cross-process maintenance lock semantics.
- The POC does not implement production History queries, retention, bounded writer queues, Runtime integration, UI facade, or recovery from real process termination.
- SQLite bundled build proves the storage topology, not Windows packaging or encrypted-at-rest behavior.
- `HistoricalApproval` remains observation-only data. Nothing in the POC can recreate Pending Approval, Session Grant or ExecutionPermit.
- A passing POC does not prove M00 Integration/E2E/Manual acceptance and does not unlock M03 by itself.
