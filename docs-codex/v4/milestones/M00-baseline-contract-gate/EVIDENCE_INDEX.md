# M00 Development Evidence Index

> This index contains Development Complete evidence only. It is not the later `ACCEPTANCE.md` and does not declare `ACCEPTED / FROZEN`.

| Evidence ID | Task / AC | Artifact | Development status |
| --- | --- | --- | --- |
| E-T1-BASELINE | T1 / M00-AC01/02 | [AS_IS_BASELINE.md](./AS_IS_BASELINE.md) | Complete; local source/tests recorded; external baseline not verified. |
| E-T2-FIXTURES | T2 / M00-AC03/04/05 | `fixtures/evidence/*.json`, `runtime/src/audit/m00_contract.rs` | Complete; `cargo test -p chatgpt-harness-runtime m00_contract` = 5 passed. |
| E-T3-SQLITE | T3 / M00-AC06/07 | [SQLITE_POC_REPORT.md](./SQLITE_POC_REPORT.md), `poc/v4-m00-evidence-store/` | Complete; isolated POC = 7 passed. |
| E-T4-CONFIG | T4 / M00-AC08/09 | `fixtures/config/*.json`, `ui/src-tauri/src/config/m00_contract.rs` | Complete; config contract = 6 passed; production schema remains 2. |
| E-T5-EVAL | T5 / M00-AC10 support | [EVAL_FEASIBILITY_REPORT.md](./EVAL_FEASIBILITY_REPORT.md), `evaluation/m00-poc/` | Local mechanics complete; real model smoke not executed. |
| E-T6-ADR | T6 / M00-AC12/13/14 | [ADR_DECISIONS.md](./ADR_DECISIONS.md) | Development handoff complete; A10 acceptance remains BLOCKED. |

## Frozen validation documents

- [SPEC.md](./SPEC.md)
- [PLAN.md](./PLAN.md)
- [TASKS.md](./TASKS.md)
- [INTEGRATION_TEST_PLAN.md](./INTEGRATION_TEST_PLAN.md) — `FROZEN / NOT EXECUTED`
- [E2E_TEST_PLAN.md](./E2E_TEST_PLAN.md) — `FROZEN / NOT EXECUTED`
- [MANUAL_ACCEPTANCE_CHECKLIST.md](./MANUAL_ACCEPTANCE_CHECKLIST.md) — `FROZEN / NOT EXECUTED`

## Deliberately absent acceptance evidence

There is no `ACCEPTANCE.md` verdict in this development round. M00 Integration, real external E2E and Human Manual Acceptance must execute against a fixed candidate before any `ACCEPTED / FROZEN` statement.
