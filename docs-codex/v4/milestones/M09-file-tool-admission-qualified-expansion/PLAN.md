# M09 PLAN — File Tool Admission and Qualified Expansion

> Input: SPEC.md
> Status: DESIGN READY / IMPLEMENTATION BLOCKED UNTIL M04+M05 ACCEPTED / FROZEN AND M09 VALIDATION DOCS ARE FROZEN
> Scope: P1 F11 only
> Primary owners: Runtime ToolRegistry / WorkspaceGuard / ExecutionPipeline / Eval Runner / existing Tools UI
> Admission rule: evidence decides scope; production implementation follows only after final ADMIT verdicts
> External rule: real controlled Model, real External ChatGPT, destructive filesystem E2E on non-disposable data, and active-instance restart require explicit execution-time authorization.

---

# 1. Delivery Strategy

M09 is intentionally staged so Codex cannot implement six Tools first and justify them later.

~~~text
M04 + M05 ACCEPTED / FROZEN
        ↓
M09 six docs frozen
        ↓
T1 Admission infrastructure + datasets + structural matrix
        ↓
Real controlled Model authorization
        ↓
T2 Candidate-specific admission + cumulative quality verdicts
        ↓
Freeze decision-set-v1
        ↓
T3 Shared file-tool security/executor foundation for ADMIT set
        ↓
T4 Implement simple/read candidates from ADMIT set
        ↓
T5 Implement dual-path/destructive candidates from ADMIT set
        ↓
T6 Config/UI/evidence/regression/package
        ↓
Formal Integration → E2E → Manual
        ↓
M09 — ACCEPTED / FROZEN
~~~

No production Tool is added before its ADMIT verdict.

---

# 2. Core Implementation Principles

1. Preserve the accepted M05 five-tool quality control exactly.
2. Reuse the existing Eval Runner and deterministic scorers.
3. Add versioned M09 profiles/fixtures rather than modifying M04/M05 fixtures.
4. Use one machine-readable decision set as the implementation scope authority.
5. REJECT/DEFER candidates remain absent from production surfaces.
6. All ADMIT Tools use native Rust executors and the existing Runtime security pipeline.
7. No shell, PowerShell, cmd.exe, robocopy, or external utility fallback.
8. Mutation Tools default ASK.
9. Upgrades do not silently expose new filesystem capabilities.
10. No overwrite in M09 v1.
11. No recursive delete in M09 v1.
12. move is same-volume atomic only; no copy+delete fallback.
13. copy is regular-file-only and binary-safe.
14. M08 observability, when present, is reused rather than extended with a second path.
15. Development Complete does not imply Integration/E2E/Manual or ACCEPTED/FROZEN.

---

# 3. Expected Landing Zones

Use existing modules:

~~~text
protocol/src/v1/
protocol/fixtures/

runtime/src/evaluation/
runtime/src/tools/
runtime/src/execution/
runtime/src/workspace/
runtime/src/audit/
runtime/src/evidence/
runtime/tests/

fixtures/v4/m09-admission/

ui/src-tauri/
ui/src/pages/tools/
ui/src/services/harness/
ui/src/i18n/
~~~

No new top-level directory.

Possible internal modules such as runtime/src/tools/copy.rs are allowed because runtime is an existing formal product module.

---

# 4. Phase A — Admission Infrastructure

## 4.1 Add M09 Eval profiles

Extend the existing EvalProfile model with:

- m09-five-tool-control-v1;
- six single-candidate profiles;
- m09-final-candidate-v1.

Each single-candidate profile must validate exact catalog equality.

No bash in any M09 controlled quality profile.

## 4.2 Add admission dataset

Create fixtures/v4/m09-admission with:

- dataset manifest;
- system prompt reference;
- 36 cases total;
- candidate-specific isolated filesystem fixtures;
- expected final-state ground truth;
- selection ground truth;
- security expectations.

Each candidate receives:

- 3 positive cases;
- 3 negative sentinels;
- 3 repetitions.

## 4.3 Candidate positive scenarios

Freeze cases that exercise unique semantics.

Examples by category:

- mkdir: empty directory creation, nested directory when recursive true, refusal on collision;
- list_directory: direct child inventory/metadata without content search;
- copy: binary file duplicate, destination conflict, source unchanged;
- move: relocate file/directory with source removal and destination creation;
- rename: same-parent rename with one-component name;
- delete: file removal and empty-directory removal with non-empty refusal.

Positive prompts do not name the Tool.

## 4.4 Negative sentinels

Use tasks where the existing five-tool control is clearly preferred:

- content search → grep;
- pattern discovery → glob;
- exact read → read_file;
- file content creation → write;
- in-place text change → edit.

Each candidate must prove it does not attract these tasks unnecessarily.

## 4.5 Structural matrix

Implement a deterministic matrix per candidate:

- user value;
- unique capability;
- overlap with current five tools;
- schema ambiguity;
- required capability;
- default policy;
- destructive nature;
- dual-path nature;
- overwrite semantics;
- cross-volume semantics;
- intermediate state;
- recovery boundary;
- Workspace/reparse/TOCTOU feasibility.

This matrix can deterministically REJECT a candidate before paid Model execution.

Exit: admission mechanics are ready without changing production ToolRegistry.

---

# 5. Phase B — Real Controlled Admission

## 5.1 Authorization boundary

Before any paid/networked run:

- require explicit user authorization;
- pin adapter/model;
- record resolved identity;
- validate accepted M05 handoff.

If authorization or accepted baseline is missing:

- scripted mechanics may continue;
- production Tool implementation remains blocked.

## 5.2 Run candidate-specific profiles

For each structurally eligible candidate, execute the 18-run frozen gate.

Compute:

- scoreable;
- per-case completion;
- positive selection;
- positive argument validity;
- positive final completion;
- negative final completion;
- negative candidate misuse;
- security count.

Apply SPEC thresholds exactly.

## 5.3 Produce provisional verdicts

- structural redundancy/safety failure → REJECT;
- valid candidate-specific quality failure → DEFER;
- blocked/inconclusive Model evidence → DEFER/BLOCKED according to evidence state;
- passing candidate → provisional ADMIT.

## 5.4 Run cumulative 72 regression

Build m09-final-candidate-v1 from five-tool control plus provisional ADMIT candidates.

Run unchanged m04-core-v1 24x3.

Use accepted M05 comparator unchanged.

## 5.5 Optional one isolation round

If cumulative FAIL, apply the exact ranking/removal algorithm from SPEC and rerun once.

No more than one isolation rerun.

## 5.6 Final decision set

Write decision-set-v1.json containing all six final verdicts and hashes.

After this point, production implementation scope is fixed for this M09 candidate.

Do not edit SPEC/PLAN/TASKS to reflect the resulting subset.

Exit: all six candidates have auditable final decisions before production expansion.

---

# 6. Phase C — Shared File-tool Security Foundation

Implement only abstractions needed by the actual ADMIT set.

## 6.1 Path validation

Reuse WorkspaceGuard.

Add narrowly-scoped helpers where required for:

- destination parent verification;
- opened source final-path validation;
- reparse rejection;
- dual-path pair validation;
- post-approval revalidation.

Do not build a second WorkspaceGuard.

## 6.2 Safe target facts

Create bounded internal target facts:

- logical relative path;
- entry kind;
- size where safe;
- parent logical scope;
- target identity token or equivalent internal revalidation fact.

Never persist physical path.

## 6.3 Mutation authorization order

For every mutation:

~~~text
parse
→ registry/capability
→ initial Workspace validation
→ Policy
→ ASK Approval
→ post-approval revalidation
→ executor
→ Audit
~~~

No mutation before Approval allow.

## 6.4 Error normalization

Map OS/filesystem errors to typed product errors.

Raw std::io::Error strings must not cross MCP/Activity/History boundaries.

## 6.5 Evidence

Add typed sanitized argument/result variants only for ADMIT Tools.

Exit: common security semantics are testable before individual executors land.

---

# 7. Phase D — Simple and Read Candidate Implementation

This phase executes only for candidates whose decision set is ADMIT.

## 7.1 mkdir

Implement:

- relative path;
- recursive boolean;
- no existing-target success;
- Workspace root and reparse safety;
- ASK;
- truthful partial-creation result if recursive operation fails.

Tests:

- direct create;
- recursive create;
- existing dir/file;
- escape/reparse;
- DENY/no mutation;
- target-change after Approval.

## 7.2 list_directory

Implement:

- non-recursive only;
- deterministic child ordering;
- maxEntries 200 default / 1000 max;
- logical names;
- kind + optional size;
- reparse identified but not followed;
- ALLOW default;
- result truncation.

Tests:

- files/directories/other;
- empty directory;
- deterministic ordering;
- bounded truncation;
- hidden/reparse handling without traversal;
- Workspace escape;
- Policy override.

Exit: admitted low-complexity candidates behave exactly as evaluated.

---

# 8. Phase E — Dual-path and Destructive Candidate Implementation

Only final ADMIT candidates are implemented.

## 8.1 copy

Implement regular-file binary copy with:

- source guard;
- destination parent guard;
- source <=64MiB;
- destination absent;
- temp file in guarded destination parent;
- byte streaming;
- flush/sync as current atomic-file conventions require;
- atomic final commit;
- temp cleanup;
- source unchanged.

No directory copy.

## 8.2 move

Implement:

- file or ordinary directory;
- destination absent;
- same-volume atomic rename only;
- root/self/descendant/reparse rejection;
- post-approval state revalidation;
- no copy-delete fallback.

## 8.3 rename

Implement:

- same-parent only;
- one-component newName;
- destination absent;
- file/directory;
- atomic rename;
- no root/reparse.

## 8.4 delete

Implement:

- regular file or empty directory;
- no recursion;
- root/reparse rejection;
- ASK;
- explicit irreversible evidence;
- no recycle-bin claim.

## 8.5 TOCTOU tests

For every ADMIT dual-path/destructive Tool add deterministic test seams for:

- source swap;
- destination appearance;
- destination parent reparse;
- approval wait target mutation.

Expected:

~~~text
TARGET_CHANGED
0 unauthorized mutation
~~~

Exit: admitted mutation Tools are safe under Windows-first race and approval boundaries.

---

# 9. Phase F — Registry, Config, Policy, MCP and UI

## 9.1 ToolRegistry

Add only actual ADMIT Tools.

Keep deterministic registry order:

~~~text
existing tools first, then admitted M09 tools in fixed candidate order
mkdir, list_directory, copy, move, rename, delete
~~~

Skip non-admitted names without placeholders.

## 9.2 AppConfig schema 3 additive evolution

Do not bump AppConfig schema.

For each admitted Tool:

- new install enabled true;
- upgraded config missing field defaults false;
- policy default from SPEC;
- explicit user change increments restart-affecting revision as current Tool config semantics require;
- normalization alone does not.

No config fields for REJECT/DEFER candidates.

## 9.3 Policy matcher

Whitelist only admitted names.

Mutation defaults ASK; list_directory ALLOW.

## 9.4 MCP schema and dispatch

Expose exact evaluated schema.

Unknown fields rejected.

No candidate-specific behavior drift between Eval and production.

## 9.5 Tools UI

Show only admitted Tools.

Add en/zh-CN descriptions and risk boundaries.

Do not show an Admission Matrix in the normal product Tools list.

Exit: production surfaces exactly equal decision-set-v1.

---

# 10. Phase G — Eval Regression and Existing-tool Protection

## 10.1 Preserve frozen M04/M05 assets

Hash-check before and after development:

- m04-core-v1 dataset;
- M04 scorer/system prompt;
- M05 comparator;
- accepted M05 control schemas.

Any drift is a development failure.

## 10.2 Re-run final cumulative profile

After production implementation, re-run the exact m09-final-candidate-v1 schema hash and ensure it matches the admission-time schema set.

If product schema differs from evaluated schema:

~~~text
BLOCKED
~~~

No grandfathering.

## 10.3 Existing tool regression

Run direct behavior regressions for:

- read_file;
- glob;
- grep;
- write;
- edit.

No M09 candidate may alter old argument schemas or security behavior.

## 10.4 M08 compatibility smoke

If M08 is in the integrated source:

- admitted Tool request appears in Activity;
- History exact identity works;
- Metrics classifies the Tool generically;
- Trace stages are produced.

Do not make M09 acceptance depend on M08 availability.

Exit: expansion does not degrade existing Agent quality or evidence identity.

---

# 11. Phase H — Packaging and Handoff

## 11.1 Full development checks

Minimum:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

Also run:

- generated protocol/TS schema verification;
- M09 scripted admission mechanics;
- authorized real admission run;
- final cumulative 72 regression;
- repository layout;
- milestone layout;
- fresh Portable build.

## 11.2 Package truth

Portable tools/list must exactly equal:

~~~text
base product catalog + final ADMIT candidates
~~~

No REJECT/DEFER candidate may leak through config or schema generation.

## 11.3 Development handoff report

Report in conversation:

~~~text
M09 Development Complete:
candidate SHA:
decision-set hash:
mkdir verdict:
list_directory verdict:
copy verdict:
move verdict:
rename verdict:
delete verdict:
admitted production catalog:
candidate admission run identity:
final cumulative 72 result:
provider drift result:
security violations:
outside-workspace mutations:
AppConfig schema:
Control Protocol revision:
Portable path/hash:
existing five-tool regression:
M08 compatibility smoke:
Formal Integration: NOT EXECUTED
Formal E2E: NOT EXECUTED
Real External ChatGPT: NOT EXECUTED
Manual: NOT EXECUTED
~~~

Do not mark M09 ACCEPTED / FROZEN.

---

# 12. Rollback Strategy

M09 does not require an AppConfig schema bump.

If the code candidate is rolled back:

- stop Runtime cleanly;
- keep user config;
- older code ignores no non-admitted fields because only admitted fields were added additively;
- if older strict deserialization cannot accept newer admitted fields, use existing config migration/normalization recovery path only after preserving the original;
- do not silently rewrite user policy;
- filesystem mutations already completed by an admitted Tool are user-visible state and are not automatically reversed.

For move/rename/delete there is no general rollback of successful filesystem action.

This is why Approval and final-state Manual acceptance are mandatory.

---

# 13. Defect Rules

If a candidate cannot satisfy frozen semantics:

- do not weaken Workspace/Approval;
- do not add shell fallback;
- do not change overwrite from false to true;
- do not add recursive delete;
- do not silently add cross-volume move fallback;
- do not change schema after Eval and reuse the same ADMIT result.

Instead:

- mark candidate DEFER/REJECT according to frozen verdict rules;
- leave it absent from product surfaces;
- continue with other candidates.

---

# 14. Development Completion Gate

M09 Development Complete requires:

- T1–T6 complete;
- real admission evidence authorized and valid;
- six verdicts finalized;
- only ADMIT Tools implemented;
- all admitted Tool unit/security tests PASS;
- final 72-run cumulative regression PASS;
- provider drift resolved;
- existing five-tool regression PASS;
- AppConfig v3 compatibility PASS;
- protocol generation PASS;
- fresh Portable candidate;
- no REJECT/DEFER exposure;
- formal Integration/E2E/Manual truthfully NOT EXECUTED unless run.

Development Complete is not M09 ACCEPTED / FROZEN.
