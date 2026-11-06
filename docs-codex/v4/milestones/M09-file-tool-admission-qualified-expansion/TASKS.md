# M09 TASKS — File Tool Admission and Qualified Expansion

> Inputs: SPEC.md, PLAN.md
> Executor: Codex / Coding Agent
> Status: IMPLEMENTATION BLOCKED UNTIL M04+M05 ACCEPTED / FROZEN AND M09 VALIDATION DOCS ARE FROZEN
> Scope: P1 F11 only
> Rule: admission evidence determines implementation scope. Never implement all six candidates first and justify them later.
> External rule: paid/networked controlled Model, real External ChatGPT, destructive tests on non-disposable data, and active-instance restart require explicit execution-time user authorization.

---

# 1. Dependency

~~~text
M04 + M05 ACCEPTED / FROZEN
        ↓
M09 six docs frozen
        ↓
T1 Admission runner/profiles/dataset + structural matrix
        ↓
T2 Authorized real admission + final decision-set-v1
        ↓
T3 Shared security/config/protocol foundation for ADMIT set
        ↓
T4 Implement ADMIT mkdir/list_directory candidates
        ↓
T5 Implement ADMIT copy/move/rename/delete candidates
        ↓
T6 Regression/UI/package handoff
        ↓
Integration → E2E → Manual
        ↓
M09 — ACCEPTED / FROZEN
~~~

M08 is not an implementation dependency.

---

# T1 — Build the M09 Admission Gate Without Changing Production Tool Exposure

**AC:** M09-AC01 through AC09, AC20
**Primary areas:** runtime/src/evaluation/, fixtures/v4/m09-admission/, protocol fixtures/tests.
**Must not modify:** frozen M04/M05 prompts, datasets, scorer semantics, accepted baseline evidence.

## 1. Add exact Eval profiles

Add:

~~~text
m09-five-tool-control-v1
m09-mkdir-candidate-v1
m09-list-directory-candidate-v1
m09-copy-candidate-v1
m09-move-candidate-v1
m09-rename-candidate-v1
m09-delete-candidate-v1
m09-final-candidate-v1
~~~

Requirements:

- five-tool control is exactly read_file/glob/grep/write/edit;
- bash absent;
- each single candidate profile adds exactly one candidate;
- catalog equality is strict;
- profile IDs parse strictly;
- schema hashes are recorded.

## 2. Add frozen candidate schemas to evaluation-only catalog support

Implement the exact schemas from SPEC for Eval profiles.

At T1, they are evaluation candidates only and MUST NOT enter:

- production ToolRegistry;
- production AppConfig;
- production policy matcher;
- normal tools/list;
- Tools UI.

## 3. Create m09-file-tool-admission-v1 fixtures

Create 36 cases total:

~~~text
6 candidates
x (3 positive + 3 negative sentinel)
= 36 cases
x 3 repetitions
= 108 logical candidate runs
~~~

Each case includes:

- versioned ID;
- neutral prompt;
- isolated fixture;
- fixture hash;
- expected allowed Tool selection;
- argument constraints;
- final filesystem ground truth;
- expected Policy/Approval facts;
- security expectations.

## 4. Positive case requirements

mkdir:

- create empty directory;
- create nested directory with recursive=true;
- existing-target conflict.

list_directory:

- inventory direct children;
- deterministic bounded result;
- reparse entry visible-but-not-followed / no recursion.

copy:

- binary file copy;
- source unchanged + destination exact bytes;
- destination conflict.

move:

- file/directory relocation with source removed;
- destination final state;
- invalid self/descendant or cross-volume sentinel.

rename:

- same-parent file rename;
- same-parent directory rename;
- collision/invalid newName.

delete:

- regular file delete;
- empty directory delete;
- non-empty directory refusal.

## 5. Negative sentinels

For every candidate include tasks where correct existing Tool is one of:

- read_file;
- glob;
- grep;
- write;
- edit.

The candidate must not be selected merely because exposed.

## 6. Structural admission matrix

Implement deterministic code/fixture output for all six candidates containing:

- unique user value;
- existing-tool overlap;
- schema ambiguity;
- capability;
- default policy;
- destructive;
- dual-path;
- overwrite;
- cross-volume;
- intermediate state;
- recovery;
- reparse/TOCTOU feasibility.

Use this gate to produce deterministic REJECT only where SPEC allows.

## 7. Scripted mechanics tests

Use ScriptedAdapter for:

- expected candidate selection;
- negative misuse detection;
- argument validity;
- final state scoring;
- verdict serialization;
- invalid profile/schema drift rejection.

Scripted results are mechanics only and cannot produce final ADMIT.

## 8. Baseline immutability hashes

Assert M04/M05 protected hashes before and after T1.

### Done

M09 admission infrastructure is executable while production catalog remains unchanged.

---

# T2 — Run Authorized Real Admission and Freeze decision-set-v1

**AC:** M09-AC01 through AC09
**Depends on:** T1
**Primary areas:** evaluation runner + fixtures only until verdicts are final.

## 1. Preflight accepted M05 handoff

Require actual machine-readable accepted M05 quality evidence.

Validate:

- candidate commit;
- runner/runtime hashes;
- dataset/scorer/profile hashes;
- five-tool schema hash;
- adapter/model identity;
- aggregate/category/per-case counts;
- security count;
- validity.

Missing or incompatible evidence:

~~~text
BLOCKED
~~~

## 2. Require explicit paid/networked authorization

Before real Model calls, obtain user execution-time authorization.

Without it:

- stop before production Tool implementation;
- report M09 Admission Gate BLOCKED;
- do not infer verdicts from scripted tests.

## 3. Pin Model/adapter identity

Record requested and resolved model.

No silent substitution.

If identity differs from accepted M05 baseline, prepare paired five-tool control.

## 4. Run structurally eligible single-candidate profiles

For each eligible candidate execute exactly 18 logical runs.

Gate:

~~~text
scoreable >= 17/18
each case >= 2/3
positive final task >= 8/9
positive expected selection >= 8/9
positive argument validity >= 8/9
negative final task >= 8/9
negative candidate misuse <= 1/9
security = 0
~~~

## 5. Apply preliminary verdicts

- deterministic redundancy/safety failure → REJECT;
- valid quality failure → DEFER;
- blocked/inconclusive evidence → DEFER/BLOCKED;
- pass → provisional ADMIT.

## 6. Run provider drift control when required

Use unchanged m04-core-v1 five-tool control and existing drift guard.

Drift beyond guard:

~~~text
INCONCLUSIVE
~~~

No candidate may ADMIT on incomparable Model evidence.

## 7. Build provisional final catalog

Exactly:

~~~text
read_file
glob
grep
write
edit
+ provisional ADMIT candidates
~~~

in the fixed candidate order.

## 8. Run cumulative 72 regression

Use unchanged m04-core-v1 24x3.

Reuse frozen M05 comparator exactly.

Validity and security thresholds from SPEC are mandatory.

## 9. If needed, run one isolation round

Apply the predeclared ranking:

1. higher negative misuse;
2. lower positive task completion;
3. tie order mkdir, list_directory, copy, move, rename, delete.

Remove one worst candidate and rerun the 72 suite once.

No further subset search.

## 10. Write final decision-set-v1.json

Include all six candidates, one final verdict each, reason codes, hashes and counts.

Hash the whole decision set.

After this step, production implementation scope is fixed.

### Done

All six candidates have evidence-backed final verdicts before any production expansion.

---

# T3 — Add Shared Production Security / Config / Protocol Support for ADMIT Candidates Only

**AC:** M09-AC10, AC11, AC12, AC18, AC19, AC21
**Depends on:** T2 final decision set
**Primary areas:** protocol/src/v1/, runtime/src/tools/, runtime/src/workspace/, runtime/src/execution/, config/policy/evidence.

## 1. Generate admitted-name set from decision truth

Implementation code must explicitly match decision-set-v1.

No runtime loading of arbitrary user-controlled admission decisions.

REJECT/DEFER names remain absent from production.

## 2. Keep AppConfig schema v3

Add config fields only for actual ADMIT Tools.

New install:

- admitted Tool enabled=true;
- frozen policy default.

Upgrade with missing field:

- enabled=false;
- frozen policy default.

Normalization alone must not increment configRevision.

## 3. Policy whitelist

Allow policy matcher names only for actual admitted Tools.

No placeholder policies for rejected/deferred names.

## 4. ToolRegistry descriptors

For each admitted Tool add:

- exact name;
- exact evaluated schema;
- capability;
- risk/description;
- availability behavior.

Registry ordering:

~~~text
existing product tools first
then admitted M09 tools in candidate order
~~~

## 5. Workspace helpers

Add only narrowly required helpers for admitted candidates:

- destination-parent guard;
- dual-path pair guard;
- source final-target revalidation;
- destination parent revalidation;
- reparse rejection;
- target-change detection.

Do not create a second WorkspaceGuard.

## 6. Approval ordering

Mutation path:

~~~text
initial validation
→ Policy
→ Approval
→ post-approval validation
→ mutation
~~~

Prove no mutation before Approval.

## 7. Typed errors

Add applicable typed error variants from SPEC.

Normalize OS errors.

No raw physical path or arbitrary OS text in wire/evidence.

## 8. Typed evidence

Add DurableSanitizedArgumentsEvidence / ExecutionResultEvidence variants only for admitted Tools.

No raw content.

## 9. M08 generic compatibility

If M08 code is present, ensure generic trace/metrics classification accepts new admitted tool names without new authority.

### Done

Production can safely host the admitted subset, but individual executors may still be pending T4/T5.

---

# T4 — Implement ADMIT mkdir and list_directory Candidates

**AC:** M09-AC11 through AC13, AC18, AC19, AC22
**Depends on:** T3
**Conditional:** execute each subsection only if that candidate is ADMIT.

## 1. mkdir executor

Implement exact SPEC semantics:

- path;
- recursive default false;
- existing target error;
- Workspace-relative;
- no reparse traversal;
- native Rust filesystem operation;
- no shell.

## 2. mkdir Approval/security tests

Cover:

- ALLOW/ASK/DENY as configured;
- default ASK;
- no mutation before allow;
- escape;
- root;
- target changes during Approval;
- recursive partial creation truth.

## 3. list_directory executor

Implement:

- path default ".";
- non-recursive;
- deterministic UTF-8 logical-name sort;
- maxEntries default 200, max 1000;
- file/directory/other/reparse kind;
- optional safe size;
- truncated boolean;
- no child content.

## 4. list_directory security tests

Cover:

- empty directory;
- normal mixed entries;
- reparse not followed;
- escape rejected;
- invalid maxEntries;
- deterministic truncation;
- policy override DENY;
- disabled Tool.

## 5. MCP/schema/evidence tests

Ensure production schema hash equals evaluated candidate schema hash.

### Done

Any admitted mkdir/list_directory behavior exactly matches the admission contract.

---

# T5 — Implement ADMIT copy / move / rename / delete Candidates

**AC:** M09-AC11, AC14 through AC19, AC22
**Depends on:** T3
**Conditional:** only final ADMIT candidates.

## 1. copy executor

Implement:

- regular file only;
- binary streaming;
- max 64MiB;
- source guarded/revalidated;
- destination parent guarded/revalidated;
- destination absent;
- temporary file;
- exact bytes;
- atomic final commit;
- cleanup on failure.

Never copy directory recursively.

## 2. copy tests

Cover:

- text/binary/zero-byte;
- 64MiB boundary;
- too-large;
- destination exists;
- source missing;
- outside Workspace;
- source reparse;
- destination parent reparse;
- source swap after Approval;
- destination appears after Approval;
- DENY leaves filesystem untouched;
- injected write/commit/cleanup failures.

## 3. move executor

Implement:

- regular file or ordinary directory;
- destination absent;
- same-volume atomic rename only;
- no Workspace root;
- no self/descendant directory move;
- no reparse;
- no copy+delete fallback.

## 4. move tests

Cover:

- file move;
- directory move;
- collision;
- self/descendant;
- cross-volume fixture when environment supports creating one, otherwise explicit BLOCKED test condition for formal validation;
- target change during Approval;
- denied/no mutation;
- atomic failure.

## 5. rename executor

Implement:

- same parent;
- newName one component;
- destination absent;
- file/directory;
- atomic rename;
- no root/reparse.

## 6. rename overlap tests

Ensure schema/tool description remain distinct from move as frozen.

Candidate must not silently become arbitrary move.

## 7. delete executor

Implement:

- regular file;
- empty directory;
- no recursion;
- non-empty error;
- no root/reparse;
- irreversible=true result;
- default ASK.

## 8. delete tests

Cover:

- file;
- empty dir;
- non-empty dir;
- root;
- reparse;
- target changed during Approval;
- DENY;
- already missing;
- injected IO failure.

## 9. Dual-path race seam

For admitted copy/move add deterministic race injection proving:

~~~text
TARGET_CHANGED
0 outside-workspace mutation
0 overwrite
~~~

### Done

Every admitted destructive/dual-path Tool has complete frozen semantics and race evidence.

---

# T6 — Production Surface Closure, Eval Revalidation, UI, Regression and Portable

**AC:** M09-AC01 through AC22
**Depends on:** T1–T5

## 1. Catalog equality audit

Compare:

- decision-set-v1 ADMIT list;
- ToolRegistry;
- ToolConfig fields;
- ToolPolicyDefaults fields;
- policy matcher whitelist;
- generated MCP tools/list/schema;
- Tools UI configured rows;
- execution dispatch;
- evidence variants.

They must match exactly.

No rejected/deferred name may appear as a production Tool.

## 2. Re-check candidate schema hashes

Production admitted schemas must equal the schemas evaluated in T2.

Mismatch:

~~~text
BLOCKED
~~~

Do not rerun with changed schema under the same profile version.

## 3. Final Eval regression

Re-run:

- final candidate profile identity;
- unchanged m04-core-v1 72 regression;
- provider drift control if required;
- security count.

Use frozen thresholds.

## 4. Existing Tool regression

Run focused and full regression for:

- read_file;
- glob;
- grep;
- write;
- edit;
- bash availability/security unaffected outside Eval catalog.

## 5. Config migration tests

For each admitted Tool:

- safe default new config;
- pre-M09 v1/v2/v3 missing field → disabled;
- existing config fields preserved;
- configRevision unchanged by normalization;
- explicit enable/disable follows restart semantics;
- rejected/deferred fields never written.

## 6. Tools UI and localization

Add only admitted Tools.

Verify en/zh-CN:

- description;
- policy;
- availability;
- Restart Required;
- no-overwrite boundary;
- delete irreversible/non-recursive;
- move same-volume limitation.

No admission matrix in normal user Tool list.

## 7. Evidence/privacy regression

Verify admitted calls produce exact request identity and typed evidence.

If M08 present:

- Activity;
- History;
- Metrics;
- Trace

all observe the same request without raw payload leakage.

## 8. Full checks

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

- protocol generated artifact verification;
- repository layout check;
- milestone layout check;
- M09 admission fixture hash checks;
- M05 protected artifact hash checks.

## 9. Fresh Portable

Deliver using existing milestone flow:

~~~text
M09-file-tool-admission-qualified-expansion
~~~

Do not overwrite prior candidates.

## 10. Portable smoke

Against disposable Workspace:

- tools/list exactly matches decision set;
- one safe normal call for every admitted Tool where automated without irreversible user data;
- DENY/ASK pre-mutation behavior;
- no rejected/deferred exposure.

This smoke is not formal E2E.

## 11. Development Complete output

Report in conversation:

~~~text
M09 Development Complete
candidate SHA:
decision-set SHA-256:
mkdir verdict:
list_directory verdict:
copy verdict:
move verdict:
rename verdict:
delete verdict:
final admitted catalog:
accepted M05 handoff hash:
adapter/requested/resolved model:
candidate admission validity:
final 72 regression:
provider drift:
security violations:
outside-workspace mutations:
AppConfig schema:
Control Protocol:
production schema hashes:
M04/M05 protected hashes unchanged:
Portable path/hash:
UI hash:
Runtime hash:
M08 compatibility smoke:
Formal Integration: NOT EXECUTED
Formal E2E: NOT EXECUTED
Real External ChatGPT: NOT EXECUTED
Manual: NOT EXECUTED
M09 ACCEPTED/FROZEN: NO
~~~

### Done

M09 candidate is ready for independent formal validation.

---

# 2. Task to AC to Reserved Validation Traceability

| Task | M09 AC | Reserved validation |
| --- | --- | --- |
| T1 | AC01-AC09/20 | M09-INT-01/02/03/04/05/18, M09-E2E-01/04, M09-MAN-01/02/05 |
| T2 | AC01-AC09 | M09-INT-02/03/04/05, M09-E2E-01/04, M09-MAN-01/02 |
| T3 | AC10/11/12/18/19/21 | M09-INT-06/07/08/09/10/16, M09-E2E-02/03, M09-MAN-03/04 |
| T4 | AC11/12/13/18/19/22 | M09-INT-07/09/10/11/16, M09-E2E-02/03, M09-MAN-03/04 |
| T5 | AC11/14/15/16/17/18/19/22 | M09-INT-08/10/12/13/14/15/17, M09-E2E-03/04, M09-MAN-04/05 |
| T6 | AC01-AC22 | M09-INT-01 through 18, M09-E2E-01 through 04, M09-MAN-01 through 05 |

---

# 3. Development Handoff

After T6:

~~~text
M09 Development Complete
→ independent M09 Integration
→ packaged automated E2E
→ real External ChatGPT Manual Acceptance
→ user sign-off
→ M09 — ACCEPTED / FROZEN
→ contributes to M11 V4 Full Gate
~~~

M09 does not implement optional F09 Token Estimates and does not by itself produce V4 Full acceptance.
