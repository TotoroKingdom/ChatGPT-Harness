# M00 Controlled Agent Evaluation Feasibility Report

> Scope: T5 local feasibility implementation.
> Status: runner mechanics PASS; real model/Agent smoke NOT EXECUTED in this development round.
> This report must not be read as F12 acceptance or M00 E2E PASS.

## 1. Isolated implementation

Path: `evaluation/m00-poc/` with an explicit empty Cargo workspace. It is not imported by the production Runtime/Tauri crates.

The spike contains:

- `EvalCase`: versioned task, fixture hash, allowed/expected tools, argument constraints, Policy expectation, final-state assertion, limits, model/adapter identity, tool-schema identity and scorer version.
- `AgentAdapter`: provider-neutral turn boundary.
- `ScriptedAgentAdapter`: local deterministic adapter for mechanics tests only.
- `HttpJsonAgentAdapter`: concrete user-supplied JSON-over-HTTP adapter path; it does not choose a permanent model vendor and accepts an optional runtime Bearer token without writing it to reports.
- `McpTransport`: abstraction with `InMemoryMcpTransport` for local mechanics and `HttpMcpTransport` for the real loopback Runtime MCP path (`initialize`, `tools/list`, `tools/call`).
- `WorkspaceFinalStateProbe`: reads only a checked relative path under an isolated Workspace and grades the final observable state independently from transport outcome.
- `EvalReport`: separates Tool selection, argument validity, Runtime call outcome, final task completion, call count and termination reason.

## 2. Versioned smoke case

- Case: `evaluation/m00-poc/cases/read-file-smoke.json`
- Fixture: `evaluation/m00-poc/fixtures/read-file/README.md`
- Fixture SHA-256: `8a3367b637c9e3699c4f5576658f76db44c53d1b254656bef4c05cf563293f5b`
- Expected tool: `read_file`
- Argument constraint: Workspace-relative `path`, expected `README.md`
- Policy expectation: `ALLOW`
- Final assertion: exact known fixture content
- Limits: 4 steps, 10 seconds, zero declared spend budget
- Adapter/model fields: provider-neutral identity fields are explicit placeholders until an authorized real endpoint is selected
- Scorer: `m00-final-state-scorer-v1`

The runner computes a SHA-256 case identity and observed tool-schema identity in each report. The committed case pins an equivalent catalog identity (`m00-read-file-tool-schema-v1`) without pretending that a real Runtime schema was observed in this local-only run.

## 3. Local verification

```text
cargo test --manifest-path evaluation/m00-poc/Cargo.toml
```

Result: **PASS** — 7 library tests passed, 0 failed; binary tests: 0; doc-tests: 0.

Covered local mechanics:

- case parsing and fixture SHA-256 verification;
- stable case identity and runner version;
- argument schema validation and unknown-field rejection;
- real runner loop plumbing through an in-memory transport;
- invalid calls recorded without forwarding;
- max-step enforcement;
- Runtime transport success separated from wrong final-state failure;
- relative Workspace final-state probe and traversal rejection;
- tool call count and cleanup through temporary Workspace ownership.

## 4. Real execution boundary

The following run was intentionally not performed in this round:

```text
M00-E2E-02 — real controllable Agent/Model → runner → real Runtime MCP → final-state grade
```

Reason: Roadmap/SPEC requires explicit authorization at execution time for a paid/networked model and separately frozen E2E evidence; the user requested this round stop at M00 Development Complete and not enter E2E or Manual Acceptance. Therefore the real adapter result is `NOT_VERIFIED`/`BLOCKED`, not a mock-derived PASS.

No provider credential, raw token, model response or external call evidence was committed.

## 5. Recommendation for M04

M04 can reuse this boundary after M03 supplies the Shared Evidence contract and an explicitly authorized controllable model endpoint. It must replace the scripted mechanics adapter with the real adapter, pin model/adapter/schema/config identities, run isolated Runtime/MCP cases, retain independent final-state grading, and freeze its sample/threshold contract before candidate evaluation.
