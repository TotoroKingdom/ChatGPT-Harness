# M10 SPEC — MCP Protocol Compatibility and Conformance

> Layout and validation lifecycle follow [AGENTS.md](../../../../AGENTS.md). This start step creates only SPEC / PLAN / TASKS; Integration / E2E / Manual plans are added later, keeping the final six-file milestone layout.
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M00+M05 ACCEPTED / FROZEN AND M10 SIX DOCS ARE FROZEN
> Milestone: M10 — MCP Protocol Compatibility and Conformance
> Scope: P1 F13 only
> Roadmap dependency: M00 + M05; M07/M08/M09 are not implementation dependencies
> Canonical ACs: F13-AC01 / F13-AC02 / F13-AC03
> Current main observed before this document: 384905c24ea584782ebdb7c33df72ae2e298cff5
> Current Runtime package: chatgpt-harness-runtime 0.1.3
> Current rmcp dependency and lock: 3.2.0
> Current transport: loopback-only Streamable HTTP at http://127.0.0.1:<port>/mcp
> Frozen MCP compatibility claim for M10: protocol revision 2025-03-26
> Frozen official conformance runner: modelcontextprotocol/conformance commit c37eec888e1c6ff140af79987a40008548b7cc5f, package 0.2.0-alpha.12, lockfile @modelcontextprotocol/sdk 1.29.0

---

# 1. Objective

M10 proves that the Harness Runtime MCP server is interoperable and protocol-correct at one explicitly frozen wire revision without confusing three different forms of evidence:

1. official MCP protocol conformance evidence;
2. Harness-specific Workspace / Policy / Approval / execution / evidence semantics;
3. real External ChatGPT → Tunnel → Runtime interoperability.

M10 may repair MCP adapter/transport compatibility defects discovered by the frozen tests.

M10 MUST NOT silently upgrade the claimed MCP protocol revision.

---

# 2. Canonical Product Contract

## F13-AC01

Fix the tested MCP protocol revision, rmcp / client / runner versions and verify initialize, tools/list, schema, tools/call, error, cancellation and timeout semantics where applicable.

## F13-AC02

When official conformance tooling is available, execute it and preserve the actual output. Anything not executed is NOT_VERIFIED rather than PASS.

## F13-AC03

Official protocol conformance, Harness-specific security semantics and real ChatGPT→Tunnel E2E are three independent evidence classes and cannot substitute for one another.

---

# 3. Frozen Protocol Identity

Formal M10 compatibility is limited to:

~~~text
MCP protocol revision = 2025-03-26
Transport = Streamable HTTP
Server role = MCP Server
Runtime SDK = rmcp 3.2.0
Runtime HTTP framework = axum 0.8
Runtime async = tokio 1.x as Cargo.lock resolves
Endpoint = loopback 127.0.0.1, OS-assigned port, /mcp
~~~

The current repository already initializes test clients with protocolVersion 2025-03-26 and sends MCP-Protocol-Version 2025-03-26 after session establishment.

M10 does not claim compatibility with:

- 2025-06-18;
- 2025-11-25;
- 2026-07-28;
- draft;
- future revisions.

A newer rmcp version or tolerant client does not expand the formal compatibility claim.

---

# 4. No Silent Wire Upgrade

Any material change to the claimed MCP wire revision requires:

1. explicit upstream product/spec decision;
2. new frozen compatibility matrix;
3. protocol adapter review;
4. official conformance rerun for that revision;
5. controlled client interoperability rerun;
6. real ChatGPT rerun.

M10 implementation cannot change 2025-03-26 to a newer revision merely to make the current conformance runner easier to pass.

Control Protocol remains v1.

AppConfig remains schema v3.

No M10 product config migration is required.

---

# 5. Official Conformance Runner Pin

Frozen runner source:

~~~text
repository = modelcontextprotocol/conformance
commit = c37eec888e1c6ff140af79987a40008548b7cc5f
package version = 0.2.0-alpha.12
package-lock = required
@modelcontextprotocol/sdk resolved version = 1.29.0
~~~

The runner is obtained into a disposable external/temp checkout or cache.

It is NOT vendored into ChatGPT-Harness and does not create a new repository top-level directory.

Frozen build:

~~~text
npm ci
npm run build
~~~

Frozen discovery command:

~~~text
node dist/index.js list --server --spec-version 2025-03-26
~~~

Frozen formal server command shape:

~~~text
node dist/index.js server
  --url <runtime-loopback-mcp-url>
  --suite active
  --spec-version 2025-03-26
  --output-dir <disposable-output-dir>
  --verbose
~~~

The exact single-line invocation may follow shell syntax of the execution environment, but the semantic options above cannot be omitted.

---

# 6. Important Official-runner Limitation

At the pinned runner commit, frozen requirement sets exist for later revisions, but there is no requirements set for 2025-03-26.

Therefore M10 MUST NOT run or claim:

~~~text
--requirements 2025-03-26
~~~

and MUST NOT describe the result as a frozen official 2025-03-26 requirements certification.

Correct wording:

~~~text
Official modelcontextprotocol/conformance runner evidence
filtered to the MCP 2025-03-26 wire and applicable server scenarios.
~~~

The runner evidence is still mandatory because it provides independent protocol and wire-schema checks.

---

# 7. Official Runner Result Semantics

Applicable result classifications:

## FAILURE

Any applicable official scenario FAILURE or wire-schema failure is an M10 blocker.

## SUCCESS

Counts as official runner success for the exact tested scenario/check only.

## WARNING

A WARNING is not silently converted to PASS.

It may remain non-blocking only when:

- the official runner itself classifies it as advisory / SHOULD / MAY;
- it does not expose a Harness security or interoperability defect;
- the formal execution evidence records it.

## SKIP / NOT_APPLICABLE

Neither PASS nor FAIL.

The reason must identify capability/version inapplicability.

## Runner unavailable / crash / missing result

~~~text
NOT_VERIFIED
~~~

and blocks M10 acceptance.

M10 starts with no expected-failure baseline. A failing official scenario cannot be hidden by creating an expected-failure file.

---

# 8. Evidence Preservation Contract

Formal execution preserves outside the six milestone planning files:

- runner repository/commit/package identity;
- package-lock hash;
- npm/node versions;
- list command output;
- exact server command;
- raw checks.json/results where emitted;
- stdout/stderr;
- exit code;
- Runtime candidate identity;
- Runtime endpoint identity without secrets;
- date/environment;
- applicable SUCCESS/FAILURE/WARNING/SKIP summary.

Execution results may live in normal test/artifact evidence locations or be reported in conversation.

Do not add a seventh milestone report document.

---

# 9. Independent Client Matrix

M10 uses four clients/evidence paths.

## 9.1 Official conformance client

Identity:

~~~text
modelcontextprotocol/conformance
commit c37eec888e1c6ff140af79987a40008548b7cc5f
internal locked @modelcontextprotocol/sdk 1.29.0
~~~

Purpose: official runner scenarios + wire-schema validation.

## 9.2 Harness raw wire client

Use the existing Rust/reqwest test stack or a narrowly-scoped test client.

It MUST NOT use rmcp for client-side message construction/validation.

Purpose: independently verify exact HTTP, JSON-RPC, session, schema, error and cancellation wire behavior.

## 9.3 Standalone official TypeScript MCP SDK client

Pin:

~~~text
@modelcontextprotocol/sdk = 1.29.0
~~~

Use it as a second controlled client against the final candidate through the real Tunnel during automated E2E.

Do not depend on latest semver resolution.

## 9.4 Real External ChatGPT

ChatGPT client internals and MCP SDK version are opaque.

Record only observable product/date/candidate facts.

Do not invent an SDK version.

This path is required for final real-user interoperability and cannot substitute for official conformance.

---

# 10. MCP 2025-03-26 Initialize Contract

The server must correctly handle the stateful initialize lifecycle.

Required:

- JSON-RPC 2.0 request/response framing;
- client sends protocolVersion 2025-03-26;
- server response negotiates 2025-03-26;
- serverInfo name/version are present and truthful;
- capabilities advertise only implemented server capabilities;
- tools capability is present;
- resources/prompts/sampling/roots or other unsupported capability claims are absent;
- initialized lifecycle is respected;
- initialize request is never cancelled by the client test;
- session behavior is consistent with Streamable HTTP 2025-03-26.

Formal M10 does not require supporting a newer requested protocol version.

If a client and server cannot agree on the frozen revision, the connection cannot be counted as interoperable.

---

# 11. Streamable HTTP Contract

Runtime MCP remains:

~~~text
127.0.0.1 only
OS-assigned port
/mcp
stateful Streamable HTTP
~~~

M10 verifies:

- endpoint is not bound to a non-loopback interface;
- allowed Origin behavior remains bounded;
- initialize can establish a session;
- subsequent request/session mechanics are valid;
- HTTP Content-Type / Accept behavior is spec-compatible;
- request/notification/response status behavior is valid;
- malformed/invalid session behavior fails safely;
- shutdown releases the listener;
- no raw secret or physical path appears in HTTP/protocol errors.

M10 does not introduce remote Runtime binding. Tunnel remains the external transport owner.

---

# 12. tools/list Contract

The MCP tools/list result is a projection of Runtime effective exposure, not the declared registry.

Required:

- only currently exposed Tools are listed;
- disabled tools remain hidden;
- unavailable tools remain hidden unless an already-frozen product policy explicitly exposes a policy-denied tool;
- ordering is deterministic;
- every item has a unique stable name;
- description is bounded/truthful;
- inputSchema is valid for MCP 2025-03-26;
- annotations contain only fields valid at 2025-03-26;
- no later-revision Tool fields are emitted on the frozen wire;
- no Tool appears solely because a test fixture knows about it.

M05 grep remains part of the expected post-M05 catalog where enabled/effective.

M09 is not an M10 dependency; M10 tests whatever final production catalog exists in the candidate and does not require M09 tools.

---

# 13. Frozen 2025-03-26 Tool Result Projection

Current Runtime internal tool adapters use typed structured payloads.

M10 preserves those internal typed payloads, but the 2025-03-26 wire MUST use only fields allowed by that revision's CallToolResult:

~~~text
content
isError?
~~~

The 2025-03-26 wire MUST NOT emit:

~~~text
structuredContent
outputSchema
or another later-version CallToolResult field
~~~

## 13.1 Success projection

For a successful typed Tool result:

1. serialize the existing typed safe result object to deterministic compact JSON;
2. put that JSON in a TextContent item;
3. isError is false or omitted consistently;
4. do not emit structuredContent.

The JSON text is the canonical machine-readable payload for the 2025-03-26 wire.

A generic human summary may be an additional bounded TextContent item only if it is deterministic and does not replace the canonical JSON result.

## 13.2 Compatibility invariant

This is a protocol adapter projection only.

It MUST NOT change:

- executor return semantics;
- WorkspaceGuard;
- PolicyEngine;
- Approval;
- ExecutionPermit;
- AuditEvent;
- Safe Evidence;
- History;
- F12 scoring truth.

---

# 14. Protocol Error vs Tool Execution Error Contract

MCP 2025-03-26 distinguishes protocol errors from Tool execution errors.

## 14.1 Unknown Tool

A tools/call for a name that the MCP server cannot find returns a JSON-RPC protocol error:

~~~text
code = -32602
Invalid Params
~~~

The message is bounded and must not echo an unbounded user-controlled tool name.

It is NOT a successful CallToolResult and is NOT -32601 Method Not Found for this frozen M10 contract.

## 14.2 Invalid Tool arguments

Schema/deserialization validation failure before valid Tool execution returns:

~~~text
code = -32602
Invalid Params
~~~

Do not return a normal success envelope with isError=true for malformed arguments.

## 14.3 Tool execution/product error

After a valid known Tool request enters Runtime product semantics, failures such as:

- Workspace violation;
- target missing/changed;
- Policy DENY;
- Approval denied/expired/cancelled;
- executor IO failure;
- grep/bash timeout;
- safe cancellation;

remain Tool execution results:

~~~text
content = safe bounded TextContent
isError = true
~~~

No structuredContent on the 2025-03-26 wire.

The TextContent must carry a deterministic safe error representation containing the existing stable product code and message, without raw physical path/OS error/secret.

---

# 15. Disabled / Unavailable Tool Contract

Disabled/unavailable Tools are normally absent from tools/list.

A stale or manually-crafted client may still call a known product Tool name.

That request must:

- remain subject to the authoritative Runtime pipeline;
- never bypass Tool enabled state / Policy / Approval;
- return safe product error semantics;
- not mutate filesystem or execute Bash when unavailable;
- not be counted as proof of protocol failure merely because the product deliberately refuses the operation.

Official protocol conformance and product exposure semantics remain separately classified.

---

# 16. Cancellation Contract

MCP 2025-03-26 defines notifications/cancelled for cancelling an in-flight request.

M10 must prove the Runtime transport connects cancellation to the existing request cancellation token.

Formal controlled cancellation case:

- start a deliberately long-running cancellable grep request in a disposable Workspace;
- send notifications/cancelled for that exact in-flight request;
- observe work ceases promptly;
- no leaked active request;
- no orphan child process/resource;
- evidence records truthful cancelled/incomplete state according to existing semantics;
- cancellation of a completed request does not resurrect or mutate it.

Forbidden:

- cancelling initialize;
- using Runtime process kill as the only cancellation evidence;
- treating client disconnect alone as proof that executor work stopped.

If rmcp's Streamable HTTP implementation uses stream closure as an additional cancellation mechanism, test and record it separately; do not invent a different protocol message.

---

# 17. Timeout Contract

MCP 2025-03-26 does not create a Harness-specific timeout RPC.

M10 separates:

## Client timeout

The controlled client stops waiting at a bounded deadline and must cancel/close safely.

The Runtime must not leak active work indefinitely.

## Product execution timeout

Existing grep/bash timeout behavior remains Harness-specific product semantics.

It is tested as a Tool execution error/cancellation lifecycle, not counted as a distinct MCP protocol method.

M10 does not change existing timeout thresholds merely to satisfy conformance.

---

# 18. Malformed and Invalid Wire Contract

The raw wire matrix covers at least:

- malformed JSON;
- wrong JSON-RPC shape;
- missing request id where a request id is required;
- invalid method envelope;
- initialize with invalid params;
- tools/list invalid params;
- tools/call invalid params;
- invalid/unknown Tool;
- invalid session id;
- invalid/missing relevant HTTP headers;
- disallowed Origin;
- request before valid lifecycle state where applicable.

Responses must be protocol-valid, bounded and sanitized.

No failure response may expose:

- secret;
- environment;
- raw argv;
- physical Workspace path;
- raw OS error details;
- internal stack trace.

---

# 19. Capability Truth Contract

Runtime currently advertises tools capability.

M10 MUST NOT advertise an MCP capability merely because rmcp can model it.

Unless actually implemented and separately scoped, the server does not claim:

- resources;
- prompts;
- sampling;
- roots;
- elicitation;
- tasks;
- completion;
- logging feature support beyond any unavoidable base protocol behavior;
- listChanged for tools unless Runtime actually emits the required notification semantics.

Unsupported official scenarios are SKIP/NOT_APPLICABLE, not PASS.

---

# 20. Harness-specific Security Evidence

Separate from official conformance, M10 revalidates:

~~~text
MCP request
→ Tool exposure/dispatch
→ WorkspaceGuard
→ PolicyEngine
→ Approval when ASK
→ ExecutionPermit
→ executor
→ Audit/Safe Evidence
~~~

Required product cases:

- ALLOW success;
- Policy DENY;
- ASK → allow;
- ASK → deny;
- disabled Tool;
- Workspace escape;
- invalid argument;
- target change where applicable;
- cancellation;
- product timeout where applicable;
- unknown Tool observation does not grant authority.

Passing these does not count as official MCP conformance.

---

# 21. Real Tunnel Controlled-client Evidence

Automated E2E uses the pinned standalone TypeScript MCP SDK 1.29.0 client through the real Tunnel to the packaged candidate.

At minimum:

- initialize;
- tools/list;
- call read_file or another safe available Tool;
- call grep when M05 catalog makes it available;
- observe one safe error;
- close session cleanly.

This is interoperability evidence.

It does not substitute for the official conformance runner.

---

# 22. Real External ChatGPT Evidence Boundary

Real External ChatGPT acceptance uses:

~~~text
ChatGPT
→ real Tunnel
→ packaged Runtime MCP
→ actual tools/list
→ actual tools/call
→ actual Tool result/error
~~~

Minimum:

- ChatGPT discovers the expected effective Tool catalog;
- at least one safe Tool call succeeds;
- at least one safe expected product error is understandable;
- no protocol-level incompatibility breaks the connection;
- evidence identity matches the same candidate.

External ChatGPT execution requires explicit user authorization at execution time.

If not executed:

~~~text
NOT_VERIFIED / BLOCKED
~~~

for final M10 acceptance.

---

# 23. Official Conformance Does Not Override Security

If an official conformance scenario expects generic Tool behavior that conflicts with the Harness security model:

1. first verify whether the scenario is actually applicable to 2025-03-26;
2. do not bypass Workspace / Policy / Approval to make it pass;
3. isolate protocol fixture behavior only if the fixture remains a real Runtime path and does not weaken product security;
4. otherwise record the incompatibility/blocker and require an explicit product decision.

No conformance-only backdoor or developer bypass is allowed.

---

# 24. Conformance Fixture Tool Boundary

If the official runner needs a deterministic Tool with behavior the production file Tools cannot safely provide, M10 may expose a test-only conformance fixture Tool only when all of the following hold:

- it is compiled/activated only under formal test configuration;
- it is impossible to expose in normal packaged production operation;
- it still traverses Runtime ToolRegistry / Policy / Execution boundaries as applicable;
- it has no arbitrary filesystem/shell/network side effect;
- its existence is visible in official-run evidence;
- it cannot be used to claim production Tool semantics.

Prefer testing the real existing Tool catalog before adding such a fixture.

---

# 25. Version / Identity Manifest

Formal M10 evidence records:

~~~text
Harness candidate commit
product version
Runtime hash
Tunnel hash
AppConfig schema
Control Protocol revision
MCP revision
rmcp version
Cargo.lock hash
official conformance repo
official conformance commit
official conformance package version
official conformance package-lock hash
official conformance internal MCP SDK version
raw client implementation version/hash
standalone TS SDK version
effective Tool catalog hash
effective Tool schema hash
OS / Node / Rust versions
~~~

A product-impacting change after official/client evidence invalidates the affected evidence and requires rerun.

---

# 26. Wire-change Invalidation Rule

Changes to any of these require official runner + raw-wire rerun:

- MCP HTTP transport;
- initialize/server info/capabilities;
- session behavior;
- Tool list metadata/schema;
- tools/call request validation;
- Tool result wire projection;
- protocol error mapping;
- cancellation plumbing.

Changes to Runtime Workspace/Policy/Approval/executor require affected Harness-security rerun.

Package/Tunnel changes that affect external reachability require controlled Tunnel + External ChatGPT rerun.

---

# 27. M10 Derived Acceptance Criteria

| M10 AC | Requirement |
| --- | --- |
| M10-AC01 | Formal target is MCP 2025-03-26 over stateful Streamable HTTP, rmcp 3.2.0, and all client/runner identities are pinned and recorded. |
| M10-AC02 | M10 claims only 2025-03-26; no silent wire upgrade occurs; Control Protocol remains v1 and AppConfig remains schema v3. |
| M10-AC03 | Official runner is pinned to conformance commit c37eec888e1c6ff140af79987a40008548b7cc5f / 0.2.0-alpha.12 / SDK 1.29.0 and uses --spec-version 2025-03-26 rather than the nonexistent 2025-03-26 requirements set. |
| M10-AC04 | initialize negotiates 2025-03-26 with truthful serverInfo/capabilities and correct stateful initialized/session lifecycle. |
| M10-AC05 | Streamable HTTP remains loopback-only and passes origin, session, HTTP content negotiation, lifecycle and clean shutdown checks. |
| M10-AC06 | tools/list exposes only effective Tools in deterministic order and stays consistent with Runtime authority. |
| M10-AC07 | Every exposed Tool inputSchema/metadata/annotations is valid for MCP 2025-03-26 and emits no later-revision Tool fields. |
| M10-AC08 | Successful tools/call emits a 2025-03-26-valid CallToolResult using content plus optional isError, never structuredContent, while preserving the typed safe result as deterministic JSON TextContent. |
| M10-AC09 | Invalid Tool arguments are JSON-RPC Invalid Params -32602 protocol errors and do not enter valid execution semantics. |
| M10-AC10 | Unknown Tool is a bounded JSON-RPC Invalid Params -32602 protocol error and cannot grant authority. |
| M10-AC11 | Valid known Tool product/execution failures return safe content with isError=true rather than being misclassified as protocol success or leaking structuredContent. |
| M10-AC12 | Disabled/unavailable Tools are hidden from normal tools/list; stale direct calls remain safely rejected by authoritative product semantics. |
| M10-AC13 | notifications/cancelled stops a controlled in-flight cancellable Tool without cancelling initialize, leaking work or inventing terminal success. |
| M10-AC14 | Client timeout/disconnect cleanup and Harness-specific grep/bash timeout are explicitly separated and both preserve truthful cancellation/error state. |
| M10-AC15 | Malformed JSON/JSON-RPC, invalid lifecycle/session/header/origin cases fail with bounded protocol-safe responses and no secret/path/stack leakage. |
| M10-AC16 | Runtime advertises only implemented MCP capabilities; unsupported resources/prompts/sampling/other scenarios are not falsely claimed. |
| M10-AC17 | Pinned official server suite filtered to 2025-03-26 has zero applicable FAILURE/wire-schema failures; warnings/skips follow the frozen classification and missing runner evidence is NOT_VERIFIED. |
| M10-AC18 | Independent raw Rust/reqwest wire client passes the frozen initialize/list/schema/call/error/session/cancel matrix without relying on rmcp as client. |
| M10-AC19 | Pinned standalone @modelcontextprotocol/sdk 1.29.0 client interoperates with the packaged final candidate through the real Tunnel. |
| M10-AC20 | Workspace/Policy/Approval/Execution/Evidence security semantics pass independently and are not relabeled as official protocol conformance. |
| M10-AC21 | Real External ChatGPT through the real Tunnel discovers the expected Tool catalog, completes a safe Tool call and receives an understandable safe error on the same candidate. |
| M10-AC22 | Final package/evidence identity is fixed; wire-impacting changes invalidate affected evidence, and M10 acceptance requires all three evidence classes rather than any one substitute. |

---

# 28. Canonical to Derived to Task to Validation Traceability

| Canonical | Derived | Task owner | Reserved Integration | Reserved E2E | Reserved Manual |
| --- | --- | --- | --- | --- | --- |
| F13-AC01 | AC01-AC16/18/19 | T1/T2/T3/T5 | M10-INT-01/02/03/04/05/06/07/08/09/10/11/12 | M10-E2E-01/02/03 | M10-MAN-01/03 |
| F13-AC02 | AC03/17/22 | T4/T6 | M10-INT-13/14 | M10-E2E-01/04 | M10-MAN-01/02 |
| F13-AC03 | AC17/20/21/22 | T4/T5/T6 | M10-INT-15/16 | M10-E2E-02/03/04 | M10-MAN-03/04/05 |

Reserved suites:

- Integration: M10-INT-01 through M10-INT-16.
- E2E: M10-E2E-01 through M10-E2E-04.
- Manual: M10-MAN-01 through M10-MAN-05.

---

# 29. Development Entry Gate

Formal M10 implementation may begin only when:

1. M00 is ACCEPTED / FROZEN;
2. M05 is ACCEPTED / FROZEN;
3. all six M10 documents are frozen;
4. current Runtime still uses the audited MCP architecture or deviations are reconciled before implementation;
5. pinned official runner source remains obtainable or the gate is BLOCKED.

M07/M08/M09 are not M10 implementation dependencies.

---

# 30. Development Exit Gate

M10 Development Complete requires:

- pinned identity/version manifest implemented;
- 2025-03-26 wire projection implemented;
- protocol-vs-tool error classification implemented;
- raw-wire compatibility suite PASS;
- lifecycle/session/cancel/timeout integration PASS;
- official pinned conformance server run actually executed with zero applicable FAILURE;
- Harness security regression PASS;
- controlled standalone SDK loopback preflight PASS;
- fresh Portable candidate produced;
- Formal Integration/E2E/Manual truthfully NOT EXECUTED unless actually run.

A development-time official runner execution is product development verification; it does not substitute for independent formal Integration execution against the final accepted candidate.

---

# 31. Final Exit Gate

M10 may become ACCEPTED / FROZEN only when:

- F13-AC01/02/03 PASS;
- M10-AC01 through M10-AC22 PASS;
- pinned official conformance execution against final candidate has zero applicable FAILURE/wire-schema failure;
- no official evidence item is missing/NOT_VERIFIED;
- raw independent client matrix PASS;
- Harness security matrix PASS;
- packaged controlled SDK client through real Tunnel PASS;
- real External ChatGPT through real Tunnel PASS;
- any WARNING/SKIP is explicitly classified and non-blocking by frozen rules;
- no expected-failure baseline hides a failure;
- no Workspace/Policy/Approval bypass exists;
- final candidate identity matches all acceptance evidence;
- no current blocker remains.

Successful wording:

~~~text
M10 — ACCEPTED / FROZEN
~~~

M10 does not claim newer MCP revisions and does not by itself produce V4 — ACCEPTED / FROZEN.
