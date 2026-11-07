# M10 PLAN — MCP Protocol Compatibility and Conformance

> Input: SPEC.md
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M00+M05 ACCEPTED / FROZEN AND M10 VALIDATION DOCS ARE FROZEN
> Scope: P1 F13 only
> Primary owners: Runtime MCP adapter / Streamable HTTP transport / protocol test harness
> Protocol target: MCP 2025-03-26 only
> Official runner: modelcontextprotocol/conformance @ c37eec888e1c6ff140af79987a40008548b7cc5f
> Security rule: protocol repair must not weaken Workspace / Policy / Approval / ExecutionPermit.
> External rule: real External ChatGPT, changes to active managed instances, and persistent/destructive tests require explicit execution-time authorization.

---

# 1. Delivery Strategy

M10 separates protocol correctness from product-security correctness and from real-client interoperability.

~~~text
M00 + M05 ACCEPTED / FROZEN
        ↓
M10 six docs frozen
        ↓
T1 compatibility identity + raw-wire harness
        ↓
T2 MCP 2025-03-26 wire projection/error repair
        ↓
T3 lifecycle/session/cancel/timeout/negative-wire coverage
        ↓
T4 pinned official conformance runner integration
        ↓
T5 independent SDK client + Harness security separation
        ↓
T6 full regression + Portable handoff
        ↓
Formal Integration
        ↓
Packaged controlled-client E2E
        ↓
Real External ChatGPT Manual Acceptance
        ↓
M10 — ACCEPTED / FROZEN
~~~

M10 is not a protocol-upgrade milestone.

---

# 2. Implementation Principles

1. Target exactly MCP 2025-03-26.
2. Keep rmcp at the audited 3.2.0 unless a defect makes the frozen target impossible and an explicit dependency revision is approved.
3. Any rmcp dependency change is product-impacting and invalidates compatibility evidence.
4. Preserve Runtime internal typed Tool results.
5. Project those results into a revision-valid 2025-03-26 wire shape.
6. Protocol validation errors and Tool execution errors remain distinct.
7. Do not bypass Product security to make official scenarios pass.
8. Official runner, Harness-specific tests and real ChatGPT remain separate evidence tracks.
9. Do not vendor the official conformance repository into this repository.
10. Do not create a new top-level conformance project.
11. Keep Control Protocol v1 and AppConfig schema v3.
12. Preserve M05 grep behavior and current effective Tool exposure.
13. Any unsupported official capability is marked inapplicable, not implemented solely to inflate conformance.
14. No expected-failure baseline is used to hide product failures.
15. Development completion is not M10 acceptance.

---

# 3. Expected Landing Zones

Use existing product/test locations:

~~~text
runtime/src/mcp/
runtime/src/server/
runtime/tests/
runtime/Cargo.toml
Cargo.lock

protocol/
fixtures/
ui/scripts/
~~~

If a helper script is useful, place it under an existing script/test area such as:

~~~text
ui/scripts/
runtime/tests/
~~~

Do not add a repository-level conformance directory.

Official runner source/output uses a system temporary directory or external cache.

---

# 4. Phase A — Freeze Compatibility Identity

## 4.1 Add one source-controlled M10 compatibility manifest

Use an existing fixture/test-data location, for example:

~~~text
fixtures/v4/m10-mcp/
~~~

Record:

- MCP protocol revision 2025-03-26;
- rmcp 3.2.0;
- Streamable HTTP;
- official conformance repository;
- official runner commit;
- runner package version;
- internal runner SDK 1.29.0;
- standalone SDK 1.29.0;
- required scenarios/coverage classes;
- result classification rules.

The manifest is test input, not an acceptance report.

## 4.2 Dependency lock assertions

Add automated assertions that:

- runtime/Cargo.toml resolves rmcp 3.2.0;
- Cargo.lock exact rmcp package is 3.2.0;
- official runner package-lock in the pinned source resolves @modelcontextprotocol/sdk 1.29.0;
- standalone client package lock resolves exactly 1.29.0.

## 4.3 Raw-wire client harness

Extend existing reqwest-based test infrastructure rather than adding a new application.

Capabilities:

- start a real Runtime MCP endpoint;
- send initialize;
- capture session id;
- POST JSON-RPC;
- send notifications;
- close/abort requests;
- inspect status/headers/body;
- validate exact JSON shape independently of rmcp client types.

Exit: one immutable compatibility identity and one SDK-independent wire oracle exist.

---

# 5. Phase B — 2025-03-26 Wire Projection

## 5.1 Isolate internal result from wire result

Refactor MCP adapters so internal execution still produces the current typed safe payload.

Add a single 2025-03-26 projector responsible for:

- successful CallToolResult;
- Tool execution error CallToolResult;
- protocol error mapping.

Do not duplicate serialization per Tool.

## 5.2 Success projection

Convert typed safe payload to canonical compact JSON TextContent.

Wire fields:

~~~text
content
isError?
~~~

Forbidden:

~~~text
structuredContent
outputSchema
later-revision result fields
~~~

Add exact JSON-shape tests for read_file, glob, grep, write and edit.

Bash follows the same projection if exposed in the product context under test.

## 5.3 Tool execution error projection

Known valid Tool request that reaches product semantics:

- preserve stable Harness code/message;
- encode them as deterministic safe TextContent;
- isError=true;
- no structuredContent;
- no physical path / raw OS detail.

## 5.4 Protocol errors

Before valid execution:

### Unknown Tool

Return JSON-RPC Invalid Params:

~~~text
-32602
~~~

### Invalid arguments

Return JSON-RPC Invalid Params:

~~~text
-32602
~~~

The protocol adapter must stop invalid calls before executor-side mutation.

## 5.5 Metadata/schema projection

Review every Tool metadata field generated by rmcp.

Ensure the 2025-03-26 wire contains only revision-valid:

- name;
- description;
- inputSchema;
- annotations.

No later output schema or revision-specific extension.

Exit: all Tools share one explicit versioned wire projector.

---

# 6. Phase C — Lifecycle and Streamable HTTP

## 6.1 Initialize

Test real HTTP initialize:

- request revision 2025-03-26;
- truthful serverInfo;
- tools capability;
- response revision;
- state/session transition;
- initialized notification/lifecycle where transport/SDK requires.

Do not advertise unsupported primitives.

## 6.2 Session handling

Verify:

- session id accepted after initialize;
- invalid session rejected safely;
- requests do not cross session identity;
- endpoint shutdown releases state/port.

## 6.3 Origin and loopback

Keep listener 127.0.0.1 only.

Test:

- expected local Origin;
- absent Origin where valid;
- disallowed Origin;
- no host-wide binding.

## 6.4 HTTP negotiation

Test relevant:

- Content-Type;
- Accept;
- POST semantics;
- notification response status;
- JSON vs event-stream response handling used by rmcp.

Do not build a second HTTP server for formal product tests.

Exit: transport behavior is validated as part of the real Runtime endpoint.

---

# 7. Phase D — Negative Wire and Error Matrix

## 7.1 Malformed JSON

Send malformed body and verify bounded protocol/HTTP failure.

## 7.2 Invalid JSON-RPC

Cover:

- invalid jsonrpc version;
- bad id shape where applicable;
- invalid method envelope;
- invalid params shape.

## 7.3 tools/list

Cover:

- before/after lifecycle;
- invalid params;
- effective catalog;
- deterministic schema.

## 7.4 tools/call

Cover:

- success;
- unknown Tool;
- invalid args;
- disabled/unavailable direct call;
- Workspace violation;
- Policy DENY;
- Approval deny;
- IO/executor failure.

Check protocol-vs-Tool-error classification exactly.

## 7.5 Cancellation

Create deterministic long-running grep fixture.

Issue notifications/cancelled for that exact request.

Assert:

- cancellation token observed;
- work stops;
- no executor leak;
- no state resurrection;
- safe evidence.

## 7.6 Client timeout / disconnect

Abort a controlled long request.

Verify bounded cleanup and truthful state.

Keep this distinct from Harness grep/bash execution timeout.

Exit: all negative paths are machine-checkable without external ChatGPT.

---

# 8. Phase E — Official Conformance Runner Integration

## 8.1 Obtain pinned source

Use disposable temp/cache location.

Verify commit:

~~~text
c37eec888e1c6ff140af79987a40008548b7cc5f
~~~

Verify package:

~~~text
0.2.0-alpha.12
~~~

Verify locked SDK:

~~~text
1.29.0
~~~

If any identity differs, fail before execution.

## 8.2 Build pinned runner

~~~text
npm ci
npm run build
~~~

Do not use latest npx resolution without the pinned checkout.

## 8.3 Discover exact scenarios

Run:

~~~text
node dist/index.js list --server --spec-version 2025-03-26
~~~

Persist the output.

This is important because the suite evolves while the M10 pin does not.

## 8.4 Execute official server suite

Start the real Runtime endpoint with a disposable Workspace/config.

Run the frozen command shape from SPEC.

Capture:

- exit code;
- checks;
- stdout/stderr;
- wire-schema checks;
- scenario list;
- environment.

## 8.5 Result parser

Add a narrow parser/checker that classifies official output:

- SUCCESS;
- FAILURE;
- WARNING;
- SKIP/NOT_APPLICABLE;
- missing/corrupt result.

Do not turn warnings into success.

Do not add expected-failure suppression.

## 8.6 Fix loop

If a real conformance defect appears:

1. classify protocol vs product;
2. fix only adapter/transport behavior needed;
3. rerun raw-wire tests;
4. rerun official suite;
5. rerun affected Harness security tests.

Do not alter frozen M10 docs or thresholds.

Exit: development candidate has actual official runner evidence, not a simulated conformance label.

---

# 9. Phase F — Harness-specific Security Regression

Run independently from the official suite.

Required real Runtime cases:

- read_file ALLOW;
- grep ALLOW;
- write/edit ASK where configured;
- ASK allow;
- ASK deny;
- Policy DENY;
- disabled Tool;
- Workspace escape;
- stale/target-changed mutation case;
- unknown Tool observation;
- cancellation;
- product timeout where applicable.

Verify:

- no bypass;
- no pre-approval mutation;
- evidence identity stable;
- protocol projection does not alter authorization truth.

These results are explicitly labelled Harness-specific security/product evidence.

---

# 10. Phase G — Controlled Client Interoperability

## 10.1 Raw Rust client

Already covered in Integration-level development tests.

It is independent of rmcp.

## 10.2 Standalone TypeScript MCP SDK 1.29.0

Use an isolated test fixture/package under existing fixture/test layout.

Lock exact SDK 1.29.0.

Test loopback first:

- initialize;
- tools/list;
- safe tools/call;
- safe expected error;
- close.

## 10.3 Real Tunnel harness

Prepare automated E2E driver capable of connecting the same pinned SDK client through the actual Tunnel endpoint.

Do not execute external/tunnel actions against a managed active instance without execution-time authorization.

The validation plan will define the formal E2E invocation.

Exit: the second controlled client is ready and version-locked.

---

# 11. Phase H — External ChatGPT Boundary

M10 development does not impersonate ChatGPT.

Formal Manual/E2E boundary later verifies the same final package with real External ChatGPT.

No internal controlled client result may be relabelled as External ChatGPT.

---

# 12. Compatibility Fix Rules

A M10 fix is allowed when it is one of:

- MCP 2025-03-26 wire shape repair;
- protocol error classification;
- Streamable HTTP/lifecycle/session compatibility;
- cancellation propagation;
- capability/metadata truthfulness;
- conformance harness/testability seam that does not weaken security.

A M10 fix is NOT allowed to:

- add unrelated product features;
- implement resources/prompts/tasks merely because runner has scenarios;
- bypass Policy/Approval;
- change grep semantics;
- change M05 evaluation thresholds;
- upgrade MCP revision;
- upgrade Control Protocol;
- bump AppConfig for no reason.

---

# 13. Rollback Strategy

M10 has no persistent data migration.

If a compatibility fix regresses product behavior:

- revert MCP adapter/transport changes;
- keep user config and Evidence DB unchanged;
- rebuild previous accepted candidate;
- rerun smoke;
- no filesystem user data migration is needed.

If rmcp itself is changed under explicit approval, rollback restores Cargo.toml/Cargo.lock pair together.

---

# 14. Development Verification Commands

Minimum product checks:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

M10-specific development verification additionally includes:

- raw-wire suite;
- exact rmcp lock assertion;
- Tool schema snapshot validation;
- cancellation/timeout lifecycle tests;
- pinned official runner build;
- pinned official runner list command;
- pinned official server suite;
- standalone SDK 1.29.0 loopback interoperability;
- M05 grep security/regression coverage;
- repository/milestone layout;
- fresh Portable build.

---

# 15. Execution Evidence Boundaries

Formal conformance output is not stored as new milestone planning docs.

Allowed evidence locations:

- test temp/output while running;
- established artifact candidate area where delivery scripts permit supporting validation records;
- conversation handoff;
- existing formal test source fixtures.

Do not add:

~~~text
M10_CONFORMANCE_REPORT.md
EVIDENCE_INDEX.md
results/
official-runner/
~~~

inside the milestone directory.

---

# 16. Candidate Invalidation

Rerun official + raw-wire evidence if any of these changes:

- runtime/src/mcp;
- MCP HTTP transport;
- rmcp dependency/lock;
- Tool schemas/metadata;
- protocol error mapping;
- cancellation wiring.

Rerun Harness security if:

- Workspace;
- Policy;
- Approval;
- ExecutionPipeline;
- executor semantics;

change.

Rerun real Tunnel controlled client if:

- Tunnel;
- Runtime package;
- public MCP endpoint behavior;

changes.

Rerun real ChatGPT on final acceptance candidate after any external-chain-impacting change.

---

# 17. Development Completion Gate

M10 Development Complete requires:

- T1–T6 complete;
- 2025-03-26 wire result projection PASS;
- invalid args/unknown Tool protocol classification PASS;
- lifecycle/session/origin/negative-wire matrix PASS;
- cancellation/timeout cleanup PASS;
- pinned official runner build/list/run completed;
- zero applicable official FAILURE/wire-schema failure;
- Harness security regression PASS;
- standalone SDK 1.29.0 loopback PASS;
- fresh Portable candidate;
- Formal Integration/E2E/Manual truthfully NOT EXECUTED unless actually run.

Development Complete is not M10 ACCEPTED / FROZEN.
