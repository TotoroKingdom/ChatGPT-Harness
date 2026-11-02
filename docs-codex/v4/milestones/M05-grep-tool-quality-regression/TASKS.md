# M05 TASKS — grep Tool and Quality Regression

> Inputs: SPEC.md, PLAN.md  
> Executor: Codex / Coding Agent  
> Status: IMPLEMENTATION BLOCKED UNTIL M04 ACCEPTED / FROZEN AND M05 VALIDATION DOCS ARE FROZEN  
> Scope: P0 F10 + mandatory F12 regression  
> Dependencies: M03 + M04 ACCEPTED / FROZEN and accepted M04 baseline handoff  
> External rule: do not run real paid/networked Model evaluation or real external ChatGPT E2E without explicit user authorization.  
> Contract rule: do not edit M04 baseline cases/prompts/fixtures/scorer/thresholds to make grep pass.

---

# 1. Dependency

~~~text
M03 ACCEPTED / FROZEN
        +
M04 ACCEPTED / FROZEN + accepted 72-run baseline
        ↓
M05 six docs frozen
        ↓
T1 Protocol/config/registry/policy
        ↓
T2 Grep engine/security
        ↓
T3 Runtime MCP/pipeline/evidence
        ↓
T4 UI/config upgrade/i18n
        ↓
T5 M04 candidate Eval/F12 regression mechanics
        ↓
T6 Regression/package/Development Complete
        ↓
Formal Integration/E2E/Manual
~~~

M04 Development Complete alone does not unlock M05 implementation.

---

# T1 — grep Protocol, Config Compatibility, Registry and Policy Contract

**AC:** M05-AC01, AC05, AC06  
**Primary areas:** protocol/src/v1, ui/src-tauri config migration, runtime ToolRegistry/Policy validation.

## Implement

1. Add grep to ToolConfig.
2. Add grep to ToolPolicyDefaults and decision_for.
3. Keep CURRENT_SCHEMA_VERSION = 3.
4. Brand-new ToolConfig default: grep enabled.
5. Existing v1/v2/v3 config missing grep: explicit grep disabled after additive normalization.
6. Existing/new policy default: grep ALLOW.
7. Add v3 additive normalizer:
   - normal load may persist atomically;
   - preserve configRevision;
   - read-only inspection writes nothing;
   - migration failure preserves original.
8. Add grep to RuntimeConfigSnapshot through ToolConfig.
9. Register grep after glob:
   - WorkspaceRead;
   - Low risk;
   - workspace scope.
10. Add grep to user Policy Tool matcher whitelist.
11. Regenerate TS/JSON Schema/protocol fixtures.

## Required unit tests

- new install grep ON;
- old v1/v2/v3 grep OFF;
- policy ALLOW;
- migration revision unchanged;
- explicit enable revision increments;
- restart semantics;
- registry order/capability;
- disabled registry behavior;
- invalid policy Tool rejected;
- serialization/unknown fields.

## Done

Config/registry/policy contract is complete before MCP exposes grep.

---

# T2 — Bounded Grep Engine and Windows Workspace Security

**AC:** M05-AC02, AC03, AC04, AC09, AC10, AC11  
**Depends on:** T1 types as needed.  
**Primary areas:** runtime/src/tools/grep.rs and Workspace security helpers.

## Implement

1. Typed GrepRequest/GrepResult.
2. Validate exact frozen argument ranges.
3. Literal matcher:
   - case-sensitive exact substring.
4. Regex:
   - Rust regex;
   - 2MiB compiled-program cap;
   - invalid -> INVALID_REGEX;
   - line-by-line.
5. Include/exclude via globset:
   - <=32 each;
   - <=512 bytes/pattern;
   - exclude wins;
   - invalid -> INVALID_FILTER_PATTERN.
6. Deterministic sorted depth-first traversal.
7. Enforce:
   - 50k discovered entries;
   - 10k scanned files;
   - 64MiB scanned bytes;
   - 8MiB/file.
8. UTF-8 + BOM.
9. Typed skip diagnostics:
   - binary/NUL;
   - invalid UTF-8;
   - too large;
   - unreadable;
   - reparse;
   - target changed;
   - unsupported.
10. One result per matching line.
11. Return path/line/preview/byte offsets.
12. Preview <=4096 UTF-8 bytes and includes first match.
13. maxResults 100 default / 1000 max.
14. maxOutputBytes 64KiB default / 256KiB max.
15. Deterministic truncation reasons.
16. 5s timeout -> SEARCH_TIMEOUT, no successful partial result.
17. Run filesystem scan off Tokio core worker.
18. Cancellation/deadline checks.
19. Windows:
   - do not traverse reparse directories;
   - candidate reparse skipped;
   - open candidate;
   - verify final opened-handle path under Workspace before reading.
20. Add test-only race hook for TOCTOU swap fixtures.

## Required tests

Cover SPEC §24 direct matrix.

Windows-only required fixtures include symlink/junction/reparse and outside-swap sentinels.

No outside sentinel bytes may appear in result/evidence/log-safe output.

## Done

Safe executor behavior is proven before production MCP wiring.

---

# T3 — ExecutionPipeline, MCP Adapter, Approval, Activity and M03 History

**AC:** M05-AC07, AC08, AC11, AC12  
**Depends on:** T1/T2.

## Implement

1. Add GrepRequest execution to ToolExecutor/ExecutionPipeline.
2. Capability comes only from ToolRegistry.
3. Create runtime/src/mcp/grep.rs.
4. Exact schema:
   - deny unknown fields;
   - apply frozen defaults/ranges;
   - use McpExecutionContext.
5. Register tools/list and tools/call routing.
6. Attach exact runtime_id + request_id correlation metadata.
7. Map grep typed errors safely.
8. Verify real Runtime Policy behavior:
   - ALLOW;
   - DENY;
   - ASK allow_once;
   - ASK deny;
   - disabled.
9. Add executor spy/hook proving no scan before authorization.
10. Approval summary contains safe logical scope only.
11. Extend closed SanitizedArgumentsEvidence/DurableSanitizedArgumentsEvidence with grep scalar facts.
12. Extend ExecutionResultEvidence with grep scalar result facts.
13. Update Audit/Activity/SafeEvidenceProjector.
14. Keep M03 physical schema v1.
15. Add raw-query/match/filter/privacy sentinels.
16. Verify exact identity against live Activity and History.

## Required tests

- literal real Runtime MCP;
- regex;
- invalid regex/filter;
- disabled;
- DENY;
- ASK allow/deny;
- Workspace violation;
- timeout/cancel;
- evidence/history;
- no raw query/match persistence.

## Done

grep is a real secure Runtime Tool, not an adapter shortcut.

---

# T4 — Tools UI, Upgrade UX and i18n

**AC:** M05-AC06, AC13  
**Depends on:** T1/T3.

## Implement

1. Add grep built-in description mapping to ToolsPage.
2. Ensure dynamic ToolConfig/runtime-name mapping recognizes grep.
3. Show saved enabled + running availability.
4. Show default Policy segmented control.
5. Show restart/mismatch state.
6. Inspector shows safe effective state only.
7. New install test:
   - ON / ALLOW.
8. Upgrade test:
   - explicit OFF / ALLOW;
   - configRevision preserved.
9. User enable test:
   - save;
   - revision increments;
   - Restart Required;
   - old Runtime unchanged;
   - restarted Runtime exposes grep.
10. Policy editor accepts grep matcher.
11. Add en/zh-CN.
12. Activity/History safe presentation for grep.

## Done

grep is discoverable/configurable without adding a separate search UI.

---

# T5 — M04 Baseline Preservation, M05 Candidate Eval and F12 Regression Mechanics

**AC:** M05-AC14, AC15, AC16, AC17, AC18  
**Depends on:** T1/T3 plus accepted M04 baseline handoff.

## Implement

### 1. Baseline immutability tests

Hash/verify unchanged:

- fixtures/v4/m04-eval/dataset.json;
- all case JSON;
- system prompt;
- fixture trees.

Do not edit them.

### 2. Preserve M04 baseline profile

Baseline still expects exactly:

~~~text
read_file
glob
write
edit
~~~

Extra grep remains HARNESS_CONTRACT_ERROR in baseline mode.

### 3. Profile abstraction

Add explicit profiles:

- m04 baseline;
- m05-grep-candidate-v1;
- m05-paired-control-v1.

Do not loosen one global catalog check.

### 4. Strict M05 candidate

Exact catalog:

~~~text
read_file
glob
grep
write
edit
~~~

Run unchanged m04-core-v1 and original strict scoring.

Candidate validity:

- 72 logical;
- >=70 scoreable;
- each case >=2/3;
- each category >=11/12;
- one candidate identity;
- zero security violations.

### 5. Grep call scoring plumbing

Allow Eval infrastructure to record/validate real grep schema/outcome without altering M04 case files.

If original M04 oracle does not allow grep for a case, strict selection scoring remains failure/unnecessary as originally defined.

### 6. Supplemental set

Create fixtures/v4/m05-eval:

~~~text
m05-grep-admission-v1
4 cases
3 repetitions
12 logical runs
~~~

Neutral tasks:

- literal;
- regex;
- scope/include/exclude;
- truncation/follow-up.

Report separately.

Validity:

- >=11/12 scoreable;
- each case >=2/3;
- security zero.

### 7. Baseline input

Accept machine-readable actual accepted M04 baseline.

Reject source-synthesized/non-formal/invalid baseline.

### 8. Comparator

Call existing compare_m05.

Do not copy thresholds into mutable CLI args.

Boundary tests must prove constants remain exact.

### 9. Model drift

If resolved identity differs:

- run full paired control with grep not exposed;
- exact M04 catalog/schema;
- call existing compare_provider_drift;
- over guard -> INCONCLUSIVE.

### 10. Local development only

Use scripted adapters for T5 development tests.

Do not send real paid requests without explicit user authorization.

## Done

Formal M05 Eval can run fairly later without rewriting M04.

---

# T6 — Cross-cutting Regression, Portable Delivery and Development Complete

**AC:** M05-AC19, AC20; closes local implementation readiness  
**Depends on:** T1–T5.

## 1. Direct grep matrix

Run real local Runtime/MCP functional/security matrix.

## 2. Windows security matrix

On actual Windows candidate verify:

- symlink;
- junction;
- reparse;
- outside target;
- TOCTOU swap;
- opened-handle final-path containment;
- no outside sentinel read.

Any required untested Windows security case blocks Development Complete.

## 3. M03 regression

- Activity identity;
- History write/query;
- typed grep evidence;
- no DB schema migration;
- privacy.

## 4. M04 regression

- baseline validate/preflight/scripted path;
- exact four-Tool baseline rejection of grep;
- dataset hashes unchanged;
- comparator constants/golden tests;
- candidate/control profile separation.

## 5. Existing product/security regression

- read_file;
- glob;
- write;
- edit;
- bash;
- Workspace;
- Tool toggles;
- Policy;
- Approval/SessionGrant;
- SecretStore;
- Start/Stop/Restart.

## 6. M01/M02 regression

- Tray/background/notifications;
- startup recorder;
- diagnostics.

## 7. Full checks

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Run repository milestone delivery/layout checks.

## 8. Fresh M05 Portable

Build fresh candidate and record path/hash.

Do not overwrite prior artifacts.

## 9. Development Complete response

Report in conversation:

- candidate SHA;
- package path/hash;
- grep Tool schema hash;
- config migration matrix;
- grep functional/security results;
- evidence privacy result;
- M04 baseline immutability result;
- scripted M05 Eval/comparator result;
- real Model regression = NOT EXECUTED unless authorized;
- real ChatGPT E2E = NOT EXECUTED unless authorized;
- remaining formal Integration/E2E/Manual.

Do not create a seventh milestone file.

### Done

Development Complete means ready for formal validation, not ACCEPTED/FROZEN.

---

# 2. Task → AC → Reserved Validation Traceability

| Task | M05 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01/05/06 | M05-INT-01/07/14, M05-E2E-03, M05-MAN-01/04 |
| T2 | AC02/03/04/09/10/11 | M05-INT-02/03/04/05/06/11/12, M05-E2E-01/02, M05-MAN-01/02 |
| T3 | AC07/08/11/12 | M05-INT-08/09/10/13, M05-E2E-01/02, M05-MAN-02/03 |
| T4 | AC06/13 | M05-INT-14, M05-E2E-03, M05-MAN-01/04 |
| T5 | AC14–AC18 | M05-INT-15/16/17, M05-E2E-03, M05-MAN-04 |
| T6 | AC19/20 | M05-INT-18, M05-E2E-04, M05-MAN-05 |

---

# 3. Development Handoff

After T6:

~~~text
M05 Development Complete
→ independent Integration
→ explicit authorization for real controlled-Model regression + external ChatGPT E2E
→ M05 E2E
→ Human Manual Acceptance
→ user sign-off
→ M05 ACCEPTED / FROZEN
→ unlock M06 V4 Core Release Gate
~~~

A local grep executor or scripted Eval run cannot unlock M06.
