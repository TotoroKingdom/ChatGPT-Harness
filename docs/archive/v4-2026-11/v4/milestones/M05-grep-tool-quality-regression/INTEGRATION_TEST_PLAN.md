# M05 Integration Test Plan — grep Tool and Quality Regression

> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M05 — grep Tool and Quality Regression  
> Inputs: SPEC.md, PLAN.md, TASKS.md  
> Owner: independent QA / Coding Agent after M05 Development Complete  
> Scope: F10 production grep mechanics + M03 evidence integration + M04 controlled-evaluation regression mechanics  
> Network rule: Integration MUST NOT make paid/networked Model requests and MUST NOT use External ChatGPT. ModelAdapter behavior is exercised only through scripted/fake boundaries. Real controlled-Model execution belongs to E2E-03 after explicit user authorization. External ChatGPT belongs to Manual Acceptance.  
> Safety: all Runtime, Workspace, SQLite, Approval, config and filesystem state must be test-owned and isolated. Never attach Integration to the user's active Workspace or active Runtime.  
> Execution results: do not edit this frozen plan to record PASS/FAIL. Report actual results in the execution conversation and delivery evidence.

---

# 1. Objective

Integration proves that M05 is a real bounded WorkspaceRead Tool integrated into the existing Runtime authority and that the M04 regression machinery remains trustworthy before any paid Model run or external ChatGPT acceptance.

The suite must prove:

- grep protocol/schema/config is exact and additive;
- literal/regex/scope/include/exclude semantics are correct;
- deterministic ordering/truncation is independent of filesystem enumeration order;
- frozen file/result/byte/output/regex/time limits are enforced;
- binary/invalid UTF-8/oversize/unreadable files are handled with typed bounded diagnostics;
- no shell grep/ripgrep/Bash/PowerShell fallback exists;
- Workspace escape, Windows symlink/junction/reparse and TOCTOU swaps cannot read outside bytes;
- disabled/DENY/ASK paths execute through the real ExecutionPipeline and do not scan before authorization;
- Runtime cancellation/stop and timeout cannot turn partial work into successful output;
- Activity and M03 History use exact runtime_id + request_id;
- durable evidence contains no raw query/filter/match text;
- AppConfig remains schema v3 with distinct new-install vs upgraded-install grep defaults;
- Tools UI exposes grep state and restart semantics correctly;
- M04 baseline files/profile/scorer remain unchanged;
- M05 candidate/control/supplemental Eval profiles are isolated and deterministic;
- the existing M04 compare_m05 and provider-drift thresholds remain immutable;
- previous M03/M04/V3/M01/M02 behavior is not regressed.

Integration PASS does not prove real controlled-Model quality and does not prove External ChatGPT usability.

---

# 2. Entry Criteria

Formal Integration may begin only when:

- M03 is ACCEPTED / FROZEN;
- M04 is ACCEPTED / FROZEN;
- the accepted M04 baseline identity/result required by M05 SPEC §23 is available;
- M05 T1–T6 are Development Complete;
- M05 candidate commit is frozen and working tree is clean;
- task-local tests are PASS;
- a fresh M05 Portable candidate exists or the candidate production binaries used for E2E are reproducible from the same SHA;
- no test requires the user's active Harness/Workspace;
- no M04 baseline prompt/fixture/scorer file has been edited by M05.

Preflight:

~~~text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

cd ui
npm run typecheck
npm test
npm run build
~~~

If M03 or M04 is not ACCEPTED / FROZEN, formal M05 Integration is BLOCKED.

---

# 3. Evidence Header

Record once in execution evidence:

~~~text
Milestone: M05
Candidate commit:
Branch / dirty state:
Runtime SHA-256:
Portable SHA-256:
Protocol/schema generation identity:
grep Tool schema hash:
M03 evidence schema/checksum:
M04 accepted baseline identity:
M04 dataset hash:
M04 system prompt hash:
M04 scorer hash:
M04 Tool schema hash:
M05 candidate profile hash:
M05 supplemental dataset hash:
Windows version:
QA owner:
Execution time:
~~~

Never record:

- provider API key;
- SecretStore values;
- real user Workspace physical path;
- raw environment;
- unrestricted file contents;
- raw OS error text containing physical paths;
- model hidden reasoning.

Allowed verdicts:

- PASS
- FAIL
- BLOCKED
- INCONCLUSIVE

---

# 4. Required Integration Cases

## M05-INT-01 — grep protocol schema and argument contract

**AC:** M05-AC01  
**Tasks:** T1/T3

Exercise the generated protocol schema and real MCP argument parser.

Verify:

- Tool name is grep;
- unknown fields are rejected;
- query is required;
- empty query is rejected;
- query length 1..=4096 UTF-8 bytes;
- mode defaults to literal;
- only literal/regex modes are accepted;
- scope defaults to dot and is bounded to 1024 bytes;
- include/exclude are arrays;
- each include/exclude pattern <=512 bytes;
- at most 32 include and 32 exclude entries;
- maxResults defaults 100, range 1..=1000;
- maxOutputBytes defaults 65536, range 1024..=262144;
- schema and Runtime parser agree on all defaults/ranges;
- TS/JSON Schema/protocol fixtures are regenerated and round-trip.

Negative controls include wrong types, missing query, unknown fields and over-limit values.

**PASS:** protocol and Runtime reject the same invalid requests and expose exactly the frozen schema.

---

## M05-INT-02 — literal/regex correctness, line numbers, offsets and preview

**AC:** M05-AC02  
**Task:** T2

Use deterministic temp fixtures covering LF, CRLF, UTF-8 multibyte text, BOM, long lines and repeated matches.

Verify:

- literal search is case-sensitive exact substring;
- regex uses Rust regex semantics;
- regex inline flags behave explicitly;
- regex never spans logical lines;
- one result is emitted per matching line even when multiple matches occur;
- lineNumber is 1-based;
- matchStartByte/matchEndByte refer to the first match in the logical line;
- previewStartByte is consistent with returned preview;
- preview is valid UTF-8;
- preview contains the first match;
- preview <=4096 bytes;
- previewTruncated is correct;
- zero-match success returns empty matches, not an error.

Positive/negative golden fixtures must be byte-for-byte deterministic.

**PASS:** search semantics and line/offset metadata exactly match the frozen contract.

---

## M05-INT-03 — relative scope, include/exclude and deterministic traversal

**AC:** M05-AC02, AC03  
**Task:** T2

Create a nested Workspace with deliberately shuffled file creation/enumeration order.

Verify:

- scope dot searches the full eligible Workspace;
- nested relative scope restricts traversal;
- include selects expected normalized Workspace-relative slash paths;
- empty include includes all eligible files;
- exclude wins over include;
- invalid glob pattern returns INVALID_FILTER_PATTERN before scanning;
- normalized result paths are Workspace-relative and slash-separated;
- traversal is deterministic sorted depth-first;
- output order is path ascending by UTF-8 byte sequence then lineNumber ascending;
- repeated runs against unchanged bytes return identical result prefixes even if creation/enumeration order differs.

**PASS:** filtering and traversal do not depend on OS enumeration order.

---

## M05-INT-04 — frozen resource bounds and deterministic truncation

**AC:** M05-AC03, AC11  
**Task:** T2

Exercise each frozen deterministic resource limit independently:

- maxResults;
- maxOutputBytes;
- MAX_DISCOVERED_ENTRIES = 50,000;
- MAX_SCANNED_FILES = 10,000;
- MAX_SCANNED_BYTES = 64 MiB;
- MAX_SINGLE_FILE_BYTES = 8 MiB;
- MAX_PREVIEW_BYTES = 4096;
- MAX_DIAGNOSTICS = 100;
- query/filter limits.

Verify:

- the next item that would exceed a deterministic limit is not accepted;
- returned matches are a deterministic prefix;
- truncated=true;
- correct truncationReasons are emitted;
- repeated runs return the same prefix;
- maxOutputBytes accounting follows path bytes + preview bytes;
- deterministic truncation remains a successful Tool result;
- no hidden unbounded collection is built before truncation.

Boundary tests include exact-limit PASS and one-over-limit behavior.

**PASS:** all product bounds are enforced exactly and truncation is deterministic.

---

## M05-INT-05 — file eligibility, encoding and bounded diagnostics

**AC:** M05-AC04  
**Task:** T2

Fixtures:

- normal UTF-8;
- UTF-8 BOM;
- NUL/binary;
- invalid UTF-8;
- >8 MiB file;
- unreadable file where the platform permits deterministic setup;
- special/non-regular file where applicable.

Verify:

- eligible UTF-8 files are searched;
- BOM is ignored only at file start;
- binary/NUL is skipped;
- invalid UTF-8 is skipped;
- oversized file is skipped without reading unbounded content;
- unreadable/special file yields a typed bounded diagnostic;
- per-file skip does not fail the whole search;
- skippedFiles counts are correct;
- diagnostics are capped at 100;
- diagnosticsTruncated is accurate;
- diagnostics contain only safe logical paths and typed codes;
- raw OS error/physical path/file body does not leak.

**PASS:** file eligibility behavior is bounded and diagnosable without leaking raw platform details.

---

## M05-INT-06 — timeout, cancellation, Runtime stop and async responsiveness

**AC:** M05-AC03, AC11  
**Tasks:** T2/T3

Use deterministic delay/cancellation hooks rather than relying on a naturally slow machine.

Verify:

- 5-second wall deadline produces SEARCH_TIMEOUT;
- timeout does not return a successful partial result;
- explicit request cancellation stops further traversal/read;
- Runtime stop terminates active grep within existing Runtime shutdown semantics;
- no external child process exists;
- no worker remains orphaned after shutdown;
- already accumulated partial matches are not reported as success on cancel/timeout;
- normal Runtime control/Approval requests remain responsive while grep performs blocking filesystem work;
- blocking scan runs outside the Tokio core worker.

**PASS:** long-running grep cannot freeze Runtime control flow or masquerade partial work as success.

---

## M05-INT-07 — AppConfig v3 new-install and additive upgrade semantics

**AC:** M05-AC06  
**Tasks:** T1/T4  
**Upstream:** A15

Run real ConfigStore fixtures for v1, v2, pre-M05 v3 and post-M05 v3.

Verify:

### New install

- schemaVersion remains 3;
- tools.grep.enabled = true;
- policies.toolDefaults.grep = allow.

### Existing config missing grep

- schemaVersion remains 3;
- grep becomes explicitly disabled;
- grep default Policy is allow;
- all existing Tool/Policy/Workspace/Tunnel/SecretReference fields are preserved;
- migration itself preserves configRevision;
- normal load may atomically persist the normalized shape;
- read-only inspection performs no write;
- failed persistence preserves the original config.

### User enablement

- explicit grep enable advances configRevision;
- running old generation shows Restart Required;
- old Runtime effective catalog does not magically change;
- restarted Runtime exposes grep.

**PASS:** upgrade does not silently expand an existing user's Tool surface and does not create hidden config state.

---

## M05-INT-08 — ToolRegistry, capability, tools/list and schema exposure

**AC:** M05-AC05, AC07  
**Tasks:** T1/T3

Verify the real Runtime registry order:

~~~text
read_file
glob
grep
write
edit
bash
~~~

Verify grep:

- implemented=true;
- capability=WorkspaceRead;
- risk=Low;
- scope=workspace;
- readiness semantics match other built-in WorkspaceRead Tools;
- enabled state comes from ToolConfig;
- disabled registration is not executable;
- tools/list includes grep only when the effective profile exposes it;
- tools/list JSON schema hash is stable for the candidate;
- M05 strict Eval candidate catalog is exactly five Tools and excludes bash;
- M04 baseline mode still has exactly four Tools and rejects grep drift.

**PASS:** one authoritative registry entry controls capability/exposure rather than duplicated metadata.

---

## M05-INT-09 — Policy, Approval and disabled Tool authority

**AC:** M05-AC07, AC08  
**Task:** T3

Use the real Runtime pipeline with an executor scan-start spy.

Required paths:

1. grep disabled;
2. Policy ALLOW;
3. Policy DENY;
4. Policy ASK → allow_once;
5. Policy ASK → deny;
6. explicit user Policy rule targeting grep.

Verify:

- disabled -> TOOL_DISABLED and scan-start count = 0;
- DENY -> no scan/file read;
- ASK creates a real Pending Approval before scan;
- allow_once resumes the exact same request_id and then scans;
- deny leaves scan-start count = 0;
- Approval summary contains no raw query/matched line;
- no SessionGrant/ExecutionPermit is fabricated by the test harness;
- current Runtime Policy remains authority.

**PASS:** read-only capability does not bypass Policy/Approval.

---

## M05-INT-10 — ExecutionPipeline ownership and no shell/direct-filesystem bypass

**AC:** M05-AC07  
**Task:** T3

Audit code and exercise instrumentation around the real MCP call.

Verify:

- MCP adapter does not directly open/search files;
- grep executor is reachable only after ExecutionPipeline authorization;
- no Command/process spawn for grep;
- no bash.exe, sh, cmd.exe, powershell.exe, pwsh, grep.exe or ripgrep/rg executable is invoked;
- no fallback to existing Bash Tool;
- executor receives typed validated request, not arbitrary trusted physical path;
- Policy/Audit lifecycle events are emitted in normal pipeline order;
- disabling the executor path causes normal typed execution failure rather than shell fallback.

**PASS:** grep is a native Runtime WorkspaceRead implementation through the existing authority path.

---

## M05-INT-11 — Workspace escape, symlink, junction and reparse defense

**AC:** M05-AC09  
**Task:** T2

On Windows candidate fixtures, create test-owned:

- parent escape path;
- absolute path;
- sibling-prefix path;
- scope symlink/reparse;
- nested directory junction/reparse pointing outside;
- candidate file symlink/reparse pointing outside.

Use an outside sentinel whose bytes must never be observed.

Verify:

- invalid scope fails before scan;
- traversal never descends through reparse directory;
- reparse candidate is skipped/rejected;
- outside sentinel never appears in Tool result, diagnostics, Activity, History or logs;
- no physical outside path is returned;
- safe typed diagnostic is emitted where required.

If the current Windows environment cannot create a required reparse fixture, this case is BLOCKED, not PASS.

**PASS:** no Workspace escape reads outside bytes.

---

## M05-INT-12 — TOCTOU opened-handle final-path protection

**AC:** M05-AC10  
**Task:** T2

Use the frozen test-only race seam to swap a candidate between enumeration/validation and open/read.

Required races:

- inside regular file -> outside target;
- inside regular file -> reparse target;
- delete/recreate with changed identity;
- safe in-Workspace replacement control.

Verify:

- final opened-handle path is rechecked before any content byte is consumed;
- outside/reparse swap yields target_changed/workspace_violation-safe behavior;
- outside sentinel bytes are never observed;
- safe in-Workspace replacement follows the documented target-changed semantics;
- no canonicalize-once shortcut can pass the negative control;
- Windows implementation exercises final-path-from-handle behavior equivalent to the SPEC.

**PASS:** post-validation path substitution cannot escape Workspace confinement.

---

## M05-INT-13 — exact Activity/M03 History correlation and privacy

**AC:** M05-AC12  
**Task:** T3  
**Upstream:** SEC-AC01/02/03, A04/A08/A17

Run real isolated Runtime grep calls for success, truncation, DENY and timeout.

For each request verify:

- exact runtime_id + request_id is returned/correlated;
- same key appears in live Activity;
- same key appears in M03 request_summary/evidence_events;
- no nearest-time/fuzzy join is used;
- durable argument evidence contains only mode, logical scope, query byte count, filter counts and limits;
- durable result evidence contains only bounded scalars;
- raw query sentinel absent;
- include/exclude pattern sentinels absent;
- matching line/preview sentinel absent;
- physical path absent;
- M03 SQLite physical schema remains v1;
- History never creates a live Approval/SessionGrant/Permit.

**PASS:** grep evidence is exact, safe and observational.

---

## M05-INT-14 — Tools UI, restart state, i18n and safe presentation

**AC:** M05-AC06, AC13  
**Task:** T4

Use frontend/Tauri integration tests.

Verify:

- grep appears in Tools page;
- localized built-in description exists in en and zh-CN;
- new install shows configured ON / ALLOW;
- upgraded pre-M05 config shows configured OFF / ALLOW;
- enabled switch saves through existing config mutation coordinator;
- Policy ALLOW/ASK/DENY control saves;
- configRevision/restart-required presentation is correct;
- saved/effective mismatch is explicit while old Runtime generation runs;
- after restart effective grep state converges;
- Inspector shows safe Tool status;
- History/Activity presentation does not reveal raw query/match text;
- no separate ad-hoc grep search UI is introduced.

**PASS:** user-visible grep configuration is truthful about desired/effective state.

---

## M05-INT-15 — M04 baseline immutability and baseline-profile rejection of grep

**AC:** M05-AC14  
**Task:** T5

Compare M05 candidate against the accepted M04 immutable identities.

Verify:

- m04-core-v1 dataset identity/hash unchanged;
- all 24 M04 case JSON identities unchanged;
- all M04 fixture tree hashes unchanged;
- M04 system prompt hash unchanged;
- M04 scorer hash/semantic golden behavior unchanged;
- M04 baseline Tool catalog remains exactly read_file, glob, write, edit;
- M04 baseline mode given an extra grep Tool returns HARNESS_CONTRACT_ERROR;
- M04 content_search oracle still uses original baseline strategy;
- no M04 prompt was rewritten to encourage/discourage grep.

**PASS:** M05 cannot improve its apparent regression result by moving the baseline.

---

## M05-INT-16 — strict M05 candidate validity and frozen compare_m05 gate

**AC:** M05-AC15, AC16  
**Task:** T5

Use scripted deterministic run summaries and candidate profile mechanics only; no real provider call.

Verify candidate profile:

- ID = m05-grep-candidate-v1;
- Tool catalog exactly read_file, glob, grep, write, edit;
- dataset = unchanged m04-core-v1;
- 24 cases × 3 = 72 logical runs;
- >=70 scoreable minimum;
- each case >=2/3 scoreable;
- each category >=11/12 scoreable;
- one stable candidate identity;
- zero security violations.

Verify compare_m05 exact boundaries:

~~~text
task regression <= 0.02
selection regression <= 0.03
argument regression <= 0.01
runtime outcome regression <= 0.01
unnecessary-call increase <= 0.05
category task guard <= 0.08
category selection guard <= 0.10
stable 3/3 case must remain >=2/3
~~~

Test exact-boundary PASS and just-over-boundary FAIL for each metric.

Any security violation must FAIL regardless of averages.

**PASS:** M05 cannot weaken the already frozen M04 regression gate.

---

## M05-INT-17 — paired control, provider drift and supplemental grep-admission mechanics

**AC:** M05-AC17, AC18  
**Task:** T5

No real provider call.

### Paired control

Verify:

- profile = m05-paired-control-v1;
- grep disabled/not exposed;
- observed Tool catalog/schema exactly matches accepted M04;
- existing compare_provider_drift is called;
- exact drift-boundary PASS;
- just-over-boundary -> INCONCLUSIVE;
- candidate is not blamed/credited when control drift exceeds guard.

### Supplemental set

Verify:

- set ID = m05-grep-admission-v1;
- exactly 4 cases × 3 = 12 logical runs;
- cases are neutral and do not name a required Tool;
- cover literal, regex, scope/include/exclude and truncation/follow-up;
- >=11/12 scoreable validity;
- each case >=2/3 scoreable;
- zero security violations;
- metrics are reported separately;
- supplemental metrics never change the strict 72-run denominators/comparator.

**PASS:** drift control and grep-admission diagnostics cannot contaminate strict regression scoring.

---

## M05-INT-18 — cross-cutting regression, packaging inputs and repository invariants

**AC:** M05-AC19, AC20  
**Task:** T6

Run full affected regression coverage:

- read_file;
- glob;
- write;
- edit;
- bash;
- Workspace;
- Tool enable/disable;
- Policy ALLOW/ASK/DENY;
- Approval/SessionGrant;
- SecretStore;
- Start/Stop/Restart;
- M03 Activity/History writer/read-only boundaries;
- M04 baseline validate/preflight/scripted/comparator paths;
- M01 Tray/background/notifications;
- M02 startup recorder/diagnostics;
- frontend typecheck/tests/build;
- repository layout checks.

Audit:

- no new top-level directory;
- no M03 DB schema migration;
- no M04 baseline file drift;
- no shell search dependency;
- no real provider request during Integration;
- no External ChatGPT invocation during Integration;
- delivery scripts can build a fresh candidate without overwriting earlier artifacts.

**PASS:** M05 is production-compatible and ready for formal E2E.

---

# 5. Integration Exit Gate

M05 Integration is PASS only when:

- M05-INT-01 through M05-INT-18 are all PASS;
- M03 and M04 dependencies are ACCEPTED / FROZEN;
- no paid/network Model request was made;
- no External ChatGPT request was made;
- real Runtime/MCP/Workspace/Policy/Approval/M03 paths were exercised;
- Windows reparse and TOCTOU cases were actually executed on Windows;
- outside sentinel read count is zero;
- durable grep privacy sentinels are absent;
- M04 baseline files/identity are unchanged;
- compare_m05/provider-drift golden controls pass;
- temp Runtime/Workspace/DB/config/process resources are cleaned;
- no unresolved regression remains.

Integration PASS unlocks formal M05 E2E. It does not mark M05 ACCEPTED / FROZEN.
