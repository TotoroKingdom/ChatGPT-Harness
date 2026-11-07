# M10 TASKS — MCP Protocol Compatibility and Conformance

> Inputs: SPEC.md, PLAN.md
> Executor: Codex / Coding Agent
> Status: IMPLEMENTATION BLOCKED UNTIL M00+M05 ACCEPTED / FROZEN AND M10 VALIDATION DOCS ARE FROZEN
> Scope: P1 F13 only
> Frozen wire: MCP 2025-03-26 over Streamable HTTP
> Frozen Runtime SDK: rmcp 3.2.0
> Frozen official runner: modelcontextprotocol/conformance @ c37eec888e1c6ff140af79987a40008548b7cc5f
> Rule: official protocol conformance, Harness security semantics and real External ChatGPT evidence are separate and all required for final acceptance.

---

# 1. Dependency

~~~text
M00 + M05 ACCEPTED / FROZEN
        ↓
M10 six docs frozen
        ↓
T1 Identity manifest + independent raw-wire harness
        ↓
T2 2025-03-26 result/error/schema projection
        ↓
T3 lifecycle/session/cancel/timeout/negative-wire integration
        ↓
T4 pinned official conformance runner execution
        ↓
T5 Harness security + standalone SDK interoperability
        ↓
T6 full regression + Portable handoff
        ↓
Integration → E2E → Manual
        ↓
M10 — ACCEPTED / FROZEN
~~~

M07/M08/M09 are not implementation dependencies.

---

# T1 — Freeze Compatibility Identity and Build an SDK-independent Raw-wire Harness

**AC:** M10-AC01, AC02, AC03, AC18, AC22
**Primary areas:** fixtures/, runtime/tests/, runtime/src/mcp tests, Cargo.toml/Cargo.lock assertions.
**Production behavior change:** none.

## 1. Add M10 compatibility fixture

Use an existing fixture hierarchy such as:

~~~text
fixtures/v4/m10-mcp/compatibility-v1.json
~~~

Freeze:

~~~text
protocolRevision = 2025-03-26
transport = streamable_http
rmcp = 3.2.0
officialRunnerRepo = modelcontextprotocol/conformance
officialRunnerCommit = c37eec888e1c6ff140af79987a40008548b7cc5f
officialRunnerPackage = 0.2.0-alpha.12
officialRunnerSdk = 1.29.0
standaloneSdk = 1.29.0
~~~

Include hashes of relevant fixture/schema inputs.

This fixture is a test contract, not an execution result.

## 2. Assert Runtime dependency identity

Add tests/build checks that fail if:

- runtime/Cargo.toml no longer requests rmcp 3.2.0;
- Cargo.lock resolves rmcp to another exact version;
- Streamable HTTP server feature disappears;
- target revision constant/fixture drifts.

A deliberate upgrade must require new frozen docs rather than silently updating the fixture.

## 3. Build raw-wire HTTP helper

Reuse reqwest 0.12 or existing low-level HTTP test support.

The helper must not use rmcp client types.

Support:

- raw initialize request;
- arbitrary JSON-RPC body;
- request/notification;
- session header;
- MCP-Protocol-Version header;
- Origin;
- Accept / Content-Type;
- response status/headers/body capture;
- stream/request abort;
- bounded timeout.

## 4. Runtime launch fixture

Start a real Runtime MCP endpoint against disposable Workspace/config.

Capture:

- loopback endpoint;
- candidate Runtime version;
- effective tool list;
- session ID where issued.

Cleanly shut down and remove temp files.

## 5. Exact response assertions

Add reusable assertions for:

- JSON-RPC response;
- JSON-RPC error;
- CallToolResult 2025-03-26 shape;
- no forbidden later fields;
- no secret/physical path leakage.

### Verification

Run targeted Runtime MCP tests.

### Done

M10 has a version-pinned contract and independent wire oracle before product compatibility changes begin.

---

# T2 — Implement the MCP 2025-03-26 Tool-result and Error Projection

**AC:** M10-AC06 through AC12, AC15, AC16, AC20
**Depends on:** T1
**Primary areas:** runtime/src/mcp/service.rs, adapters, error projection/tests.

## 1. Introduce one versioned wire projector

Centralize 2025-03-26 output projection rather than fixing every Tool independently.

Responsibilities:

- successful typed Tool payload;
- known Tool execution error;
- unknown Tool protocol error;
- invalid arguments protocol error.

No Policy/Workspace logic belongs in the projector.

## 2. Preserve internal typed results

Refactor as needed so existing adapters/executors still produce the same typed safe payload internally.

Do not change:

- read_file semantics;
- glob semantics;
- grep semantics;
- write/edit mutation truth;
- bash execution truth;
- Audit/Safe Evidence.

## 3. Remove structuredContent from the frozen wire

For every successful Tool response at 2025-03-26:

- serialize existing safe result as deterministic compact JSON;
- return it in TextContent;
- isError false or omitted consistently;
- assert structuredContent is absent;
- assert outputSchema and later fields are absent.

Do not discard fields the Agent needs to complete the task.

## 4. Project execution errors

For valid known Tool requests that fail in product execution:

~~~text
result.content = safe deterministic error JSON/text
result.isError = true
~~~

Preserve current stable Harness error code/message.

No raw OS/path detail.

## 5. Unknown Tool

Change the MCP ServerHandler mapping for a tools/call unknown name to:

~~~text
JSON-RPC code -32602
Invalid Params
~~~

Use bounded generic wording.

Do not echo an arbitrary attacker-controlled name.

Keep the existing unknown-tool audit observation if it remains safe.

## 6. Invalid arguments

Move schema/deserialization failures that occur before valid execution to:

~~~text
JSON-RPC code -32602
Invalid Params
~~~

They must not appear as a successful result envelope with isError=true.

Add malformed/unknown-field/wrong-type/missing-required tests for every representative Tool schema.

## 7. tools/list revision-valid schema

Review generated Tool objects.

Assert:

- name;
- description;
- inputSchema;
- valid 2025-03-26 annotations only;
- no later-version metadata fields.

## 8. Golden wire tests

At minimum for:

- read_file success;
- glob success;
- grep success;
- write success;
- edit success;
- representative product error;
- invalid arguments;
- unknown Tool.

### Done

The MCP adapter emits one explicit 2025-03-26 wire shape while Runtime internals remain unchanged.

---

# T3 — Complete Streamable HTTP Lifecycle, Session, Cancellation, Timeout and Negative-wire Coverage

**AC:** M10-AC04, AC05, AC13, AC14, AC15, AC16, AC18, AC20
**Depends on:** T1/T2
**Primary areas:** runtime/src/mcp/http.rs, service.rs, runtime/server integration tests.

## 1. Initialize lifecycle

Using the raw client:

- initialize protocolVersion 2025-03-26;
- verify server response version;
- verify serverInfo;
- verify capabilities;
- capture session ID;
- complete initialized lifecycle required by rmcp/transport.

Assert no unsupported capability claim.

## 2. Pre/post lifecycle behavior

Cover:

- tools/list before valid lifecycle state;
- tools/call before valid lifecycle state;
- after initialization/ack readiness;
- after shutdown.

Responses must remain spec/product truthful.

## 3. Session tests

Cover:

- valid session;
- missing session where required;
- invalid session;
- session from another endpoint/run;
- shutdown invalidates old session.

No cross-session product authority.

## 4. Origin / listener tests

Assert:

- address is 127.0.0.1;
- expected local Origin accepted;
- disallowed Origin rejected;
- no 0.0.0.0/IPv6-any binding unless separately approved.

## 5. HTTP header/content tests

Cover relevant:

- Accept application/json;
- Accept text/event-stream combinations;
- Content-Type application/json;
- invalid/missing headers;
- POST request;
- notification response status.

Keep assertions revision-specific.

## 6. Malformed wire matrix

Cover:

- malformed JSON;
- invalid jsonrpc field;
- invalid request object;
- invalid initialize params;
- invalid tools/list params;
- invalid tools/call params;
- invalid method;
- invalid session/header.

Verify bounded sanitized responses.

## 7. Cancellation fixture

Create a deterministic large disposable Workspace that keeps grep in-flight long enough for cancellation.

Process:

1. initialize;
2. start grep tools/call;
3. send notifications/cancelled for exact request id;
4. verify cancellation token reaches search;
5. verify request terminates;
6. verify no active request leak;
7. verify final evidence state is cancelled/incomplete, never fabricated success.

Never cancel initialize.

## 8. Connection abort / client timeout

Start controlled request and abort/timeout the client transport.

Verify:

- bounded Runtime cleanup;
- no runaway executor;
- next request can proceed;
- state remains truthful.

## 9. Product timeout separation

Run representative grep/bash timeout tests where existing product fixtures support them.

Label them Harness product semantics, not MCP-native timeout RPC evidence.

### Done

The real Streamable HTTP endpoint has a complete stateful negative/cleanup matrix.

---

# T4 — Integrate and Execute the Pinned Official MCP Conformance Runner

**AC:** M10-AC03, AC17, AC22
**Depends on:** T1–T3
**Primary areas:** existing scripts/test harness + disposable external runner checkout.
**Do not vendor:** modelcontextprotocol/conformance.

## 1. Add runner bootstrap/check logic

Implement a narrow existing-script-area command that:

- creates/uses a temp runner directory;
- obtains the exact official repository commit;
- verifies HEAD SHA;
- verifies package version;
- verifies package-lock;
- verifies locked @modelcontextprotocol/sdk 1.29.0.

If identity differs, fail before running.

Do not mutate repository top-level layout.

## 2. Build the runner

Execute:

~~~text
npm ci
npm run build
~~~

Capture Node/npm versions and build exit code.

## 3. List applicable server scenarios

Execute exactly:

~~~text
node dist/index.js list --server --spec-version 2025-03-26
~~~

Capture list output/hash.

Assert the command does not use:

~~~text
--requirements 2025-03-26
~~~

because the pinned runner has no frozen requirements set for that revision.

## 4. Start real Runtime MCP endpoint

Use a disposable Workspace and production Runtime MCP endpoint.

Do not start a mock MCP server as the server under test.

## 5. Run official suite

Execute frozen semantics:

~~~text
node dist/index.js server
  --url <real-runtime-url>
  --suite active
  --spec-version 2025-03-26
  --output-dir <temp-dir>
  --verbose
~~~

Capture:

- stdout;
- stderr;
- exit;
- checks/results;
- wire-schema checks;
- scenario list;
- runtime logs needed for safe correlation.

## 6. Parse official results

Classify:

- SUCCESS;
- FAILURE;
- WARNING;
- SKIP/NOT_APPLICABLE;
- missing/corrupt.

Gate:

~~~text
applicable FAILURE = 0
wire-schema FAILURE = 0
runner missing/crash = NOT_VERIFIED / blocker
~~~

WARNING/SKIP must stay visible.

## 7. No expected-failure suppression

Do not create or pass an expected-failures file to turn failures green.

If a runner defect is suspected:

- reproduce;
- preserve raw evidence;
- keep M10 BLOCKED/NOT_VERIFIED;
- do not patch the official runner silently.

## 8. Conformance-driven repair loop

For each real product defect:

- patch Runtime adapter/transport;
- rerun T2/T3;
- rerun official suite;
- rerun affected security cases.

### Done

An actual pinned official runner has tested the real Harness Runtime against the frozen 2025-03-26 wire.

---

# T5 — Prove Harness Security Separately and Add the Pinned Standalone MCP SDK Client

**AC:** M10-AC18 through AC21
**Depends on:** T1–T4
**Primary areas:** Runtime integration tests, existing E2E client fixture/script locations.

## 1. Harness security matrix

Against real Runtime pipeline, execute:

- ALLOW read_file;
- ALLOW grep;
- ASK allow mutation;
- ASK deny mutation;
- Policy DENY;
- disabled Tool;
- Workspace escape;
- target change where applicable;
- unknown Tool;
- cancellation;
- product timeout.

Assert:

- no security bypass;
- no mutation before approval;
- stable request identity/evidence;
- wire-projection changes do not alter Policy truth.

Label this evidence Harness-specific, not official conformance.

## 2. Build standalone TypeScript SDK client fixture

Pin exactly:

~~~text
@modelcontextprotocol/sdk = 1.29.0
~~~

Use a lockfile.

The client is test-only and located in an existing fixture/test structure.

It must support:

- initialize 2025-03-26;
- tools/list;
- tools/call;
- safe error observation;
- clean close.

## 3. Loopback interoperability

Run the standalone SDK client against the local real Runtime endpoint.

Assert:

- negotiated connection succeeds;
- expected tools visible;
- read_file/grep success payload remains usable;
- safe error can be consumed;
- no structuredContent dependency exists.

## 4. Real Tunnel driver preparation

Wire the same pinned controlled client to accept a supplied external Tunnel MCP URL for formal E2E.

Do not execute against the user's active instance without authorization.

## 5. External ChatGPT boundary

Do not automate or fake External ChatGPT in T5.

Formal Manual/E2E plan owns it.

### Done

M10 has official, raw-client and second-SDK evidence paths with security classification kept distinct.

---

# T6 — Full Regression, Package Identity and Development Handoff

**AC:** M10-AC01 through AC22
**Depends on:** T1–T5

## 1. Rust checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

## 2. Frontend checks

M10 should not need UI product changes, but run normal repository regression:

~~~text
cd ui
npm run typecheck
npm test
npm run build
~~~

## 3. Protocol/fixture checks

Verify:

- Control Protocol remains v1;
- AppConfig remains schema v3;
- MCP compatibility fixture exact;
- rmcp remains 3.2.0;
- official runner pin exact;
- standalone SDK lock exact;
- no accidental 2025-06-18/2025-11-25/draft formal claim.

## 4. Repeat critical wire gate

Re-run against final development candidate:

- initialize;
- tools/list/schema;
- success result;
- invalid args;
- unknown Tool;
- product error;
- cancellation;
- malformed/session/origin matrix.

## 5. Repeat official runner

Run the pinned official suite against the final development candidate.

Record:

~~~text
applicable failure count
wire-schema failure count
warning count
skip/not-applicable count
runner exit
result hash
~~~

Required development gate:

~~~text
applicable failure = 0
wire-schema failure = 0
~~~

## 6. Existing product regression

Re-run affected:

- read_file;
- glob;
- grep;
- write;
- edit;
- Bash security/availability;
- Workspace/Policy/Approval;
- M05 grep regression;
- Activity/History/evidence identity where affected.

No F12 threshold change.

## 7. Repository governance

Run:

- ui/scripts/check-repository-layout.ps1;
- ui/scripts/check-milestone-layout.ps1.

At this stage layout check is expected to PASS only after all six M10 docs exist; if the three validation docs have not yet been created/frozen, report the gate truthfully rather than weakening the script.

## 8. Fresh Portable

Use existing delivery flow with milestone:

~~~text
M10-mcp-protocol-compatibility-conformance
~~~

Do not overwrite prior candidate artifacts.

## 9. Portable smoke

Against disposable Workspace:

- start packaged Runtime;
- obtain MCP endpoint;
- raw client initialize/list/call;
- validate no structuredContent on frozen wire;
- safe error;
- shutdown/no orphan.

This is not formal E2E.

## 10. Development Complete output

Report in conversation:

~~~text
M10 Development Complete
candidate SHA:
product version:
Runtime hash:
Tunnel hash:
MCP revision:
rmcp:
Cargo.lock hash:
official runner repo/commit/package:
official runner SDK:
official list output hash:
official result hash:
official applicable failures:
official wire-schema failures:
official warnings:
official skips:
raw-wire matrix:
standalone SDK:
Harness security matrix:
cancellation:
client-timeout cleanup:
AppConfig schema:
Control Protocol:
effective Tool catalog hash:
effective Tool schema hash:
Portable path/hash:
Formal Integration: NOT EXECUTED
Formal E2E: NOT EXECUTED
Real External ChatGPT: NOT EXECUTED
Manual: NOT EXECUTED
M10 ACCEPTED/FROZEN: NO
~~~

### Done

M10 development candidate is ready for independent validation.

---

# 2. Task to AC to Reserved Validation Traceability

| Task | M10 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01/02/03/18/22 | M10-INT-01/02/13, M10-E2E-01, M10-MAN-01/02 |
| T2 | AC06/07/08/09/10/11/12/15/16/20 | M10-INT-03/04/05/06/07/08, M10-E2E-01/02, M10-MAN-03 |
| T3 | AC04/05/13/14/15/16/18/20 | M10-INT-02/09/10/11/12/15, M10-E2E-01/02, M10-MAN-03/04 |
| T4 | AC03/17/22 | M10-INT-13/14, M10-E2E-01/04, M10-MAN-01/02 |
| T5 | AC18/19/20/21 | M10-INT-15/16, M10-E2E-02/03/04, M10-MAN-03/04/05 |
| T6 | AC01-AC22 | M10-INT-01 through 16, M10-E2E-01 through 04, M10-MAN-01 through 05 |

---

# 3. Development Handoff

After T6:

~~~text
M10 Development Complete
→ independent M10 Integration
→ packaged controlled-client E2E through real Tunnel
→ real External ChatGPT acceptance
→ user sign-off
→ M10 — ACCEPTED / FROZEN
→ contributes to M11 V4 Full Gate
~~~

M10 does not claim newer MCP protocol revisions and does not itself produce V4 — ACCEPTED / FROZEN.
