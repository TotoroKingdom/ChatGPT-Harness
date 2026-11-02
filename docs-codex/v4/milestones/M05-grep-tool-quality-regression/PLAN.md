# M05 PLAN — grep Tool and Quality Regression

> Input: SPEC.md  
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M04 ACCEPTED / FROZEN  
> Scope: P0 F10 + mandatory F12 regression  
> Dependencies: M03 + M04 ACCEPTED / FROZEN  
> Layout: extend existing protocol/runtime/ui/evaluation/fixtures trees only; no new top-level project.  
> External rule: real ChatGPT and real paid/networked controlled-Model E2E require explicit user authorization before execution.

---

# 1. Delivery Strategy

~~~text
M04 ACCEPTED / FROZEN + accepted baseline handoff
        ↓
M05 six docs frozen
        ↓
T1 Protocol / config / registry / policy contract
        ↓
T2 Bounded grep engine + Windows path security
        ↓
T3 ExecutionPipeline / MCP / Activity / History
        ↓
T4 UI / config migration / i18n
        ↓
T5 M04 candidate Eval profile + F12 comparator integration
        ↓
T6 Cross-cutting regression / package / Development Complete
        ↓
Formal Integration
        ↓
Authorized real Model + real ChatGPT E2E
        ↓
Manual Acceptance
~~~

No task may weaken M04 comparator thresholds or M03 evidence identity.

---

# 2. Module Ownership

| Capability | Owner |
| --- | --- |
| grep protocol/config enums | protocol/src/v1 |
| additive v3 config normalization | ui/src-tauri/src/config |
| Tool registration/capability | runtime/src/tools/registry.rs |
| grep request/result/executor | runtime/src/tools/grep.rs |
| secure traversal/open validation | runtime/src/workspace + grep executor helper |
| MCP adapter/schema | runtime/src/mcp/grep.rs |
| ExecutionPipeline integration | runtime/src/execution |
| Policy default/matcher | protocol + existing Runtime Policy path |
| Audit/Activity evidence | runtime/src/audit + protocol activity DTO |
| durable grep evidence | runtime/src/evidence + protocol history DTO |
| Tools UI | ui/src/pages/tools |
| i18n | ui/src/i18n |
| strict M05 Eval candidate profile | runtime/src/evaluation |
| M05 supplemental fixed data | fixtures/v4/m05-eval |
| M04 baseline dataset | fixtures/v4/m04-eval — read-only upstream input |

No grep-specific second Policy engine, Workspace guard or evidence pipeline.

---

# 3. Phase A — Protocol, Config, Registry and Policy

Extend ToolConfig with grep and ToolPolicyDefaults with grep.

Regenerate protocol TS/JSON Schema/fixtures through existing generation path.

Keep schemaVersion 3.

Distinct defaults:

~~~text
brand-new post-M05 config:
  grep.enabled = true
  grep default policy = allow

existing config missing grep:
  grep.enabled = false
  grep default policy = allow
~~~

Normal load atomically persists additive v3 shape normalization while preserving configRevision.

Read-only inspection normalizes in memory only.

v1/v2 migration preserves old Tool exposure by using grep disabled.

ToolRegistry registers grep after glob as Low-risk WorkspaceRead.

Policy matcher whitelist accepts grep.

Exit: protocol/config/registry/policy unit tests PASS before grep is exposed through MCP.

---

# 4. Phase B — Grep Engine

Create runtime/src/tools/grep.rs.

Implement:

- typed request/result;
- frozen validation limits;
- literal matcher;
- Rust regex with compiled-program cap;
- globset include/exclude;
- sorted depth-first traversal;
- UTF-8/BOM handling;
- binary/invalid UTF-8/oversize/unreadable diagnostics;
- bounded preview;
- deterministic result/truncation;
- 5-second timeout;
- cancellation checks.

Windows security:

- never traverse reparse directories;
- reject/skip candidate reparse entries;
- open candidate;
- resolve opened-handle final path;
- re-check canonical Workspace containment before reading;
- expose a test-only race hook for TOCTOU fixtures.

Filesystem scan runs off Tokio core worker.

Exit: executor-level correctness/resource/security tests PASS.

---

# 5. Phase C — Runtime Path and Evidence

Add grep to ToolExecutor/ExecutionPipeline using existing WorkspaceRead authority.

Create runtime/src/mcp/grep.rs.

MCP adapter:

- parses exact schema;
- rejects unknown fields;
- builds GrepRequest;
- uses McpExecutionContext;
- attaches exact correlation metadata.

Policy/Approval tests prove:

- disabled no scan;
- DENY no scan;
- ASK pending before scan;
- allow_once resumes same request;
- deny no scan.

Add safe grep argument/result evidence.

No raw query/filter/match text in durable evidence.

M03 physical schema stays v1.

Exit: real Runtime MCP + Activity + M03 History tests PASS.

---

# 6. Phase D — UI / Upgrade UX / i18n

Reuse Tools page.

Add grep description and generated ToolConfig support.

Test:

- new install: ON / ALLOW;
- upgraded old config: explicit OFF / ALLOW;
- additive migration preserves revision;
- user enable increments revision and shows Restart Required;
- restarted Runtime exposes grep.

Add en/zh-CN strings.

Ensure Activity/History presentation is safe.

Exit: frontend + Tauri config tests PASS.

---

# 7. Phase E — M04 Eval Reuse and Quality Regression

Preserve M04 baseline mode exactly:

~~~text
read_file, glob, write, edit
~~~

It must still reject grep catalog drift.

Add explicit candidate profiles, not a loosened baseline validator.

Strict candidate:

~~~text
profile: m05-grep-candidate-v1
tools: read_file, glob, grep, write, edit
dataset: unchanged m04-core-v1
runs: 24 × 3 = 72
scorer: original M04 strict scorer
~~~

Paired control:

~~~text
profile: m05-paired-control-v1
grep disabled/not exposed
catalog/schema: exact accepted M04 baseline
~~~

Supplemental set:

~~~text
m05-grep-admission-v1
4 cases × 3 = 12
reported separately
~~~

Strict comparator calls existing compare_m05.

Do not duplicate/widen threshold constants.

If Model identity differs, use paired control + existing compare_provider_drift.

Development T5 uses scripted adapters only; real provider calls remain formal E2E after user authorization.

Exit: baseline preservation, candidate profile, supplemental set and comparator mechanics PASS locally.

---

# 8. Phase F — Development Completion

Run focused M05 tests and full:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Run required milestone delivery/layout scripts.

Build fresh M05 Portable.

Development Complete response records:

- candidate SHA;
- package/runtime hash;
- grep Tool schema hash;
- config migration matrix;
- direct grep functional matrix;
- Windows reparse/TOCTOU matrix;
- evidence privacy result;
- M04 hash/baseline-preservation tests;
- scripted M05 candidate/comparator result;
- real Model regression = NOT EXECUTED unless authorized;
- real ChatGPT E2E = NOT EXECUTED unless authorized;
- formal Integration/E2E/Manual state.

No seventh milestone document.

---

# 9. Grep Execution Sequence

~~~text
validate request
→ compile filters
→ compile literal/regex matcher
→ WorkspaceGuard(scope)
→ deterministic traversal
→ candidate eligibility
→ final opened-handle containment
→ bounded file read
→ UTF-8/binary validation
→ line scan
→ bounded preview
→ append within budgets
→ continue until complete/truncated/cancelled/timeout
~~~

Do not read the whole Workspace into memory.

At most one eligible <=8MiB file buffer plus bounded traversal/result state should be held per request.

---

# 10. Concurrency / Races

Cover:

- grep config changes while old Runtime generation runs;
- scope/candidate swapped after enumeration;
- candidate swapped outside before open;
- Runtime stop during search;
- Approval resolved while waiting;
- M03 writer degraded during grep;
- concurrent independent grep requests within existing Runtime request limits.

Rules:

- old generation keeps old Tool profile;
- opened-handle final path is authoritative for containment;
- cancellation stops scan;
- evidence failure does not change Policy;
- each grep remains one canonical request.

---

# 11. Error / Recovery Plan

Invalid regex/filter: fail before traversal.

Invalid scope: fail before scan.

Per-file binary/unreadable/oversize: bounded diagnostic and continue.

Timeout: SEARCH_TIMEOUT; no successful partial result.

Cancellation: terminal cancel; no success partial result.

Evidence failure: Runtime result remains authoritative, M03 reports degradation.

Config migration failure: preserve original config and fail through existing config flow; never silently enable grep.

---

# 12. Security Review

Before T6 prove:

- no shell/Bash/external search process;
- MCP cannot inject a trusted absolute executor path;
- traversal cannot follow outside junction/reparse;
- TOCTOU outside swap blocked before bytes read;
- DENY/ASK occurs before executor;
- old History Approval cannot authorize grep;
- raw query/match absent from durable DB;
- output bounds enforced;
- no unrestricted match-content production logging.

---

# 13. Eval Reproducibility

Formal M05 Eval records:

- accepted M04 baseline identity/result;
- M05 candidate commit/runtime hash;
- unchanged M04 dataset/scorer/system prompt hashes;
- M05 Tool schema/profile hash;
- adapter/requested/resolved Model;
- candidate run identity;
- compare_m05 result;
- paired control if needed;
- supplemental set/hash/result.

No historical M04 baseline is synthesized from source-only values.

---

# 14. Rollback

M05 does not change AppConfig schemaVersion or M03 DB schema.

A pre-M05 binary may reject post-M05 config containing grep because its strict ToolConfig did not know the field.

Therefore delivery/rollback evidence must explicitly test/document config compatibility.

M05 must not claim transparent binary downgrade unless a real pre-M05 candidate successfully parses the post-M05 config.

If rollback requires restoring the prior config copy, use existing safe config recovery flow; never silently delete fields from a live config.

---

# 15. Development Completion Gate

Before formal Integration:

- T1–T6 complete;
- task-local tests PASS;
- grep schema/limits frozen in code;
- no shell/external grep;
- Windows reparse + TOCTOU development tests PASS;
- config new/upgrade behavior PASS;
- Policy/Approval path PASS;
- M03 grep evidence privacy PASS;
- M04 baseline mode unchanged;
- M05 candidate/control modes deterministic locally;
- compare_m05 thresholds unchanged;
- full workspace/frontend regression PASS;
- fresh M05 Portable delivered;
- real paid Eval and real ChatGPT remain truthfully NOT EXECUTED unless authorized.

Development Complete is not M05 ACCEPTED / FROZEN.
