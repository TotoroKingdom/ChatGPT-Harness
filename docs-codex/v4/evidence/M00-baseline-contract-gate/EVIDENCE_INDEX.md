# M00 Development Evidence Index

> This index contains Development Complete evidence only. It is not the later `ACCEPTANCE.md` and does not declare `ACCEPTED / FROZEN`.

| Evidence ID | Task / AC | Artifact | Development status |
| --- | --- | --- | --- |
| E-T1-BASELINE | T1 / M00-AC01/02 | [AS_IS_BASELINE.md](./AS_IS_BASELINE.md) | Complete; local source/tests recorded; external baseline not verified. |
| E-T2-FIXTURES | T2 / M00-AC03/04/05 | `fixtures/v4/m00/evidence/*.json`, `runtime/src/audit/m00_contract.rs` | Complete; `cargo test -p chatgpt-harness-runtime m00_contract` = 5 passed. |
| E-T3-SQLITE | T3 / M00-AC06/07 | [SQLITE_POC_REPORT.md](./SQLITE_POC_REPORT.md), `runtime/tests/m00/evidence-store-poc/` | Complete; isolated POC = 7 passed. |
| E-T4-CONFIG | T4 / M00-AC08/09 | `fixtures/v4/m00/config/*.json`, `ui/src-tauri/src/config/m00_contract.rs` | Complete; config contract = 6 passed; production schema remains 2. |
| E-T5-EVAL | T5 / M00-AC10 support | [EVAL_FEASIBILITY_REPORT.md](./EVAL_FEASIBILITY_REPORT.md), `runtime/tests/m00/agent-evaluation-poc/` | Local mechanics complete; real model smoke not executed. |
| E-T6-ADR | T6 / M00-AC12/13/14 | [ADR_DECISIONS.md](./ADR_DECISIONS.md) | Development handoff complete; A10 acceptance remains BLOCKED. |

## Frozen validation documents

- [SPEC.md](../../milestones/M00-baseline-contract-gate/SPEC.md)
- [PLAN.md](../../milestones/M00-baseline-contract-gate/PLAN.md)
- [TASKS.md](../../milestones/M00-baseline-contract-gate/TASKS.md)
- [INTEGRATION_TEST_PLAN.md](../../milestones/M00-baseline-contract-gate/INTEGRATION_TEST_PLAN.md) — `FROZEN / NOT EXECUTED`
- [E2E_TEST_PLAN.md](../../milestones/M00-baseline-contract-gate/E2E_TEST_PLAN.md) — frozen criteria; execution evidence recorded; E2E-01/E2E-02 `BLOCKED`
- [MANUAL_ACCEPTANCE_CHECKLIST.md](../../milestones/M00-baseline-contract-gate/MANUAL_ACCEPTANCE_CHECKLIST.md) — `FROZEN / NOT EXECUTED`

## Independent E2E execution evidence

| Evidence ID | Case / AC | Artifact | Verdict |
| --- | --- | --- | --- |
| E-E2E-01-20261024 | M00-E2E-01 / M00-AC11 | [M00_E2E_EXECUTION_REPORT.md](./M00_E2E_EXECUTION_REPORT.md) | `BLOCKED`; current candidate Harness could not reach an observable Ready state, so no External ChatGPT call was claimed. |
| E-E2E-02-20261024 | M00-E2E-02 / M00-AC10 | [M00_E2E_EXECUTION_REPORT.md](./M00_E2E_EXECUTION_REPORT.md) | `BLOCKED`; no real controllable Agent/Model execution was available. |
| E-E2E-03-PRIOR | M00-E2E-03 / M00-AC12/13/14 | prior independent QA record | `PASS`; not re-executed in this run. |

The current M00 E2E verdict is `BLOCKED`; the Manual Acceptance entry condition is not satisfied. This index does not declare `ACCEPTED / FROZEN`.

## Deliberately absent acceptance evidence

There is no `ACCEPTANCE.md` verdict in this development round. M00 Integration passed previously, while the current external E2E execution recorded E2E-01/E2E-02 as `BLOCKED`; Human Manual Acceptance remains unexecuted. No `ACCEPTED / FROZEN` statement is made.

## Portable delivery and layout handoff — 2026-10-24

- Candidate package: `artifacts/V4-M00-baseline-contract-gate/20261024-023208-fd61d9f1/ChatGPT-Harness_0.1.3_x64-portable/`.
- [Delivery manifest](../../../../artifacts/V4-M00-baseline-contract-gate/20261024-023208-fd61d9f1/delivery.json) records the build source snapshot, dirty state, file hashes and package-verification result. It is a previously built candidate; later documentation and test-directory relocations are not represented as a fresh build.
- [Package verification](../../../../artifacts/V4-M00-baseline-contract-gate/20261024-023208-fd61d9f1/portable-verification.txt): version 0.1.3, six icon entries and local Runtime smoke passed; 10 delivered file hashes were checked. This is not External ChatGPT acceptance.
- Layout handoff checks: Runtime contract 5/5, Tauri config contract 6/6; relocated SQLite POC 7/7 and evaluation POC 7/7; 14 shared fixture files preserved byte-for-byte. Evaluation was rebuilt after relocation because the copied cache contained the previous compile-time path.
- M00 plans contain only the six required files. Reports remain here; shared fixtures are in `fixtures/v4/m00/`; prototype source is in `runtime/tests/m00/`.
- External E2E remains `BLOCKED`; Manual Acceptance remains unexecuted. Historical execution evidence retains its original candidate identity. M01 is not unlocked.
