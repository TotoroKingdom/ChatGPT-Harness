# M1 Manual Acceptance Checklist — Write/Edit Production Hardening

> Status: ACCEPTED / FROZEN
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`, `E2E_TEST_PLAN.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Owner: Human Reviewer  
> Purpose: the only human-intervention layer for M1.

## 1. Acceptance Responsibility

M1 verification ownership is:

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

The human reviewer does **not** manually drive Unit, Integration or automated E2E cases.

Manual Acceptance is responsible for:

1. reviewing automated evidence;
2. checking scope/security claims at milestone level;
3. validating the real user-facing `External ChatGPT → Tunnel → Harness → Runtime` chain where required;
4. making the final `ACCEPTED / FROZEN` decision.

M1 does not revisit M0's product capability question. M0 already proved External ChatGPT can invoke write/edit. M1 manual acceptance only checks that the hardened build did not regress that real user path.

## 2. Entry Criteria

Manual Acceptance may begin only after the Coding Agent reports:

```text
Unit Tests = PASS
Integration Tests = PASS
Local Black-box E2E = PASS
```

and provides build / commit identity plus automated evidence.

Required automated completion:

- [x] T1–T4 development complete.
- [x] `cargo fmt --all -- --check` PASS.
- [x] `cargo check --workspace` PASS.
- [x] `cargo test --workspace` PASS.
- [x] `git diff --check` PASS.
- [x] `INTEGRATION_TEST_PLAN.md` PASS.
- [x] `E2E_TEST_PLAN.md` Local Black-box E2E PASS.
- [x] Windows atomic-replace automated evidence present.
- [x] atomic fault-injection evidence present.
- [x] concurrent edit `TARGET_CHANGED` evidence present.

The reviewer must not compensate for a missing automated layer by manually performing equivalent low-level cases.

## 3. Build / Source Identity

- [x] Record the acceptance commit SHA.
- [x] Confirm the reviewed build contains the M1 T1–T4 production implementation.
- [x] Confirm the build is not an M0 binary.
- [x] Record any later test-only commit separately from the production implementation commit.
- [x] Confirm no unexplained production source changes were introduced after automated verification.

Evidence:

```text
Production implementation commit: 7db206b6232e955df2553d054cb7c2e9e063c76c
Later automated-test coverage commit: 22f57a87f9d5e9dec0d68d61c3825b6703b38a33
Final acceptance commit/build: 455228e130e1c8100e1061606b4c445376d29b3d (tag: `M1-freeze`)
Reviewer: User-directed acceptance evidence review
Date: 2026-09-30
```

The later `22f57a8...` commit adds/strengthens automated test coverage; it does not by itself invalidate already completed External ChatGPT evidence for the unchanged production semantics. If later commits modify production write/edit semantics, the reviewer must reassess whether external evidence needs rerun.

## 4. Automated Evidence Review

### 4.1 Unit / Task Evidence

- [x] T1 Shared Atomic File Mutation Primitive PASS.
- [x] T2 Production Harden `write` PASS.
- [x] T3 Production Harden `edit` PASS.
- [x] T4 Errors / Evidence / M0 Regression PASS.
- [x] No M2 Tool Toggle semantics introduced.
- [x] No M3 configurable Policy semantics introduced.
- [x] No M4 Approval semantics introduced.

### 4.2 Integration Evidence

From `INTEGRATION_TEST_PLAN.md`:

- [x] Integration result = PASS.
- [x] INT-01 through INT-18 executed with evidence.
- [x] atomic write create/overwrite PASS.
- [x] atomic edit PASS.
- [x] TempWrite/TempFlush/Commit failures preserve existing target.
- [x] failed create leaves no named target.
- [x] concurrent edit `TARGET_CHANGED` PASS.
- [x] UTF-8 boundary PASS.
- [x] Workspace boundary PASS.
- [x] M0 write/edit/read/glob regression PASS.
- [x] Activity/Audit confidentiality PASS.

If Windows symlink/junction fixture creation was unavailable:

- [x] `ENVIRONMENT_SKIP` is explicitly justified where a Windows symlink/junction fixture is unavailable.
- [x] other Windows atomic/boundary tests still PASS.

### 4.3 Local Black-box E2E Evidence

From `E2E_TEST_PLAN.md`:

- [x] E2E-01 through E2E-08 executed autonomously by Coding Agent.
- [x] actual compiled Runtime process was spawned.
- [x] Runtime was initialized through real stdin/stdout Control RPC.
- [x] MCP endpoint was obtained through `mcp.endpoint.get`.
- [x] tools were called through the real loopback MCP HTTP endpoint.
- [x] filesystem bytes were asserted directly.
- [x] Activity was asserted through `activity.list`.
- [x] write create/overwrite PASS.
- [x] exact edit/read-back PASS.
- [x] ambiguous edit remained non-mutating.
- [x] outside mutation was rejected.
- [x] UTF-8 production path PASS.
- [x] Runtime shutdown/cleanup PASS.
- [x] no E2E case was replaced by mock or in-process executor testing.

Automated evidence reference:

```text
Integration command/result:
E2E command/result:
Build identity:
```

## 5. Atomic / Security Design Review

The reviewer checks evidence and source-level design, but does not manually reproduce internal failure injection.

- [x] temp file is created in the same approved target directory.
- [x] temp uses exclusive create semantics.
- [x] full bytes are staged/flushed before commit.
- [x] target/parent are revalidated before commit.
- [x] Windows existing-target replacement uses real atomic-replace semantics.
- [x] implementation is not `delete target → rename temp`.
- [x] atomic helper is not an authorization authority.
- [x] WorkspaceGuard remains the path-authorization authority.
- [x] Policy evaluation remains inside ExecutionPipeline.
- [x] MCP adapters do not bypass Pipeline/ExecutionPermit.
- [x] test/fault seams are not exposed as user-controlled MCP/Tauri interfaces.

## 6. Compatibility / Scope Review

- [x] MCP tool names remain `read_file`, `glob`, `write`, `edit`.
- [x] write input remains `path`, `content`.
- [x] edit input remains `path`, `oldText`, `newText`.
- [x] core success fields remain M0-compatible.
- [x] annotations remain M0-compatible.
- [x] `WorkspaceGuard::approve()` existing-only semantics remain intact.
- [x] read_file / glob behavior remains compatible.
- [x] no new protocol major was introduced for M1.
- [x] M1 does not claim power-loss durability.
- [x] M1 does not claim parent-directory durable fsync.
- [x] M1 does not claim multi-file transactions.
- [x] M1 does not claim a filesystem/OS sandbox.
- [x] Tool Toggle / User Policy / Approval / Bash / i18n remain out of scope.

## 7. External ChatGPT / Tunnel / Harness Manual Acceptance

This section contains the **real user-chain cases** previously located in `E2E_TEST_PLAN.md`.

They are Manual Acceptance because they require a real External ChatGPT session, Tunnel connectivity and Harness instance/user observation.

Target chain:

```text
External ChatGPT
→ Tunnel
→ Harness
→ Runtime MCP
→ ExecutionPipeline
→ Workspace filesystem
→ Activity/Audit
```

### 7.1 Manual Cases

#### MANUAL-EXT-01 — External write create

External ChatGPT invokes `write` to create:

```text
m1-e2e.txt = hello from M1
```

Verify actual tool call, exact filesystem bytes and Activity evidence.

#### MANUAL-EXT-02 — External full overwrite

External ChatGPT invokes `write` to make the file exactly:

```text
hello from M1 overwrite
```

Verify no old-content residue and successful Activity evidence.

#### MANUAL-EXT-03 — External exact edit

External ChatGPT invokes `edit`:

```text
hello from M1 overwrite
→ hello from M1 hardened
```

Verify exact final bytes and Activity evidence.

#### MANUAL-EXT-04 — Ambiguous edit safety

Prepare/use:

```text
m1-duplicate.txt = same same
```

Ask for `same → changed` without selecting a specific occurrence.

Verify the Harness edit fails as non-unique and the file remains unchanged. A later user/ChatGPT decision to perform a different explicit operation is outside this case.

#### MANUAL-EXT-05 — Workspace boundary

External ChatGPT attempts a mutation into a dedicated adjacent outside test directory.

Verify:

- a real mutation tool call reaches Runtime;
- Runtime rejects it;
- outside target is unchanged/not created;
- Activity shows rejection, not execution success.

#### MANUAL-EXT-06 — External read-back

External ChatGPT uses `read_file` on final `m1-e2e.txt` and observes:

```text
hello from M1 hardened
```

### 7.2 Carried-forward M1 External Evidence

The above External ChatGPT chain has already been executed for the M1 production implementation during this milestone's acceptance work.

Carry forward the completed evidence rather than requiring an identical rerun solely because verification documents were reorganized.

Recorded basis:

```text
Production implementation commit:
7db206b6232e955df2553d054cb7c2e9e063c76c

External acceptance workspace used during completed evidence:
C:\AI\M1-E2E-Workspace-20260930

Dedicated adjacent outside test directory:
C:\AI\M1-E2E-Outside-20260930

Observed final hardened file content:
hello from M1 hardened
```

Carried-forward case status:

```text
MANUAL-EXT-01 write create                    = PASS
MANUAL-EXT-02 write overwrite                 = PASS
MANUAL-EXT-03 exact edit                      = PASS
MANUAL-EXT-04 ambiguous edit non-mutation     = PASS
MANUAL-EXT-05 outside-workspace rejection     = PASS
MANUAL-EXT-06 read-back                       = PASS
```

Rules for reuse:

- [x] Evidence corresponds to the M1 production semantics under review.
- [x] No later production code change invalidates the exercised behavior.
- [x] Test-only commit `22f57a8...` alone does not force an External ChatGPT rerun.
- [x] If production write/edit/MCP/Workspace/Activity behavior changes after that evidence, affected Manual cases must be rerun.
- [x] Historical evidence is not rewritten to claim coverage it did not perform.

Manual External evidence reference / notes:

```text
Existing completed M1 External ChatGPT acceptance evidence retained.
No repeat execution required solely for this documentation-boundary refactor.
```

## 8. Error / Audit Review

- [x] `TARGET_CHANGED` is stable and sanitized where externally observable.
- [x] staging/flush/commit OS detail is not leaked to MCP callers.
- [x] temp absolute path/name is not leaked to Activity/Audit.
- [x] raw `content`, `oldText`, `newText` are not in Activity/Audit.
- [x] unnecessary outside absolute paths are not exposed.
- [x] success / target-changed / I/O failure / Workspace rejection can be distinguished from evidence.

## 9. Acceptance Closure

All required automated and manual acceptance layers have completed with PASS evidence. No unresolved M1 implementation, atomicity / failure-safety, integration, automated E2E, security boundary, M0 regression, external user-chain, local test environment, or manual acceptance item remains.

Unit/Integration/E2E closure is based on Coding Agent evidence, and the External ChatGPT cases are closed by the carried-forward manual evidence recorded above.

## 10. Final Decision

Only the Human Reviewer may choose the M1 milestone decision:

```text
[x] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — ATOMICITY / FAILURE-SAFETY FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — AUTOMATED E2E FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — M0 REGRESSION
[ ] REJECTED — EXTERNAL USER-CHAIN REGRESSION
```

Reviewer conclusion:

```text
The M1 automated evidence chain is complete: T1–T4, workspace checks, Integration INT-01–INT-18,
Windows atomic replacement/fault-safety, and Local Black-box E2E E2E-01–E2E-08 are PASS. The
carried-forward External ChatGPT evidence for MANUAL-EXT-01–06 matches the reviewed M1 production
semantics. M0 compatibility and security boundaries are preserved. No unresolved M1 acceptance item
remains.

M1 — ACCEPTED / FROZEN
```

## 11. Freeze Condition

M1 is marked:

```text
M1 — ACCEPTED / FROZEN
```

The Human Reviewer confirmed:

- [x] Unit evidence PASS from Coding Agent.
- [x] Integration evidence PASS from Coding Agent.
- [x] Local Black-box E2E evidence PASS from Coding Agent.
- [x] Windows atomic-replace/fault-safety evidence complete.
- [x] M0 frozen contract has no regression.
- [x] carried-forward or newly executed External ChatGPT manual evidence is valid for the production semantics under review.
- [x] no unresolved M1 blocking defect.
- [x] no unresolved security boundary regression.

Then development may proceed to:

```text
M2 — Tool Enable / Disable
```
