# M09 SPEC — File Tool Admission and Qualified Expansion

> Layout and validation lifecycle follow [AGENTS.md](../../../../AGENTS.md). This start step creates only SPEC / PLAN / TASKS; Integration / E2E / Manual plans are added later, keeping the final six-file milestone layout.
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M04+M05 ACCEPTED / FROZEN AND M09 SIX DOCS ARE FROZEN
> Milestone: M09 — File Tool Admission and Qualified Expansion
> Scope: P1 F11 only
> Roadmap dependency: M04 + M05; M08 is not a dependency
> Canonical ACs: F11-AC01 / F11-AC02 / F11-AC03, preserving F12-AC01 / F12-AC02 / F12-AC03 / F12-AC04
> Current main observed before this document: 670a46aa12b47f6c6836329512af1188749a554b
> Current production catalog: read_file / glob / grep / write / edit / bash
> Current controlled M05 quality catalog: read_file / glob / grep / write / edit, with bash forbidden
> Current AppConfig schema: 3. Current Control Protocol revision: v1.
> User-supplied state: M08 Development Complete. This does not change M09 dependency.

---

# 1. Objective

M09 is an admission gate first and a tool-expansion milestone second.

It evaluates six candidates:

1. mkdir
2. list_directory
3. copy
4. move
5. rename
6. delete

Every candidate must end M09 with exactly one auditable verdict:

~~~text
ADMIT
REJECT
DEFER
~~~

Only ADMIT candidates may be added to the production ToolRegistry and delivered as real MCP Tools.

M09 does not promise that all six candidates will be implemented.

---

# 2. Canonical Product Contract

## F11-AC01

mkdir / list / copy / move / rename / delete candidates each receive an auditable ADMIT / REJECT / DEFER decision rather than being added by default.

## F11-AC02

Every ADMIT and actually delivered Tool has a clear schema, Workspace / Policy / Approval semantics, and uses the complete Runtime pipeline.

## F11-AC03

Destructive / dual-path operations define overwrite, conflict, cross-volume, intermediate-state, irreversibility and recovery boundaries, and pass real E2E plus Eval regression.

## F12 preservation

M09 must preserve:

- F12-AC01 real controllable Agent/Model, versioned dataset/ground truth, isolated Workspace and real MCP Runtime;
- F12-AC02 separate Tool selection, argument validity, runtime outcome and final task completion;
- F12-AC03 frozen baseline/candidate/sample/randomness/regression rules, with BLOCKED/INCONCLUSIVE for invalid evidence;
- F12-AC04 passive telemetry cannot masquerade as selection accuracy, unissued choices or hidden reasoning.

---

# 3. Existing Quality Baseline

The M09 control profile is the accepted M05 five-tool catalog:

~~~text
read_file
glob
grep
write
edit
~~~

bash is disabled and absent from controlled quality profiles.

M09 MUST NOT modify:

- m04-core-v1 prompts;
- M04 ground truth;
- M04 scorer semantics;
- M05 strict comparator thresholds;
- accepted M05 quality evidence;
- m05-grep-admission-v1;
- the five-tool control schemas.

M09 adds only new versioned admission profiles and fixtures.

---

# 4. Verdict Semantics

## 4.1 ADMIT

A candidate is ADMIT only when all required gates pass:

1. product-value gate;
2. existing-tool overlap gate;
3. schema-clarity gate;
4. security-model feasibility gate;
5. candidate-specific controlled Eval gate;
6. final cumulative quality regression gate;
7. zero unresolved security/privacy blocker.

ADMIT authorizes only the exact M09 v1 semantics frozen below.

## 4.2 REJECT

REJECT is reserved for deterministic product or safety reasons, including:

- material subsumption by existing or already-admitted tools;
- harmful selection ambiguity without compensating unique value;
- inability to express safe bounded semantics through current Workspace / Policy / Approval;
- dependence on forbidden shell or external filesystem utilities;
- intrinsically ambiguous schema.

Unavailable Model evidence is never a REJECT reason.

## 4.3 DEFER

DEFER means potentially useful but not safely admissible now because:

- controlled Model evidence is BLOCKED or INCONCLUSIVE;
- candidate-specific quality threshold fails;
- cumulative regression fails;
- provider drift invalidates comparison;
- implementation safety remains insufficiently verified.

DEFER candidates MUST NOT enter ToolRegistry, AppConfig, policy whitelist, MCP tools/list, UI, or dispatch.

---

# 5. Machine-readable Admission Decision Set

Implementation produces one immutable decision artifact under the existing fixtures hierarchy:

~~~text
fixtures/v4/m09-admission/decision-set-v1.json
~~~

One record per candidate contains at least:

~~~text
candidate
verdict
candidateSchemaHash
controlSchemaHash
admissionDatasetHash
modelAdapter
requestedModel
resolvedModel
positiveTaskCounts
negativeSentinelCounts
selectionCounts
argumentValidityCounts
runtimeOutcomeCounts
finalTaskCounts
securityViolationCount
overlapFinding
securityFinding
regressionFinding
reasonCode
evidenceHashes
~~~

No secrets, raw provider payloads, or hidden reasoning.

This is implementation/evaluation evidence, not a seventh milestone document.

---

# 6. Frozen Candidate Schemas

A material schema change after evaluation requires a new versioned profile and cannot reuse an earlier ADMIT result.

## 6.1 mkdir

Tool name: mkdir

Schema:

~~~text
mkdir({
  path: string,
  recursive?: boolean
})
~~~

Semantics:

- Workspace-relative directory path;
- recursive defaults false;
- existing target returns TARGET_EXISTS;
- existing file never counts as success;
- no shell fallback.

Capability: WorkspaceWrite

Default policy if ADMIT: ASK

## 6.2 list_directory

Tool name: list_directory

Schema:

~~~text
list_directory({
  path?: string,
  maxEntries?: integer
})
~~~

Semantics:

- path defaults to ".";
- one directory level only;
- deterministic UTF-8 byte ordering by logical entry name;
- bounded logical name + kind + safe scalar metadata;
- never follows symlink / junction / reparse entries;
- no recursive traversal;
- not a content-search replacement for grep.

Bounds:

~~~text
DEFAULT_MAX_ENTRIES = 200
MAX_MAX_ENTRIES = 1000
~~~

Capability: WorkspaceRead

Default policy if ADMIT: ALLOW

## 6.3 copy

Tool name: copy

Schema:

~~~text
copy({
  sourcePath: string,
  destinationPath: string
})
~~~

M09 v1 semantics:

- regular file only;
- binary-safe;
- destination must not exist;
- no overwrite mode;
- temporary destination plus atomic final commit;
- source and destination parent are guarded separately;
- source identity is revalidated immediately before reading;
- no recursive directory copy;
- no shell / PowerShell / robocopy fallback.

Capability: WorkspaceWrite

Default policy if ADMIT: ASK

Frozen bound:

~~~text
MAX_COPY_FILE_BYTES = 64 MiB
~~~

## 6.4 move

Tool name: move

Schema:

~~~text
move({
  sourcePath: string,
  destinationPath: string
})
~~~

M09 v1 semantics:

- source may be a regular file or ordinary directory tree;
- destination must not exist;
- no overwrite;
- same-volume atomic filesystem move only;
- cross-volume returns CROSS_VOLUME_UNSUPPORTED;
- no copy+delete fallback;
- Workspace root cannot move;
- directory cannot move into itself or descendant;
- no symlink / junction / reparse source or traversal.

Capability: WorkspaceWrite

Default policy if ADMIT: ASK

## 6.5 rename

Tool name: rename

Schema:

~~~text
rename({
  path: string,
  newName: string
})
~~~

M09 v1 semantics:

- same-parent rename only;
- newName is exactly one filesystem component and contains no path separators;
- destination must not exist;
- no overwrite;
- ordinary file or directory;
- Workspace root cannot be renamed;
- no symlink / junction / reparse target;
- atomic rename only.

Capability: WorkspaceWrite

Default policy if ADMIT: ASK

rename is explicitly evaluated against move for overlap and selection ambiguity. Both are not automatically admitted.

## 6.6 delete

Tool name: delete

Schema:

~~~text
delete({
  path: string
})
~~~

M09 v1 semantics:

- deletes one regular file or one empty ordinary directory only;
- recursive deletion is not supported;
- non-empty directory returns DIRECTORY_NOT_EMPTY;
- Workspace root deletion is forbidden;
- symlink / junction / reparse target is rejected;
- successful deletion is irreversible;
- no recycle-bin promise;
- no shell fallback.

Capability: WorkspaceWrite

Default policy if ADMIT: ASK

Recursive delete is outside M09 v1 and remains deferred.

---

# 7. Shared Path Contract

For every ADMIT path-bearing Tool:

- input is bounded UTF-8;
- absolute path is rejected;
- parent traversal is rejected;
- path remains Workspace-relative;
- reparse / symlink / junction traversal cannot escape Workspace;
- path normalization alone is not authorization;
- opened source and destination parent identity are revalidated as applicable immediately before mutation;
- raw physical paths are never persisted as durable evidence.

Frozen bounds:

~~~text
MAX_LOGICAL_PATH_BYTES = 1024
MAX_NAME_COMPONENT_BYTES = 255
~~~

---

# 8. Dual-path Guard Contract

copy and move are dual-path Tools.

Before mutation:

1. validate source logical path;
2. validate destination logical path;
3. verify source final target remains inside Workspace and eligible;
4. verify destination parent final target remains inside Workspace;
5. verify destination is absent;
6. bind Approval and evidence to the same logical invocation;
7. re-check source/destination facts immediately before mutation.

State change after Approval returns:

~~~text
TARGET_CHANGED
~~~

No best-effort continuation.

---

# 9. Overwrite and Conflict Contract

M09 v1 has no overwrite behavior.

For copy / move / rename:

~~~text
destination exists -> TARGET_EXISTS
~~~

For mkdir:

~~~text
target exists -> TARGET_EXISTS
~~~

For delete:

~~~text
non-empty directory -> DIRECTORY_NOT_EMPTY
~~~

No prompt or schema may imply implicit replacement.

---

# 10. Cross-volume and Intermediate-state Contract

## copy

Copy may stream bytes only when both guarded paths remain inside the Workspace security boundary. Failure before final commit leaves the source unchanged and final destination absent. Temporary artifact cleanup is required and cleanup failure is observable.

## move

move requires same-volume atomic rename semantics.

Cross-volume:

~~~text
CROSS_VOLUME_UNSUPPORTED
~~~

No copy+delete fallback.

## rename

Same-parent atomic rename; cross-volume is not applicable.

## delete

No recursive delete, preventing partially-deleted directory trees in M09 v1.

---

# 11. Recovery Boundary

## mkdir

If recursive creation partially creates new parent directories before a later failure, the result must report partial creation truthfully and must never remove pre-existing directories. Created logical paths remain bounded evidence.

## list_directory

Read-only and mutation-free.

## copy

Before atomic final commit:

- source unchanged;
- final destination absent;
- temp artifact cleaned if possible;
- cleanup failure observable.

## move / rename

Exactly one atomic filesystem rename operation after preflight. If it fails, there is no multi-step fallback.

## delete

After success there is no product-level undo guarantee. Irreversibility must be visible in Tool description and Approval UX.

---

# 12. Safe Evidence Contract

Every ADMIT Tool adds a typed bounded DurableSanitizedArgumentsEvidence variant.

Allowed examples:

- mkdir: logicalPath, recursive;
- list_directory: logicalPath, maxEntries;
- copy: sourceLogicalPath, destinationLogicalPath, sourceBytes;
- move: sourceLogicalPath, destinationLogicalPath, entryKind;
- rename: logicalPath, newNameBytes;
- delete: logicalPath, entryKind.

Bounded result evidence may include:

- created;
- entryCount;
- truncated;
- bytesCopied;
- moved;
- renamed;
- deleted;
- entryKind.

Forbidden:

- raw file contents;
- raw directory listing persisted as evidence;
- physical paths;
- secret / env / argv;
- arbitrary OS error text;
- temp physical paths.

If M08 exists, admitted Tools must flow through the same generic Activity / History / Metrics / Trace identity.

---

# 13. Production Surface Admission Rule

Only final ADMIT candidates may be added to:

- ToolRegistry;
- ToolConfig;
- ToolPolicyDefaults;
- policy matcher whitelist;
- MCP tools/list/schema;
- ExecutionPipeline dispatch;
- Tools UI;
- typed evidence variants.

REJECT and DEFER candidates remain completely absent from production surfaces.

AppConfig remains schemaVersion 3.

For every ADMIT Tool:

### New install

~~~text
tools.<tool>.enabled = true
policy default = frozen candidate default
~~~

### Existing pre-M09 v1/v2/v3 config missing the field

~~~text
tools.<tool>.enabled = false
policy default = frozen candidate default
~~~

This prevents upgrade-time surprise exposure.

Normalization must preserve configRevision unless the user explicitly changes restart-affecting Tool state.

---

# 14. Admission Evaluation Profiles

## 14.1 Five-tool control

Profile:

~~~text
m09-five-tool-control-v1
~~~

Exact catalog:

~~~text
read_file
glob
grep
write
edit
~~~

It must be schema-identical to the accepted M05 quality control.

bash forbidden.

## 14.2 Single-candidate profiles

Exactly one candidate is added to the five-tool control:

~~~text
m09-mkdir-candidate-v1
m09-list-directory-candidate-v1
m09-copy-candidate-v1
m09-move-candidate-v1
m09-rename-candidate-v1
m09-delete-candidate-v1
~~~

No profile silently contains another candidate.

---

# 15. Candidate Admission Dataset

Dataset id:

~~~text
m09-file-tool-admission-v1
~~~

For each candidate:

- 3 positive capability cases;
- 3 negative ambiguity/sentinel cases;
- 3 repetitions per case.

Total:

~~~text
6 candidates x 6 cases x 3 repetitions = 108 logical candidate-profile runs
~~~

Positive prompts are neutral natural-language tasks and do not name the Tool.

Negative sentinels are tasks where existing read_file / glob / grep / write / edit is the correct choice; the candidate should not be selected merely because it exists.

---

# 16. Product-value and Overlap Gate

Before paid Model runs, every candidate gets a deterministic capability matrix.

ADMIT eligibility requires at least 2 of 3 positive cases to represent a final state that the five-tool control cannot reasonably achieve with equivalent semantics without:

- bash;
- shell commands;
- unsafe raw filesystem access;
- fabricating success;
- violating the task.

If current tools cleanly cover the candidate and the new Tool mainly adds alias ambiguity, verdict is REJECT.

This gate is deterministic product evidence, not model opinion.

---

# 17. Candidate-specific Controlled Eval Gate

For a candidate that passes structural gates:

~~~text
18 logical runs
scoreable >= 17/18
each case scoreable >= 2/3
positive final task completion >= 8/9
positive expected-candidate selection >= 8/9
positive argument validity >= 8/9
negative sentinel final task completion >= 8/9
negative sentinel candidate misuse <= 1/9
security violations = 0
~~~

Tool transport success is not final task completion.

A candidate failing a valid quality gate becomes DEFER, not automatically REJECT.

---

# 18. Final Cumulative Regression Gate

Build the provisional ADMIT set from candidates that pass structural and candidate-specific gates.

Profile:

~~~text
m09-final-candidate-v1
~~~

Catalog:

~~~text
five-tool control + provisional ADMIT candidates only
~~~

Run unchanged:

~~~text
m04-core-v1
24 cases x 3 = 72 logical runs
~~~

Use the accepted M05 comparator unchanged:

~~~text
Ctask >= Btask - 0.02
Csel >= Bsel - 0.03
Carg >= Barg - 0.01
Cout >= Bout - 0.01
Cunnecessary <= Bunnecessary + 0.05
category task guard >= baseline - 0.08
category selection guard >= baseline - 0.10
stable baseline 3/3 case -> candidate >= 2/3
security violations = 0
~~~

Validity:

~~~text
72 logical runs
scoreable >= 70
each case >= 2/3
each category >= 11/12
~~~

No threshold lowering is allowed.

---

# 19. One Diagnostic Isolation Round

If the provisional set fails cumulative regression, exactly one pre-declared isolation round is allowed.

Procedure:

1. rank provisional candidates by higher negative-sentinel misuse;
2. then lower positive final-task completion;
3. then fixed tie order: mkdir, list_directory, copy, move, rename, delete;
4. remove the worst candidate;
5. rerun the same 72-run cumulative regression once.

If PASS:

- removed candidate = DEFER;
- remaining provisional candidates may ADMIT.

If FAIL:

- all remaining provisional candidates = DEFER.

No arbitrary subset search and no repeated threshold shopping.

---

# 20. Provider Drift and Real Model Rules

Real controlled Model execution requires explicit user authorization at execution time.

Record:

- adapter/version;
- requested model;
- resolved model;
- dataset / scorer / profile / schema hashes;
- repetition count;
- runner/runtime commit;
- environment.

If accepted M05 model identity differs or is not established, run the five-tool paired control on unchanged m04-core-v1 and apply the existing provider-drift guard.

Drift beyond guard:

~~~text
INCONCLUSIVE
~~~

No silent model substitution.

---

# 21. Accepted M05 Handoff Requirement

Formal M09 admission requires machine-readable accepted M05 quality evidence containing at least:

- accepted candidate commit;
- runtime/runner hashes;
- m04-core-v1 dataset hash;
- scorer/system-prompt/profile hashes;
- five-tool schema hash;
- adapter/version;
- requested/resolved model;
- aggregate raw counts;
- category counts;
- per-case counts;
- security count;
- validity result.

Source constants are not a substitute for actual accepted evidence.

Missing handoff:

~~~text
M09 Admission Quality Gate = BLOCKED
~~~

---

# 22. Runtime Security Pipeline

Every ADMIT Tool uses:

~~~text
MCP tools/call
-> Runtime MCP adapter
-> ExecutionPipeline
-> ToolRegistry / CapabilityGate
-> WorkspaceGuard
-> PolicyEngine
-> Approval when ASK
-> ExecutionPermit
-> native Rust executor
-> canonical AuditEvent / Safe Evidence
-> Activity / History / optional M08 Metrics+Trace
~~~

Forbidden:

- shell fallback;
- PowerShell;
- cmd.exe;
- robocopy;
- external filesystem utility;
- UI-direct filesystem mutation;
- Developer/Test bypass of Workspace / Policy / Approval.

---

# 23. Approval Contract

Mutation candidates default to ASK.

Required:

- no filesystem mutation before Approval allow;
- DENY leaves filesystem unchanged;
- allow_once applies only to exact invocation;
- stale Approval cannot authorize changed source/destination;
- target change after Approval returns TARGET_CHANGED.

list_directory defaults ALLOW but still respects Tool enabled state and Policy override.

---

# 24. Typed Error Contract

Applicable typed errors include:

~~~text
INVALID_ARGUMENTS
WORKSPACE_VIOLATION
TOOL_DISABLED
POLICY_DENIED
APPROVAL_REQUIRED
APPROVAL_DENIED
TARGET_NOT_FOUND
TARGET_EXISTS
TARGET_CHANGED
NOT_A_DIRECTORY
NOT_A_FILE
DIRECTORY_NOT_EMPTY
UNSUPPORTED_REPARSE
CROSS_VOLUME_UNSUPPORTED
FILE_TOO_LARGE
IO_FAILURE
ATOMIC_FAILURE
~~~

MCP / Activity / History must not expose raw OS error strings or physical paths.

---

# 25. Result Contracts

## list_directory

At most maxEntries entries, each with:

~~~text
name
kind: file | directory | other | reparse
sizeBytes?: integer
~~~

Logical child name only, deterministic ordering, no child content, no recursion, bounded truncated flag.

## copy

Returns safe logical facts:

~~~text
sourcePath
destinationPath
bytesCopied
atomicCommit
~~~

File contents must match byte-for-byte.

M09 v1 does not promise ACL/metadata preservation.

## move / rename

Returns safe logical facts plus entry kind and atomic=true.

Unsupported atomic semantics fail rather than falling back.

## delete

Returns:

~~~text
path
entryKind
deleted: true
irreversible: true
~~~

No recursive deletion and no recovery promise.

---

# 26. UI Contract

Tools page shows only production-admitted Tools.

For each ADMIT Tool show:

- localized description;
- capability;
- enabled state;
- saved policy;
- effective Runtime availability;
- Restart Required when applicable.

Mutation descriptions disclose:

- ASK default;
- no overwrite;
- move cross-volume unsupported;
- delete irreversibility and non-recursive scope.

REJECT/DEFER candidates are not shown as disabled product Tools.

---

# 27. M08 Compatibility

M08 is not an M09 implementation dependency.

If M08 exists in the integrated candidate, ADMIT Tools must automatically appear in canonical:

- Activity;
- History;
- Metrics;
- Request Trace.

M09 must not add a second observability path.

M11 later validates the combined Full candidate.

---

# 28. M09 Derived Acceptance Criteria

| M09 AC | Requirement |
| --- | --- |
| M09-AC01 | All six candidates have exactly one machine-readable ADMIT/REJECT/DEFER verdict with evidence hashes and reason code. |
| M09-AC02 | Verdict rules distinguish deterministic REJECT from evidence/quality DEFER; unavailable Model evidence never becomes REJECT or PASS. |
| M09-AC03 | Five-tool M05 control remains exact and immutable; bash is absent from controlled quality profiles. |
| M09-AC04 | Each candidate has frozen v1 schema, value/overlap analysis, capability, policy, Workspace and failure semantics before admission. |
| M09-AC05 | ADMIT-eligible candidate proves at least two unique positive scenarios that current five-tool control cannot safely satisfy equivalently. |
| M09-AC06 | Admission dataset has 3 positive + 3 negative sentinel cases x3 repetitions per candidate with versioned hashes and ground truth. |
| M09-AC07 | Candidate evidence meets >=17/18 scoreable, per-case >=2/3, positive completion/selection/argument >=8/9, negative completion >=8/9, misuse <=1/9, security=0. |
| M09-AC08 | Final provisional set passes unchanged m04-core-v1 72-run regression against accepted M05 five-tool baseline using frozen M05 thresholds. |
| M09-AC09 | Provider/model drift uses paired control and INCONCLUSIVE rules; no silent substitution or threshold lowering. |
| M09-AC10 | REJECT/DEFER candidates are absent from ToolRegistry, AppConfig, policy whitelist, MCP catalog/schema, UI and dispatch. |
| M09-AC11 | Every ADMIT Tool uses complete Workspace/Policy/Approval/Permit/executor/Audit pipeline with no shell/external utility fallback. |
| M09-AC12 | New installs enable only actual ADMIT tools with frozen defaults; upgrades missing fields keep them disabled while preserving AppConfig schema v3 and revision semantics. |
| M09-AC13 | mkdir/list_directory obey frozen relative-path, bound, reparse and deterministic semantics. |
| M09-AC14 | copy is guarded binary-safe file-only copy, destination-absent, <=64MiB and temp+atomic final commit without overwrite. |
| M09-AC15 | move is dual-path guarded same-volume atomic move only, destination absent, no copy+delete fallback, rejecting root/self/descendant/reparse cases. |
| M09-AC16 | rename is same-parent one-component atomic rename with destination absent and explicit overlap evaluation against move. |
| M09-AC17 | delete is ASK-by-default, irreversible, limited to regular file/empty directory, rejecting recursive/non-empty/root/reparse deletion. |
| M09-AC18 | Dual-path/destructive Tools detect post-approval target change and define conflict, cross-volume, intermediate-state and recovery boundaries. |
| M09-AC19 | ADMIT Tools persist only typed bounded safe evidence and preserve exact request identity/privacy. |
| M09-AC20 | Existing read_file/glob/grep/write/edit behavior and M05 quality gate regressions remain PASS; M04/M05 baselines stay unchanged. |
| M09-AC21 | If M08 is present, ADMIT Tools appear in Activity/History/Metrics/Trace through canonical evidence without a second path. |
| M09-AC22 | Packaged candidate catalog exactly matches final ADMIT set and every delivered ADMIT Tool has real Runtime/E2E normal, deny/approval, boundary and final-filesystem-state evidence. |

---

# 29. Canonical to Derived to Validation Traceability

| Canonical | Derived | Task owner | Reserved Integration | Reserved E2E | Reserved Manual |
| --- | --- | --- | --- | --- | --- |
| F11-AC01 | AC01-AC10 | T1/T2 | M09-INT-01/02/03/04/05 | M09-E2E-01 | M09-MAN-01/02 |
| F11-AC02 | AC10-AC13/19/22 | T3/T4/T5 | M09-INT-06/07/08/09/10/16 | M09-E2E-02/03 | M09-MAN-03/04 |
| F11-AC03 | AC14-AC18/20/22 | T3/T4/T5/T6 | M09-INT-11/12/13/14/15/17/18 | M09-E2E-03/04 | M09-MAN-04/05 |
| F12-AC01/02/03/04 | AC03/06/07/08/09/20 | T1/T2/T6 | M09-INT-02/03/04/05/18 | M09-E2E-01/04 | M09-MAN-01/05 |

Reserved suites:

- Integration: M09-INT-01 through M09-INT-18.
- E2E: M09-E2E-01 through M09-E2E-04.
- Manual: M09-MAN-01 through M09-MAN-05.

---

# 30. Development Entry Gate

Formal M09 implementation may begin only when:

1. M04 is ACCEPTED / FROZEN;
2. M05 is ACCEPTED / FROZEN;
3. accepted M05 five-tool quality handoff is available;
4. all six M09 documents are frozen;
5. no unresolved Workspace / Policy / Approval / F12 blocker exists.

M08 Development Complete is not an entry criterion.

---

# 31. Development-stage Real Model Gate

Admission decisions determine implementation scope, so the real controlled Model admission run is part of M09 development gating.

At execution time:

- explicit user authorization is required before paid/networked Model calls;
- without authorization, Codex may build and test the runner using scripted fixtures but MUST stop before production candidate implementation;
- status is M09 Development BLOCKED at Admission Gate;
- unit tests cannot be used to guess ADMIT decisions.

---

# 32. Development Exit Gate

M09 Development Complete requires:

- admission runner/datasets/profiles implemented;
- real authorized admission evidence completed;
- all six verdicts frozen in decision-set-v1;
- only ADMIT candidates implemented;
- production schemas exactly match evaluated schemas;
- all ADMIT Runtime/unit/security tests PASS;
- final cumulative 72-run regression PASS;
- five-tool regression PASS;
- package Tool catalog equals decision set;
- fresh Portable candidate produced;
- Formal Integration/E2E/Manual remain NOT EXECUTED unless actually run.

Development Complete is not M09 ACCEPTED / FROZEN.

---

# 33. Final Exit Gate

M09 may become ACCEPTED / FROZEN only when:

- F11-AC01/02/03 PASS;
- F12 preservation remains valid;
- M09-AC01 through M09-AC22 PASS;
- every candidate has exactly one final verdict;
- every ADMIT Tool passes Unit + Integration + packaged E2E + Manual;
- every REJECT/DEFER Tool is absent from production catalog;
- final cumulative quality regression PASS;
- provider drift rules are satisfied;
- security violation count = 0;
- outside-Workspace unauthorized mutation count = 0;
- destructive semantics and irreversibility are accurately presented;
- no shell fallback exists;
- existing read/glob/grep/write/edit regressions PASS;
- real ChatGPT Tool list matches the final ADMIT set;
- no current blocker remains.

Successful wording:

~~~text
M09 — ACCEPTED / FROZEN
~~~

M09 does not require all six candidates to be ADMIT and does not implement optional F09 Token Estimates.
