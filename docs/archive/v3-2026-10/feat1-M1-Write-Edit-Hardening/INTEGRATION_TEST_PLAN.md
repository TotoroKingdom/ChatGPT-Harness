# M1 Integration Test Plan — Write/Edit Production Hardening

> Status: DRAFT / READY FOR AUTOMATED EXECUTION  
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Owner: Coding Agent  
> Scope: code-level / component-combination verification inside the Runtime package. No human interaction is required.

## 1. Responsibility Boundary

M1 verification is divided into four layers:

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

This document owns the **Integration** layer only.

Integration may use Runtime-internal deterministic test seams where required to prove failure safety, TOCTOU behavior or fault phases. It may instantiate Runtime components in-process when the test is specifically validating component composition.

Integration does **not** need to:

- operate External ChatGPT;
- operate a browser;
- click Harness UI;
- start the Tunnel client;
- prove the full packaged/near-final Runtime process boundary.

Those responsibilities are split between `E2E_TEST_PLAN.md` and `MANUAL_ACCEPTANCE_CHECKLIST.md`.

## 2. Objective

Verify that T1–T4 compose into the production M1 write/edit semantics without weakening the M0 Runtime enforcement chain:

```text
MCP adapter / Runtime integration surface
→ ExecutionPipeline
→ WorkspaceGuard
→ Policy
→ ExecutionPermit
→ write/edit executor
→ shared atomic file primitive
→ Activity/Audit
→ filesystem result
```

Integration must prove:

- atomic create / overwrite / edit behavior;
- staging / flush / commit failure safety;
- failed create leaves no named partial target;
- concurrent edit does not silently overwrite another actor;
- UTF-8 text boundary;
- Workspace / symlink / junction boundary revalidation;
- stable MCP error mapping and audit sanitization;
- M0 write/edit/read/glob regression compatibility.

## 3. Entry Criteria

The Coding Agent may start Integration when:

- T1–T4 implementation is complete;
- task-local Unit Tests pass;
- deterministic atomic fault injection tests exist;
- Windows existing-target atomic replacement has an automated test path;
- `cargo fmt --all -- --check` passes;
- `cargo check --workspace` passes;
- `cargo test --workspace` passes before dedicated Integration execution;
- `git diff --check` passes.

If implementation is incomplete:

```text
BLOCKED_BY_IMPLEMENTATION
```

Do not weaken assertions to force a PASS.

## 4. Integration Environment

Use disposable test directories, for example:

```text
base/
├ workspace/
│  ├ existing.txt
│  ├ edit.txt
│  ├ duplicate.txt
│  ├ utf8.txt
│  └ binary.bin
└ outside/
   └ outside.txt
```

Filesystem assertions must inspect final bytes, not only returned tool status.

Suggested fixtures:

```text
existing.txt = old content
edit.txt     = hello from ChatGPT
duplicate    = same same
utf8.txt     = 你好, ChatGPT 👋
binary.bin   = invalid UTF-8 bytes
outside.txt  = must not change
```

## 5. Required Integration Cases

### INT-01 — Ready MCP contract compatibility

Verify the composed Runtime exposes `read_file`, `glob`, `write`, `edit` with the M0-compatible schemas and annotations. No M2+ toggle/config semantics are introduced.

### INT-02 — Atomic write create

Create a missing file and assert exact bytes, `created=true`, no atomic temp residue, and successful execution evidence.

### INT-03 — Atomic write overwrite

Overwrite an existing UTF-8 file and assert the final file contains only complete new bytes, reports overwrite semantics, and leaves no temp residue.

### INT-04 — Exact UTF-8 byte semantics

Cover multibyte UTF-8, empty content, no trailing newline, caller-supplied LF/CRLF, and explicitly supplied BOM where representable. Harness must not normalize Unicode or line endings or append newlines.

### INT-05 — Existing binary overwrite rejected

Writing over invalid UTF-8 content returns `UNSUPPORTED_CONTENT`; original bytes remain unchanged; no temp residue; audit does not contain raw content.

### INT-06 — Atomic exact edit success

Unique exact replacement succeeds, final bytes are exact, `replacements=1`, and no temp residue remains.

### INT-07 — Edit 0-match / multi-match remain non-mutating

Return `EDIT_MATCH_NOT_FOUND` / `EDIT_MATCH_NOT_UNIQUE`; original bytes remain unchanged.

### INT-08 — Edit invalid UTF-8 remains non-mutating

Return `UNSUPPORTED_CONTENT`; original bytes remain unchanged.

### INT-09 — Concurrent edit source change returns `TARGET_CHANGED`

Use deterministic synchronization, not sleep races:

```text
Runtime reads original
→ replacement prepared/staged
→ test actor changes target
→ Runtime pre-commit check
```

Assert `TARGET_CHANGED`, external actor content remains, staged temp is cleaned up, and Activity/Audit records the correct failure category.

### INT-10 — Edit target removed/replaced before commit fails closed

Simulate target deletion/replacement after prepare. The old replacement must not be committed onto the new object.

### INT-11 — Atomic failure preserves existing target

Inject at least:

```text
TempWrite
TempFlush
Commit
```

For write/edit existing-target paths, assert old target bytes remain unchanged and no successful execution evidence is emitted.

### INT-12 — Failed create leaves no named target

Inject a pre-commit failure for a missing target. The named target must not appear and staging temp must be cleaned up.

### INT-13 — Target-created race does not silently clobber

If another actor creates the target after M1 observed it missing, M1 must return its deterministic conflict behavior and must not silently overwrite that actor's file.

### INT-14 — Workspace boundary survives atomic staging

Cover relative/absolute outside writes, outside edit, sibling-prefix escape, missing parent and Windows unsupported/root-relative forms. Staging must never escape into an unapproved outside directory.

### INT-15 — Symlink/junction boundary revalidation

On Windows, exercise a target/parent change between authorization and commit. Workspace escape must fail closed. `ENVIRONMENT_SKIP` is allowed only when fixture creation is unavailable and the reason is recorded.

### INT-16 — M0 write/edit MCP regression

Re-run local write create, overwrite, exact edit, outside rejection, malformed args and sanitized errors.

### INT-17 — `read_file` / `glob` regression

Verify normal read/glob paths and outside rejection; the new atomic helper must not change existing-path WorkspaceGuard semantics.

### INT-18 — Audit confidentiality / failure categories

For success, `TARGET_CHANGED` and atomic I/O failure, assert Activity/Audit can distinguish outcomes and does not contain raw mutation content, temp filename/path, unnecessary Workspace root or outside absolute path.

## 6. Fault-Injection Rules

M1 failure safety must be executed, not inferred from code review.

The Integration harness must deterministically trigger at least:

```text
TempWrite
TempFlush
Commit
Concurrent target change
```

Test seams must be Runtime-internal/test-only, disabled in production behavior, and must not expose a new MCP/Tauri control surface.

## 7. Windows Atomic Replace Requirement

At least one automated Windows test must exercise a real existing-target replacement and prove:

```text
old target bytes
→ production replacement primitive
→ new target bytes
```

The implementation must not be `delete target → rename temp`.

If the instantaneous atomic visibility window cannot be observed externally, combine observable filesystem assertions with primitive-level evidence for the actual Windows replacement API/semantics.

## 8. Agent Execution / Evidence Format

The Coding Agent executes all Integration cases without human intervention and records:

```text
Case ID
Result: PASS / FAIL / BLOCKED / ENVIRONMENT_SKIP
Automated test name / command
Relevant MCP/runtime result code
Filesystem before/after assertion
Temp residue assertion
Activity/Audit assertion where applicable
Injected phase where applicable
```

Secrets and raw temp absolute paths must not be copied into evidence.

## 9. Integration Exit Criteria

Integration is `PASS` only when:

- INT-01 through INT-18 pass, except an explicitly justified Windows symlink/junction `ENVIRONMENT_SKIP`;
- real Windows existing-target atomic replacement has automated evidence;
- atomic write create/overwrite and atomic edit pass;
- TempWrite/TempFlush/Commit failures preserve old target bytes;
- failed create leaves no named target;
- concurrent edit produces `TARGET_CHANGED` without clobbering external modification;
- UTF-8 and Workspace boundaries pass;
- M0 write/edit/read/glob regression passes;
- Activity/Audit sanitization passes.

When Integration passes, the **Coding Agent**, not a human reviewer, proceeds to `E2E_TEST_PLAN.md`.

Integration PASS alone is not M1 acceptance.

## 10. Non-Claims

Integration PASS does not claim:

```text
real process-boundary E2E complete
External ChatGPT/Tunnel complete
power-loss durable fsync guarantee
multi-file transaction guarantee
OS sandbox guarantee
M2+ capabilities complete
M1 ACCEPTED / FROZEN
```
