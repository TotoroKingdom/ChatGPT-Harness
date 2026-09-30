# M1 Manual Acceptance Checklist — Write/Edit Production Hardening

> Status: DRAFT / READY FOR FINAL HUMAN REVIEW  
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

M1 does not re-open M0's product capability question. M0 already proved External ChatGPT can invoke write/edit. M1 manual acceptance only checks that the hardened build did not regress that real user path.

## 2. Entry Criteria

Manual Acceptance may begin only after the Coding Agent reports:

```text
Unit Tests = PASS
Integration Tests = PASS
Local Black-box E2E = PASS
```

and provides build / commit identity plus automated evidence.

Required automated completion:

- [ ] T1–T4 development complete.
- [ ] `cargo fmt --all -- --check` PASS.
- [ ] `cargo check --workspace` PASS.
- [ ] `cargo test --workspace` PASS.
- [ ] `git diff --check` PASS.
- [ ] `INTEGRATION_TEST_PLAN.md` PASS.
- [ ] `E2E_TEST_PLAN.md` Local Black-box E2E PASS.
- [ ] Windows atomic-replace automated evidence present.
- [ ] atomic fault-injection evidence present.
- [ ] concurrent edit `TARGET_CHANGED` evidence present.

The reviewer must not compensate for a missing automated layer by manually performing equivalent low-level cases.

## 3. Build / Source Identity

- [ ] Record the acceptance commit SHA.
- [ ] Confirm the reviewed build contains the M1 T1–T4 production implementation.
- [ ] Confirm the build is not an M0 binary.
- [ ] Record any later test-only commit separately from the production implementation commit.
- [ ] Confirm no unexplained production source changes were introduced after automated verification.

Evidence:

```text
Production implementation commit: 7db206b6232e955df2553d054cb7c2e9e063c76c
Later automated-test coverage commit: 22f57a87f9d5e9dec0d68d61c3825b6703b38a33
Final acceptance commit/build:
Reviewer:
Date:
```

The later `22f57a8...` commit adds/strengthens automated test coverage; it does not by itself invalidate already completed External ChatGPT evidence for the unchanged production semantics. If later commits modify production write/edit semantics, the reviewer must reassess whether external evidence needs rerun.

## 4. Automated Evidence Review

### 4.1 Unit / Task Evidence

- [ ] T1 Shared Atomic File Mutation Primitive PASS.
- [ ] T2 Production Harden `write` PASS.
- [ ] T3 Production Harden `edit` PASS.
- [ ] T4 Errors / Evidence / M0 Regression PASS.
- [ ] No M2 Tool Toggle semantics introduced.
- [ ] No M3 configurable Policy semantics introduced.
- [ ] No M4 Approval semantics introduced.

### 4.2 Integration Evidence

From `INTEGRATION_TEST_PLAN.md`:

- [ ] Integration result = PASS.
- [ ] INT-01 through INT-18 executed with evidence.
- [ ] atomic write create/overwrite PASS.
- [ ] atomic edit PASS.
- [ ] TempWrite/TempFlush/Commit failures preserve existing target.
- [ ] failed create leaves no named target.
- [ ] concurrent edit `TARGET_CHANGED` PASS.
- [ ] UTF-8 boundary PASS.
- [ ] Workspace boundary PASS.
- [ ] M0 write/edit/read/glob regression PASS.
- [ ] Activity/Audit confidentiality PASS.

If Windows symlink/junction fixture creation was unavailable:

- [ ] `ENVIRONMENT_SKIP` is explicitly justified.
- [ ] other Windows atomic/boundary tests still PASS.

### 4.3 Local Black-box E2E Evidence

From `E2E_TEST_PLAN.md`:

- [ ] E2E-01 through E2E-08 executed autonomously by Coding Agent.
- [ ] actual compiled Runtime process was spawned.
- [ ] Runtime was initialized through real stdin/stdout Control RPC.
- [ ] MCP endpoint was obtained through `mcp.endpoint.get`.
- [ ] tools were called through the real loopback MCP HTTP endpoint.
- [ ] filesystem bytes were asserted directly.
- [ ] Activity was asserted through `activity.list`.
- [ ] write create/overwrite PASS.
- [ ] exact edit/read-back PASS.
- [ ] ambiguous edit remained non-mutating.
- [ ] outside mutation was rejected.
- [ ] UTF-8 production path PASS.
- [ ] Runtime shutdown/cleanup PASS.
- [ ] no E2E case was replaced by mock or in-process executor testing.

Automated evidence reference:

```text
Integration command/result:
E2E command/result:
Build identity:
```

## 5. Atomic / Security Design Review

The reviewer checks evidence and source-level design, but does not manually reproduce internal failure injection.

- [ ] temp file is created in the same approved target directory.
- [ ] temp uses exclusive create semantics.
- [ ] full bytes are staged/flushed before commit.
- [ ] target/parent are revalidated before commit.
- [ ] Windows existing-target replacement uses real atomic-replace semantics.
- [ ] implementation is not `delete target → rename temp`.
- [ ] atomic helper is not an authorization authority.
- [ ] WorkspaceGuard remains the path-authorization authority.
- [ ] Policy evaluation remains inside ExecutionPipeline.
- [ ] MCP adapters do not bypass Pipeline/ExecutionPermit.
- [ ] test/fault seams are not exposed as user-controlled MCP/Tauri interfaces.

## 6. Compatibility / Scope Review

- [ ] MCP tool names remain `read_file`, `glob`, `write`, `edit`.
- [ ] write input remains `path`, `content`.
- [ ] edit input remains `path`, `oldText`, `newText`.
- [ ] core success fields remain M0-compatible.
- [ ] annotations remain M0-compatible.
- [ ] `WorkspaceGuard::approve()` existing-only semantics remain intact.
- [ ] read_file / glob behavior remains compatible.
- [ ] no new protocol major was introduced for M1.
- [ ] M1 does not claim power-loss durability.
- [ ] M1 does not claim parent-directory durable fsync.
- [ ] M1 does not claim multi-file transactions.
- [ ] M1 does not claim a filesystem/OS sandbox.
- [ ] Tool Toggle / User Policy / Approval / Bash / i18n remain out of scope.

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

- [ ] Evidence corresponds to the M1 production semantics under review.
- [ ] No later production code change invalidates the exercised behavior.
- [ ] Test-only commit `22f57a8...` alone does not force an External ChatGPT rerun.
- [ ] If production write/edit/MCP/Workspace/Activity behavior changes after that evidence, affected Manual cases must be rerun.
- [ ] Historical evidence is not rewritten to claim coverage it did not perform.

Manual External evidence reference / notes:

```text
Existing completed M1 External ChatGPT acceptance evidence retained.
No repeat execution required solely for this documentation-boundary refactor.
```

## 8. Error / Audit Review

- [ ] `TARGET_CHANGED` is stable and sanitized where externally observable.
- [ ] staging/flush/commit OS detail is not leaked to MCP callers.
- [ ] temp absolute path/name is not leaked to Activity/Audit.
- [ ] raw `content`, `oldText`, `newText` are not in Activity/Audit.
- [ ] unnecessary outside absolute paths are not exposed.
- [ ] success / target-changed / I/O failure / Workspace rejection can be distinguished from evidence.

## 9. Failure Classification

If Manual Acceptance fails, choose the actual cause:

```text
REJECTED — IMPLEMENTATION FAILURE
REJECTED — ATOMICITY / FAILURE-SAFETY FAILURE
REJECTED — INTEGRATION FAILURE
REJECTED — AUTOMATED E2E FAILURE
REJECTED — SECURITY BOUNDARY FAILURE
REJECTED — M0 REGRESSION
REJECTED — EXTERNAL USER-CHAIN REGRESSION
BLOCKED — LOCAL TEST ENVIRONMENT
BLOCKED — MANUAL ACCEPTANCE ENVIRONMENT
```

Do not use missing human execution of Unit/Integration/E2E as a reason to mark them PASS; those layers must have Coding Agent evidence first.

## 10. Final Decision

Only the Human Reviewer may choose the M1 milestone decision:

```text
[ ] ACCEPTED / FROZEN
[ ] REJECTED — IMPLEMENTATION FAILURE
[ ] REJECTED — ATOMICITY / FAILURE-SAFETY FAILURE
[ ] REJECTED — INTEGRATION FAILURE
[ ] REJECTED — AUTOMATED E2E FAILURE
[ ] REJECTED — SECURITY BOUNDARY FAILURE
[ ] REJECTED — M0 REGRESSION
[ ] REJECTED — EXTERNAL USER-CHAIN REGRESSION
[ ] BLOCKED — LOCAL TEST ENVIRONMENT
[ ] BLOCKED — MANUAL ACCEPTANCE ENVIRONMENT
```

Reviewer conclusion:

```text

```

## 11. Freeze Condition

M1 may be marked:

```text
M1 — ACCEPTED / FROZEN
```

only after the Human Reviewer confirms:

- [ ] Unit evidence PASS from Coding Agent.
- [ ] Integration evidence PASS from Coding Agent.
- [ ] Local Black-box E2E evidence PASS from Coding Agent.
- [ ] Windows atomic-replace/fault-safety evidence complete.
- [ ] M0 frozen contract has no regression.
- [ ] carried-forward or newly executed External ChatGPT manual evidence is valid for the production semantics under review.
- [ ] no unresolved M1 blocking defect.
- [ ] no unresolved security boundary regression.

Then development may proceed to:

```text
M2 — Tool Enable / Disable
```
