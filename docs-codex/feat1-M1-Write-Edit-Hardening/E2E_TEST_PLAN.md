# M1 E2E Test Plan — Local Black-box Runtime/MCP Regression

> Status: DRAFT / READY FOR AUTOMATED EXECUTION  
> Milestone: M1 — Write/Edit Production Hardening  
> Inputs: `SPEC.md`, `PLAN.md`, `TASKS.md`, `INTEGRATION_TEST_PLAN.md`  
> Baseline: M0 — ACCEPTED / FROZEN  
> Owner: Coding Agent  
> Scope: local black-box E2E against a final / near-final Runtime build. No human interaction is required.

## 1. Responsibility Boundary

M1 uses this verification split:

```text
Unit Tests           → Coding Agent
Integration Tests    → Coding Agent
Local Black-box E2E  → Coding Agent
Manual Acceptance    → Human Reviewer
```

This document owns **Local Black-box E2E** only.

External ChatGPT, Tunnel and Harness UI are **not** part of this automated E2E layer. They belong to `MANUAL_ACCEPTANCE_CHECKLIST.md` because they require real user/product interaction.

All E2E cases in this document must be executable by Codex locally without:

- browser automation;
- clicking Harness UI;
- External ChatGPT;
- Tunnel credentials;
- manual file edits between cases.

## 2. What Counts as M1 E2E

The E2E runner may be test-only code, but the system under test must be the real built Runtime process and real external surfaces:

```text
Coding Agent test runner
→ actual compiled chatgpt-harness-runtime process
→ stdin/stdout Control RPC initialize
→ mcp.endpoint.get
→ real loopback MCP HTTP endpoint
→ MCP tools/list + tools/call
→ ExecutionPipeline / WorkspaceGuard / Policy / executor
→ real Workspace filesystem
→ real Activity pipeline
→ activity.list over Control RPC
```

The runner must **not** call these as its main E2E execution path:

```text
RuntimeCore::execute(...)
ToolExecutor::execute(...)
McpService::call_tool(...) in-process
atomic helper directly
mock MCP transport
fake Activity sink
```

Those are Unit/Integration techniques, not E2E.

## 3. Build Requirement

E2E must run against an explicit final / near-final build identity.

Preferred target:

```text
cargo build -p chatgpt-harness-runtime --release
```

and then spawn the produced `chatgpt-harness-runtime` binary.

A Cargo integration-test binary target is acceptable if it launches the real package binary through `CARGO_BIN_EXE_chatgpt-harness-runtime`; it must not substitute an in-process Runtime instance.

The E2E server process must use production-default behavior:

- no atomic fault injection;
- no test-only alternate executor;
- no mock filesystem;
- no alternate WorkspaceGuard;
- no fake Activity source.

## 4. Required Automated Runner Contract

The Coding Agent may implement the runner as Rust integration-test code or an equivalent checked-in test-only harness.

The runner must autonomously:

1. create disposable `workspace/` and adjacent `outside/` directories;
2. start the actual Runtime binary with piped stdin/stdout;
3. send the real Control RPC `initialize` request using the production protocol;
4. send the `initialized` acknowledgement;
5. request `mcp.endpoint.get`;
6. connect to that returned loopback URL using a real MCP Streamable HTTP client;
7. call `tools/list` / `tools/call` through MCP;
8. inspect actual filesystem bytes;
9. query `activity.list` through Control RPC;
10. stop the Runtime through `runtime.stop` or equivalent graceful control flow;
11. fail the test process on any mismatch.

A single command should be sufficient to execute the complete E2E suite after the runner exists, for example:

```text
cargo test -p chatgpt-harness-runtime --test m1_e2e -- --nocapture
```

The exact filename/command may differ, but execution must remain non-interactive and reproducible.

## 5. Entry Criteria

The Coding Agent starts E2E only when:

- M1 development T1–T4 is complete;
- Unit Tests pass;
- `INTEGRATION_TEST_PLAN.md` result = PASS;
- `cargo fmt --all -- --check` passes;
- `cargo check --workspace` passes;
- `cargo test --workspace` passes;
- the E2E target build identity is recorded;
- the real Runtime process can start from the test runner.

No human confirmation is an Entry Criterion.

If the local machine cannot launch the actual Runtime binary or loopback MCP endpoint, mark the E2E run:

```text
BLOCKED — LOCAL TEST ENVIRONMENT
```

Do not replace it with an in-process mock and call that E2E.

## 6. Test Fixture

The runner creates all fixtures itself:

```text
base/
├ workspace/
│  ├ duplicate.txt
│  └ optional pre-existing fixtures
└ outside/
   └ outside.txt
```

Initial values where needed:

```text
duplicate.txt = same same
outside.txt   = must not change
```

No case may depend on a human preparing or repairing the fixture.

## 7. Required E2E Cases

### E2E-01 — Runtime process boot + real MCP exposure

The runner:

- spawns the actual Runtime build;
- initializes it with the disposable Workspace;
- obtains the real `mcp.endpoint.get` URL;
- connects over loopback MCP HTTP;
- lists tools.

Expected tools include:

```text
read_file
glob
write
edit
```

Also query `activity.list` to prove the Control/Activity surface belongs to the same Runtime generation.

### E2E-02 — Real MCP write create

Call `write` over the real MCP endpoint:

```json
{"path":"m1-e2e.txt","content":"hello from M1"}
```

Assert:

- MCP returns success;
- `workspace/m1-e2e.txt` exists;
- bytes equal exactly `hello from M1`;
- no additional newline or normalization;
- no `.chatgpt-harness-*` temp residue remains;
- `activity.list` contains the corresponding request/execution success evidence.

### E2E-03 — Real MCP write full overwrite

Call `write` again:

```json
{"path":"m1-e2e.txt","content":"hello from M1 overwrite"}
```

Assert:

- success;
- final bytes equal exactly the new content;
- no old-content residue;
- no atomic temp residue;
- Activity contains successful write evidence.

This case verifies production observable overwrite behavior. Deterministic commit-failure safety remains an Integration responsibility.

### E2E-04 — Real MCP exact edit + read-back

Call `edit`:

```json
{
  "path":"m1-e2e.txt",
  "oldText":"hello from M1 overwrite",
  "newText":"hello from M1 hardened"
}
```

Then call `read_file` over the same real MCP endpoint.

Assert:

- edit succeeds;
- final filesystem bytes equal exactly `hello from M1 hardened`;
- read_file returns the same content;
- no temp residue;
- Activity contains successful edit/read evidence.

### E2E-05 — Ambiguous edit fails without mutation

Prepare `duplicate.txt = same same` automatically, then call:

```json
{
  "path":"duplicate.txt",
  "oldText":"same",
  "newText":"changed"
}
```

Assert:

- tool result is an error with stable `EDIT_MATCH_NOT_UNIQUE` semantics;
- filesystem bytes remain `same same`;
- no temp residue;
- Activity contains failure evidence and no successful edit completion for that request.

### E2E-06 — Workspace boundary rejected through real MCP

Use an absolute or otherwise validly formed path pointing into the adjacent disposable `outside/` directory and call `write`.

Assert:

- real MCP call reaches Runtime and is rejected as Workspace violation;
- `outside.txt` remains exactly `must not change` or the requested outside target is not created;
- no outside temp file is created;
- error is sanitized;
- Activity records Workspace rejection and no successful execution.

### E2E-07 — Exact UTF-8 production path

Call `write` for:

```text
m1-utf8.txt = 你好，M1 👋
```

Then call `read_file`.

Assert:

- filesystem bytes equal the caller-supplied UTF-8 bytes exactly;
- MCP read-back is semantically identical;
- no automatic newline/Unicode normalization;
- Activity contains successful mutation/read evidence.

### E2E-08 — Runtime shutdown / process cleanup

After all tool cases:

- issue graceful Runtime stop through the real control path;
- assert child process exits;
- assert the loopback MCP endpoint is no longer usable;
- assert Workspace files remain in their final committed state;
- assert no atomic temp residue remains.

## 8. Deliberately Integration-only Cases

Do not use the E2E layer to inject internal failure hooks for:

- TempCreate / TempWrite / TempFlush / Commit failure;
- deterministic `TARGET_CHANGED` synchronization;
- target-created race hook;
- junction/symlink mutation hook;
- cleanup-failure hook.

Those are intentionally proven by Integration tests. E2E uses production-default behavior against the actual Runtime process.

## 9. Evidence Required

The Coding Agent records for each case:

```text
Case ID
Result: PASS / FAIL / BLOCKED
Build / commit identity
Runner command
Runtime process identity / generation identity where available
MCP endpoint loopback assertion
Observed MCP tool/result or stable error code
Filesystem byte assertion
Activity assertion
Temp-residue assertion where applicable
```

Do not record secrets or unnecessary absolute sensitive paths.

## 10. E2E Exit Criteria

M1 Local Black-box E2E is `PASS` only when:

- E2E-01 through E2E-08 execute without human intervention;
- the actual compiled Runtime process is the server under test;
- MCP calls travel through the real loopback MCP HTTP endpoint;
- filesystem assertions use real files under a disposable Workspace;
- Activity assertions come from the Runtime `activity.list` control surface;
- write create / overwrite pass;
- exact edit + read-back pass;
- ambiguous edit remains non-mutating;
- Workspace escape remains fail-closed;
- UTF-8 path passes;
- graceful shutdown passes;
- no case is substituted by mock/in-process executor testing.

When E2E passes, the automated verification chain is complete:

```text
Unit PASS
→ Integration PASS
→ Local Black-box E2E PASS
```

Only then does the process move to `MANUAL_ACCEPTANCE_CHECKLIST.md` for human final acceptance.

## 11. Non-Claims

Automated Local E2E does not claim:

```text
External ChatGPT product behavior verified
Tunnel verified
Harness desktop UI verified
browser/UI interaction verified
power-loss durability verified
multi-file transaction support
M2+ functionality
M1 ACCEPTED / FROZEN
```

Those external/user-facing observations belong to Manual Acceptance where applicable.
