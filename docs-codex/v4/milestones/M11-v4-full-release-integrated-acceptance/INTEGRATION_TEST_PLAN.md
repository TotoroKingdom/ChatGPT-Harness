# M11 Integration Test Plan — V4 Full Release Integrated Acceptance

> Status: PLAN ONLY / NOT EXECUTED
> Scope: automated development and formal Integration cases for the one locked M11 Full Release Candidate.
> This file defines checks; it is not execution evidence and does not authorize V4 acceptance.

## 1. Execution Rules

- Every case binds to one `candidate_id`, source SHA, source-state hash, package hashes and identity manifest.
- A product-impacting change creates a new candidate and invalidates affected evidence.
- Historical evidence may be reused only after the M11 identity-equivalence command returns `EXACTLY_EQUIVALENT`.
- `FAIL`, `BLOCKED`, `INCONCLUSIVE` and `NOT_VERIFIED` remain non-passing outcomes.
- Real External ChatGPT, OS-visible Manual observations and paid/network Model execution are outside this automated plan.
- Test state uses disposable temporary directories. No user AppData, Workspace, credentials or active instance may be used.

## 2. Case Matrix

| ID | Coverage | Automated procedure | Required evidence / gate |
| --- | --- | --- | --- |
| M11-INT-01 | Upstream acceptance | Validate M06–M10 state artifact and exact accepted identities. | `ACCEPTED_FROZEN` for all five; otherwise BLOCKED. |
| M11-INT-02 | Candidate identity | Validate deterministic M11 candidate serialization, source cleanliness, package/config/protocol/evidence identities. | Candidate identity PASS; no missing required field. |
| M11-INT-03 | Matrix and AC completeness | Validate exactly F01–F15, Shared Evidence and 43 mandatory canonical ACs plus optional deferred ACs. | No duplicate/missing/misclassified row. |
| M11-INT-04 | Portable/NSIS pair | Build or inspect both packages from the same source candidate and compare UI/Runtime/Tunnel payload hashes. | Package identity PASS; no stale payload. |
| M11-INT-05 | Shipping contamination | Run M06 package audit plus M11 checks for secrets, test fixtures, conformance checkout, evaluation credentials and debug-only tools. | Zero contamination findings. |
| M11-INT-06 | Config upgrade and rollback | Run existing v2→v3, existing-v3, invalid-config and rollback-preservation tests in disposable state. | Fields and SecretReference preserved; no silent reset. |
| M11-INT-07 | Evidence DB migration | Run M03/M08 schema v1→v2 migration, checksum, backup and recovery tests. | History preserved; migration identity exact; no live authorization restored. |
| M11-INT-08 | Runtime MCP lifecycle | Run Runtime HTTP initialize/session/origin/header/malformed/unknown-tool/invalid-argument tests. | Bounded sanitized protocol behavior; loopback-only binding. |
| M11-INT-09 | Core tool authority | Run read/grep, Workspace boundary, Policy DENY, ASK allow/deny, disabled-tool and cancellation tests. | Workspace/Policy/Approval/Execution remain authoritative. |
| M11-INT-10 | V3 security/lifecycle | Run existing Bash, SecretStore, write/edit, Workspace and orphan/crash regression suites. | No bypass, secret leak or owned orphan. |
| M11-INT-11 | F02 Auto Start | Run isolated Auto Start development scenarios and host lifecycle tests. | Default OFF, explicit choice, exactly-once start and no same-process restart. |
| M11-INT-12 | Shared Evidence | Run canonical `runtime_id + request_id` Activity/History/evidence identity tests. | No timestamp-nearest join or second truth. |
| M11-INT-13 | F08 Metrics | Run M08 success, DENY, pre-execution failure and incomplete metrics tests. | Denominators, durations, P95 and UNKNOWN semantics remain server-derived. |
| M11-INT-14 | F14 Trace | Run M08 trace stage, incomplete, capacity and restart tests. | Exact request-local stages; no hidden reasoning or fabricated span. |
| M11-INT-15 | M09 decision set | Validate six candidate verdicts, schema hashes and final catalog projection against the immutable M09 decision set. | Every ADMIT present; every REJECT/DEFER absent; no verdict changes. |
| M11-INT-16 | F12 equivalence | Compare final catalog/profile/dataset/scorer/model identities with accepted M04/M05/M09 evidence. | `EXACTLY_EQUIVALENT` or exact frozen rerun requirement; no threshold drift. |
| M11-INT-17 | F13 equivalence | Compare Runtime MCP, rmcp, revision, transport, schema and official runner identities with M10. | `EXACTLY_EQUIVALENT` or official rerun required; no protocol upgrade. |
| M11-INT-18 | Harness MCP security | Run protocol-level invalid args/unknown tool/cancellation cases separately from official conformance. | Harness security result is independently classified. |
| M11-INT-19 | Release truth | Generate candidate release notes and verify F09/F15 DEFERRED, formal statuses and no early acceptance wording. | Truth check PASS; no `V4 — ACCEPTED / FROZEN` before final lifecycle. |
| M11-INT-20 | Fail-closed verdict | Run the M11 gate with synthetic missing, drifted and blocked inputs. | Gate rejects every mandatory invalid state. |

## 3. Recommended Development Commands

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/m11-release-gate.ps1 self-test
powershell -NoProfile -ExecutionPolicy Bypass -File ui/scripts/m11-development.ps1 -Preflight
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
Push-Location ui
npm run typecheck
npm test
npm run build
Pop-Location
```

The development driver must label its output as development verification. It must not convert these results into formal M11 Integration, E2E or Manual acceptance.

## 4. Evidence Boundary

Raw logs, candidate manifests and disposable test output belong under the existing `target-exe/` or candidate artifact location. Do not add result files to this milestone directory. Formal Integration results are recorded only after the final candidate is locked and independently executed.
