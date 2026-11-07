# M05 SPEC — grep Tool and Quality Regression

> 布局与验证生命周期遵守 [AGENTS.md](../../../../AGENTS.md)。本目录启动阶段只创建 SPEC / PLAN / TASKS；后续补齐 Integration / E2E / Manual 三份验证计划，最终严格保持六文件布局。不得新增独立报告、里程碑子目录、仓库副本、worktree 或独立 grep/eval 工程。  
> Status: SPEC_READY / IMPLEMENTATION BLOCKED UNTIL M04 ACCEPTED / FROZEN  
> Milestone: M05 — grep Tool and Quality Regression  
> Scope: P0 F10 + mandatory F12 regression  
> Roadmap dependencies: M03 Shared Evidence + M04 Controlled Agent Evaluation Baseline  
> Current main observed before this document commit: 3d392cd17b2ab9c10a68a169e278277c21cdf58c  
> Current M04 state supplied by user: Development Complete. This permits M05 documentation work, but does not satisfy the M05 implementation/acceptance dependency.  
> Contract rule: M04 dataset, prompts, fixtures, scorer formulas and regression thresholds are upstream frozen inputs. M05 may add a versioned candidate catalog and supplemental admission set, but may not rewrite the accepted M04 baseline to make grep pass.

---

# 1. Objective

M05 adds one production Tool:

~~~text
grep
~~~

for bounded, Workspace-confined text search.

The feature must provide:

- literal text search;
- Rust-regex search;
- Workspace-relative scope;
- include/exclude path filters;
- 1-based line numbers;
- bounded match previews;
- bounded file/byte/result/output scanning;
- deterministic ordering and deterministic truncation;
- typed errors/diagnostics;
- full Workspace / ToolConfig / Policy / Approval / ExecutionPipeline semantics;
- canonical Activity / M03 History evidence;
- real MCP discovery/call behavior;
- real ChatGPT E2E;
- controlled Agent quality regression against the accepted M04 baseline.

M05 is not accepted merely because a local search function works.

Required production path:

~~~text
ChatGPT / MCP client
        ↓
Runtime MCP grep adapter
        ↓
ExecutionPipeline
        ↓
ToolRegistry / CapabilityGate
        ↓
WorkspaceGuard
        ↓
PolicyEngine
        ↓
Approval when ASK
        ↓
Grep executor
        ↓
bounded Tool result
        ↓
canonical AuditEvent
        ↓
Live Activity + M03 durable evidence
~~~

The grep executor must never invoke shell grep, Git Bash, PowerShell, ripgrep, cmd.exe or another external search process.

---

# 2. Canonical Upstream Acceptance Criteria

## F10

- F10-AC01 — grep supports text / regex, Workspace-relative scope, include / exclude, line number, bounded results and deterministic truncation.
- F10-AC02 — grep goes through existing Workspace / Policy / Approval / ExecutionPipeline; hidden shell grep is forbidden.
- F10-AC03 — grep covers escape, symlink / junction / reparse, TOCTOU, invalid regex, oversized input/output and passes F12 baseline regression.

## F12 regression dependency

M05 consumes the accepted M04 controlled-evaluation contract:

- F12-AC01 — real controllable Model/Agent + versioned dataset/ground truth + isolated real Runtime/MCP.
- F12-AC02 — selection, argument validity, Runtime outcome and task completion remain independent.
- F12-AC03 — sample/randomness/regression thresholds remain frozen before candidate results.
- F12-AC04 — passive production telemetry does not become selection/task quality evidence.

## Shared Evidence

- SEC-AC01 — grep requests use canonical runtime_id + request_id identity.
- SEC-AC02 — durable grep evidence is typed, bounded and sanitized.
- SEC-AC03 — grep History never becomes authorization state.

## Architecture

- A12 — grep is WorkspaceRead through the existing Runtime security path.
- A04/A08 — AuditEvent and runtime_id + request_id remain canonical execution evidence/identity.
- A15 — AppConfig remains schema v3 with additive tool/config migration; old configs must not accidentally expose or mysteriously lose the new Tool.
- A17 — History/Evaluation remain observational.

---

# 3. Current Code Facts

At 3d392cd17b2ab9c10a68a169e278277c21cdf58c:

- ToolRegistry explicitly owns read_file, glob, write, edit and bash.
- ToolCapability already has WorkspaceRead.
- ToolConfig and ToolPolicyDefaults explicitly enumerate current tools.
- AppConfig CURRENT_SCHEMA_VERSION = 3.
- Tools UI dynamically renders ToolConfig keys and runtime Tool diagnostics.
- read_file/glob already demonstrate the intended WorkspaceRead + MCP + ExecutionPipeline integration pattern.
- M03 History schema is v1 and stores closed typed argument/result JSON projections.
- M04 Controlled Eval exists under runtime/src/evaluation.
- M04 baseline constants are 24 cases × 3 repetitions = 72 runs.
- M04 baseline exposed Tool catalog is exactly read_file, glob, write, edit.
- M04 comparator already contains the frozen M05 regression thresholds.
- M04 hardening rejects Tool-catalog drift in baseline mode and verifies exact live Activity + M03 request identity.
- M04 content_search prompts explicitly instruct the baseline strategy to use glob and read_file.

M05 must extend these systems rather than replace them.

---

# 4. Scope

## 4.1 Included

M05 implements:

1. grep as a production WorkspaceRead Tool;
2. protocol/config additions for grep enabled/default Policy state;
3. safe additive v3 config normalization for existing installations;
4. ToolRegistry registration and effective Tool diagnostics;
5. user Policy matcher support for grep;
6. MCP tools/list JSON schema and tools/call adapter;
7. ExecutionPipeline integration;
8. Workspace-safe bounded recursive traversal;
9. literal and Rust-regex line matching;
10. include/exclude glob filters;
11. deterministic match ordering;
12. deterministic limit/truncation semantics;
13. binary/invalid UTF-8/oversize/unreadable diagnostics;
14. Windows symlink/junction/reparse-point defense;
15. opened-handle final-path validation for TOCTOU escape defense;
16. typed safe live/durable evidence;
17. Activity/History compatibility without SQLite schema migration;
18. Tools UI and en/zh-CN text;
19. M04 runner candidate catalog mode with grep;
20. strict 72-run M04 baseline regression on unchanged m04-core-v1;
21. versioned M05 grep-admission supplemental cases;
22. existing M04 comparator reuse without threshold changes;
23. provider/model drift paired-control handling;
24. real ChatGPT grep E2E;
25. affected M03/M04/V3/M01/M02 regression.

## 4.2 Non-goals

M05 does not:

- invoke shell grep/ripgrep/PowerShell/Bash;
- search outside the configured Workspace;
- search arbitrary absolute paths;
- search binary files;
- perform byte-pattern/binary search;
- support multiline regex;
- support replace;
- support file mutation;
- add an index/database of file contents;
- watch files continuously;
- return unlimited matching lines;
- return complete huge lines;
- persist raw search query or matching line text into History;
- change M03 SQLite physical schema v1;
- change M04 baseline prompts/fixtures/ground truth;
- change M04 regression thresholds;
- claim passive production traffic proves Agent Tool-selection quality;
- implement F08/F09/F11/F13/F14/F15.

---

# 5. Tool Identity and Capability

Freeze:

~~~text
tool name: grep
capability: WorkspaceRead
risk: Low
scope: workspace
production default for a new install: enabled
production default policy for a new install: ALLOW
~~~

Registry order:

~~~text
read_file
glob
grep
write
edit
bash
~~~

The grep descriptor states that it searches text content inside the active Workspace.

Existing Tool descriptions must not be changed solely to make M05 Eval favorable.

---

# 6. MCP grep Schema v1

Logical request:

~~~text
grep({
  query: string,
  mode?: "literal" | "regex",
  scope?: string,
  include?: string[],
  exclude?: string[],
  maxResults?: integer,
  maxOutputBytes?: integer
})
~~~

## 6.1 query

Required.

Rules:

- non-empty; do not trim and silently change semantics;
- UTF-8 byte length 1..=4096;
- literal mode treats it as literal UTF-8 text;
- regex mode compiles it as Rust regex syntax;
- matching is line-by-line and never spans newline.

## 6.2 mode

~~~text
default: literal
allowed: literal | regex
~~~

Matching is case-sensitive in M05.

Regex inline flags remain explicit regex syntax; M05 adds no separate caseSensitive argument.

## 6.3 scope

Workspace-relative directory.

~~~text
default: "."
maximum UTF-8 bytes: 1024
~~~

Rules:

- absolute path rejected;
- parent/sibling escape rejected;
- scope must resolve to an existing directory inside Workspace;
- explicit reparse/symlink/junction scope rejected;
- caller never supplies a trusted physical path.

## 6.4 include / exclude

Arrays of Workspace-relative path glob patterns matched against normalized slash-separated paths from Workspace root.

~~~text
max include patterns: 32
max exclude patterns: 32
max UTF-8 bytes per pattern: 512
empty include: include all eligible files
exclude: evaluated after include and wins
pattern engine: existing globset semantics
~~~

Invalid patterns produce INVALID_FILTER_PATTERN; they are not ignored.

## 6.5 maxResults

~~~text
default: 100
minimum: 1
maximum: 1000
~~~

One result represents one matching logical line even if multiple matches occur on that line.

## 6.6 maxOutputBytes

~~~text
default: 65536
minimum: 1024
maximum: 262144
~~~

For deterministic accounting, match payload bytes are:

~~~text
UTF-8 bytes(path) + UTF-8 bytes(preview)
~~~

Scalar metadata and bounded diagnostics are separately bounded by fixed structural limits.

---

# 7. Result Contract

Successful result:

~~~text
{
  matches: [
    {
      path,
      lineNumber,
      preview,
      previewStartByte,
      matchStartByte,
      matchEndByte,
      previewTruncated
    }
  ],
  scannedFiles,
  scannedBytes,
  skippedFiles,
  truncated,
  truncationReasons,
  diagnostics,
  diagnosticsTruncated
}
~~~

## 7.1 Match semantics

- path is normalized Workspace-relative slash form.
- lineNumber is 1-based.
- byte offsets are zero-based within the logical line.
- one result per matching line.
- first match on the line defines matchStartByte/matchEndByte.
- preview is valid UTF-8 and includes the first match.
- maximum preview bytes per match = 4096.
- preview may be centered around the first match.
- previewTruncated=true when the full logical line is not returned.

The MCP result may contain bounded matching preview because the calling Model requested the search.

Durable evidence may not persist that preview.

## 7.2 Deterministic ordering

Matches are ordered by:

1. normalized Workspace-relative path, ascending by UTF-8 byte sequence;
2. lineNumber ascending.

Directory traversal must be deterministic:

- enumerate each directory;
- normalize/sort entries before descending;
- use deterministic depth-first traversal.

Filesystem enumeration order must not affect output.

---

# 8. Frozen Search Resource Limits

Production constants:

~~~text
MAX_QUERY_BYTES = 4096
MAX_FILTER_PATTERNS_PER_KIND = 32
MAX_FILTER_PATTERN_BYTES = 512
DEFAULT_MAX_RESULTS = 100
MAX_MAX_RESULTS = 1000
DEFAULT_MAX_OUTPUT_BYTES = 64 KiB
MAX_MAX_OUTPUT_BYTES = 256 KiB
MAX_PREVIEW_BYTES = 4096
MAX_DISCOVERED_ENTRIES = 50_000
MAX_SCANNED_FILES = 10_000
MAX_SCANNED_BYTES = 64 MiB
MAX_SINGLE_FILE_BYTES = 8 MiB
MAX_DIAGNOSTICS = 100
SEARCH_WALL_TIMEOUT = 5 seconds
REGEX_COMPILED_PROGRAM_LIMIT = 2 MiB
~~~

These are product safety bounds, not user-configurable AppConfig settings in M05.

The MCP caller can only choose maxResults and maxOutputBytes inside the frozen ranges.

---

# 9. Deterministic Truncation vs Timeout

Truncation is a successful bounded search.

Frozen truncation reasons:

~~~text
max_results
max_output_bytes
max_discovered_entries
max_scanned_files
max_scanned_bytes
~~~

Rules:

- stop in deterministic traversal/result order when the next item would exceed a frozen limit;
- return already accepted matches;
- set truncated=true;
- include applicable reason(s);
- unchanged tree + same arguments returns the same prefix.

Wall-clock timeout is different:

~~~text
error code: SEARCH_TIMEOUT
~~~

On wall-clock timeout:

- return an error;
- do not return partial matches as a successful result;
- avoid machine-speed-dependent random prefixes being presented as deterministic truncation.

---

# 10. File Eligibility and Encoding

Eligible content:

- regular files;
- size <= 8 MiB;
- valid UTF-8, with UTF-8 BOM accepted/ignored at file start;
- no NUL byte.

Skipped with typed bounded diagnostic:

- binary/NUL;
- invalid UTF-8;
- file too large;
- unreadable;
- reparse/symlink/junction candidate;
- target changed before/open;
- unsupported special file.

Per-file skip is not a whole-call failure unless requested scope itself is invalid/unsafe.

Diagnostics contain:

- logical relative path where safe;
- typed diagnostic code;
- no raw OS error;
- no physical path;
- no file content.

---

# 11. Literal and Regex Semantics

## Literal

- exact UTF-8 substring search;
- case-sensitive;
- no escape processing;
- searched independently within each logical line.

## Regex

- Rust regex syntax;
- no shell/PCRE external engine;
- explicit compiled-program memory limit;
- invalid or oversized compiled regex -> INVALID_REGEX;
- searched line-by-line;
- no cross-line matching;
- one result per matching line.

Regex execution uses a bounded/linear-time Rust regex implementation and the global scan/time limits.

Catastrophic-backtracking engines are forbidden.

---

# 12. Workspace and Windows Security Contract

## 12.1 Scope validation

Before traversal:

- use existing WorkspaceGuard semantics;
- canonical Workspace root remains authority;
- scope must be an in-Workspace directory;
- absolute/parent/sibling-prefix escapes fail closed.

## 12.2 Reparse/symlink/junction traversal

Traversal must not follow directory symlink/junction/reparse points.

Candidate file entries that are symlink/reparse are skipped.

This includes Windows reparse points.

## 12.3 TOCTOU

Pre-enumeration path validation alone is insufficient.

Immediately before bytes are consumed:

1. open candidate file with the platform-safe path/open behavior;
2. obtain/verify the opened handle's final resolved path;
3. verify final path remains under canonical Workspace root;
4. verify opened object is an eligible regular file;
5. only then read/search bytes.

On Windows, use opened-handle final-path verification equivalent to GetFinalPathNameByHandleW.

If the path is swapped between enumeration and open:

- outside Workspace -> reject without reading outside bytes;
- unsafe reparse/special object -> skip/reject;
- emit TARGET_CHANGED/WORKSPACE_VIOLATION-safe evidence as appropriate.

Implementation must test this race through a controllable hook.

A canonicalize-once-then-open-later implementation cannot satisfy F10-AC03.

---

# 13. Runtime Pipeline Contract

grep uses the normal Runtime chain.

Required:

- ToolRegistry registration;
- ToolConfig enabled state;
- ToolPolicyDefaults;
- user Policy Tool matcher;
- CapabilityGate;
- WorkspaceGuard;
- PolicyEngine;
- ApprovalManager for ASK;
- ExecutionPermit;
- executor;
- AuditEvent;
- correlation metadata.

Forbidden:

- calling grep executor before Policy/Approval;
- separate direct MCP fast path;
- direct filesystem access from MCP adapter;
- Bash fallback;
- read-only therefore bypass Policy logic.

When disabled:

~~~text
TOOL_DISABLED
~~~

When Policy DENY:

- executor does not scan;
- no file content is read.

When ASK:

- no scan starts before real Approval resolves;
- allow_once resumes the same invocation;
- deny does not scan.

---

# 14. Config Compatibility Contract

AppConfig remains:

~~~text
schemaVersion = 3
~~~

No v4 schema bump.

## 14.1 New installation defaults

A newly created post-M05 config:

~~~text
tools.grep.enabled = true
policies.toolDefaults.grep = allow
~~~

## 14.2 Existing pre-M05 configurations

Any existing v1/v2/v3 config that does not contain grep is normalized/migrated explicitly to:

~~~text
tools.grep.enabled = false
policies.toolDefaults.grep = allow
~~~

This preserves the pre-upgrade exposed Tool surface while making grep state explicit/editable.

Normal ConfigStore load may atomically persist this additive v3 shape upgrade.

Rules:

- schemaVersion remains 3;
- existing configRevision is preserved during migration itself;
- all existing fields/SecretReference/Policy rules remain unchanged;
- migration failure preserves original file;
- read-only diagnostics inspection may normalize in memory but must not write;
- after migration, user explicitly enabling grep changes restart semantics, advances configRevision and creates Restart Required.

## 14.3 Policy matcher

User Policy matcher Tool whitelist adds:

~~~text
grep
~~~

Old Policy rules are preserved.

Migration never invents a user grep rule.

---

# 15. Typed Errors and Diagnostics

Required MCP-facing typed failures include:

- INVALID_ARGUMENTS
- INVALID_REGEX
- INVALID_FILTER_PATTERN
- TARGET_NOT_FOUND
- INVALID_SCOPE
- WORKSPACE_VIOLATION
- TOOL_DISABLED
- existing Runtime Policy denial mapping
- existing Approval denial mapping
- SEARCH_TIMEOUT
- EXECUTION_FAILED for unexpected internal failure

Recommended per-file grep diagnostics:

- binary_skipped
- invalid_utf8_skipped
- file_too_large
- unreadable
- reparse_skipped
- target_changed
- unsupported_file

Errors/diagnostics must not include physical paths or raw OS messages.

---

# 16. Typed Activity and Durable Evidence

M05 extends existing closed evidence enums.

## 16.1 Safe argument evidence

Logical live/durable grep evidence:

~~~text
Grep {
  mode,
  logicalScope,
  queryBytes,
  includeCount,
  excludeCount,
  maxResults,
  maxOutputBytes
}
~~~

Do not persist:

- raw query;
- include/exclude pattern text;
- matched preview text;
- whole matching line;
- physical path.

## 16.2 Result evidence

~~~text
Grep {
  matchCount,
  scannedFiles,
  scannedBytes,
  skippedFiles,
  truncated
}
~~~

All fields are bounded scalars.

## 16.3 M03 compatibility

M03 SQLite physical schema remains version 1.

Typed JSON evidence variants extend protocol v1 additively.

History must:

- identify toolName=grep;
- preserve exact runtime_id + request_id;
- display safe typed facts;
- never expose raw query/match text from durable storage.

No M03 DB migration is required solely for grep.

---

# 17. MCP Tool Result Correlation

Like M04:

- every grep result/error has exact Runtime correlation metadata;
- runtime_id + request_id matches Activity and M03 evidence;
- structured business result remains the grep Tool result;
- no timestamp-nearest matching.

---

# 18. UI Contract

Reuse existing Tools page.

grep appears like other built-in Tools with:

- localized description;
- enabled switch;
- default Policy ALLOW / ASK / DENY;
- saved vs current Runtime state;
- Restart Required semantics;
- effective availability/decision/readiness.

No separate grep search UI is required in M05.

All new visible strings require en + zh-CN.

Activity/History rendering remains understandable without exposing raw query/match preview.

---

# 19. M04 Baseline Preservation

M05 must not modify:

- m04-core-v1 prompts;
- M04 fixture bytes;
- M04 ground truth;
- M04 baseline selection oracle;
- M04 deterministic final-state scorer semantics;
- M04 24 × 3 sample contract;
- M04 run limits;
- accepted M04 baseline aggregate;
- M04 comparator thresholds.

M04 baseline Runner mode must continue to reject an extra grep Tool in the baseline catalog.

This is a deliberate regression test.

---

# 20. M05 Eval Profiles

## 20.1 Strict regression candidate

Profile ID:

~~~text
m05-grep-candidate-v1
~~~

Exposed Tool catalog exactly:

~~~text
read_file
glob
grep
write
edit
~~~

bash remains disabled.

Dataset:

~~~text
m04-core-v1 unchanged
24 cases × 3 repetitions = 72 logical runs
~~~

Authoritative regression metrics use the original M04 case scorer/oracle unchanged.

This lane answers whether adding grep degrades existing Agent quality.

## 20.2 Paired control

Profile ID:

~~~text
m05-paired-control-v1
~~~

grep is disabled and not exposed.

Observed Tool catalog/schema must exactly match accepted M04 baseline.

Use this only when exact accepted M04 resolved Model identity is unavailable or provider drift must be checked.

## 20.3 Supplemental grep-admission set

A separate versioned supplemental set is added under:

~~~text
fixtures/v4/m05-eval/
~~~

Freeze:

~~~text
set_id: m05-grep-admission-v1
case_count: 4
repetitions: 3
logical_runs: 12
~~~

Neutral content-search tasks do not name a specific Tool and cover:

1. literal search across multiple files;
2. regex search;
3. relative scope + include/exclude;
4. bounded/truncated result then exact follow-up answer.

Purpose:

- observe whether controlled Agent discovers/selects grep;
- validate grep arguments under the same real Model/Runtime loop;
- diagnose ambiguity with glob/read_file.

This supplemental set never replaces strict 72-run regression and never changes M04 denominators.

Validity:

~~~text
scoreable runs >= 11 of 12
each case >= 2 of 3 scoreable
security violations = 0
~~~

No arbitrary absolute selection/task accuracy threshold is required; real grep Agent usability is also proven by required real ChatGPT E2E.

---

# 21. Frozen F12 Regression Gate

M05 reuses existing M04 comparator implementation/constants.

Let accepted M04 baseline = B and M05 strict candidate = C.

Required:

~~~text
Ctask >= Btask - 0.02
Csel  >= Bsel  - 0.03
Carg  >= Barg  - 0.01
Cout  >= Bout  - 0.01
Cunnecessary <= Bunnecessary + 0.05
~~~

Category guards:

~~~text
candidate category task >= baseline category task - 0.08
candidate category selection >= baseline category selection - 0.10
~~~

Stable case guards:

- baseline task 3/3 -> candidate task >= 2/3;
- baseline selection 3/3 -> candidate selection >= 2/3.

Safety:

~~~text
candidate security violations = 0
~~~

No M05 CLI option may widen these thresholds.

---

# 22. Provider / Model Drift

Prefer exact accepted M04 requested/resolved Model and adapter identity.

If exact resolved identity is unavailable:

1. run full paired-control profile against unchanged m04-core-v1;
2. compare paired control P with historical M04 baseline B;
3. use existing M04 drift guard:

~~~text
abs(Ptask - Btask) <= 0.05
abs(Psel  - Bsel)  <= 0.05
abs(Parg  - Barg)  <= 0.02
abs(Pout  - Bout)  <= 0.02
abs(Punnecessary - Bunnecessary) <= 0.05
~~~

If any guard exceeds:

~~~text
M05 Eval verdict = INCONCLUSIVE
~~~

Do not blame or credit grep for provider/model drift.

Paid/networked Model execution still requires explicit user authorization immediately before formal execution.

---

# 23. Accepted M04 Baseline Handoff

Formal M05 regression requires a machine-readable reference to the actually accepted M04 baseline.

Minimum handoff:

~~~text
M04 accepted candidate commit
Runner/runtime binary hashes
dataset hash
system prompt hash
scorer hash
eval profile hash
M04 Tool schema hash
adapter id/version
requested/resolved model
aggregate metrics with raw numerators/denominators
category metrics
per-case 0/3..3/3 counts
security violation count
baseline validity verdict
~~~

M05 must not synthesize a baseline from source constants if the real accepted M04 72-run result is missing.

If accepted M04 baseline result cannot be supplied:

~~~text
M05 F12 regression = BLOCKED
~~~

Do not create a seventh M04/M05 report file automatically.

---

# 24. Direct grep Functional Matrix

M05 implementation/validation covers at least:

## Search correctness

- literal one match;
- literal multiple files;
- regex simple;
- regex inline flag semantics;
- zero match;
- repeated matches on one line -> one line result;
- CRLF / LF line numbering;
- UTF-8 multibyte text;
- UTF-8 BOM;
- nested directories;
- relative scope;
- include only;
- exclude only;
- include + exclude where exclude wins;
- deterministic path/line ordering.

## Limits

- maxResults;
- maxOutputBytes;
- maxDiscoveredEntries;
- maxScannedFiles;
- maxScannedBytes;
- maxSingleFileBytes;
- maxDiagnostics;
- 4096-byte preview;
- 4096-byte query;
- 32 filters;
- invalid over-limit arguments;
- search timeout.

## File handling

- binary/NUL;
- invalid UTF-8;
- unreadable;
- special file where applicable;
- oversized file.

## Security

- parent escape;
- absolute path;
- sibling-prefix escape;
- Workspace-root reparse;
- nested symlink;
- junction;
- generic Windows reparse point;
- candidate swapped after enumeration;
- candidate swapped outside Workspace before open;
- Policy DENY;
- ASK allow_once;
- ASK deny;
- disabled Tool;
- Runtime cancellation/stop where applicable.

---

# 25. Responsiveness Contract

M05 does not freeze a throughput benchmark as a quality metric.

Safety/resource limits are the performance contract:

- 5-second wall timeout;
- 64 MiB max scanned bytes;
- 10,000 max scanned files;
- 50,000 discovered entries;
- 8 MiB/file;
- bounded output/results.

Async MCP path must not block a Tokio core worker for the whole filesystem scan.

Use a blocking worker/spawn_blocking or equivalent bounded execution boundary.

A slow search yields SEARCH_TIMEOUT/cancellation rather than making Runtime control/Approval state unresponsive.

---

# 26. Cancellation and Runtime Stop

Where current Runtime request cancellation is available:

- cancellation stops further traversal/read;
- terminal outcome is cancelled;
- no success partial result is produced;
- M03 evidence shows terminal state.

Runtime stop:

- cancels/terminates active grep within existing shutdown semantics;
- leaves no search thread/process orphan.

---

# 27. Privacy and Data-Minimization Invariants

M05 proves:

- no raw query in durable History;
- no matching line/preview in durable History;
- no include/exclude pattern text in durable History;
- no physical Workspace path in Activity/History;
- no raw OS error in diagnostics/evidence;
- no file content in grep Approval summary;
- no provider secret in Eval output;
- no hidden model reasoning in Eval;
- Tool result is bounded before return to Model;
- no unrestricted production debug log of match content.

---

# 28. M05 Derived Acceptance Criteria

| M05 AC | Requirement |
| --- | --- |
| M05-AC01 | grep MCP schema supports frozen query/mode/scope/include/exclude/maxResults/maxOutputBytes and rejects invalid/unknown arguments. |
| M05-AC02 | Literal/regex search returns correct 1-based line results with bounded preview and deterministic path+line ordering. |
| M05-AC03 | Frozen result/file/byte/discovery/output/preview/regex/time bounds are enforced; deterministic limits return deterministic truncation while wall timeout returns SEARCH_TIMEOUT without successful partial output. |
| M05-AC04 | UTF-8/BOM/binary/invalid-UTF8/oversize/unreadable semantics and bounded typed diagnostics work as frozen. |
| M05-AC05 | grep is registered as Low-risk WorkspaceRead and exposed through ToolRegistry/ToolConfig/ToolPolicyDefaults/MCP/effective Tool diagnostics. |
| M05-AC06 | New installs default grep enabled + ALLOW; existing configs missing grep normalize to explicit grep disabled + ALLOW without schemaVersion/configRevision drift; later user enablement creates restart debt. |
| M05-AC07 | grep uses existing ExecutionPipeline/WorkspaceGuard/Policy/Approval/Permit path; no shell/external process/direct filesystem MCP bypass exists. |
| M05-AC08 | disabled, Policy DENY, ASK allow_once and ASK deny preserve Runtime authority and executor does not scan before authorization. |
| M05-AC09 | Workspace escape and symlink/junction/reparse traversal are rejected/skipped and never read outside bytes. |
| M05-AC10 | Opened-handle/final-path TOCTOU protection prevents a candidate swapped outside Workspace from being read. |
| M05-AC11 | Typed MCP errors/results are bounded/sanitized; timeout/cancel/Runtime stop do not create successful partial results or orphan work. |
| M05-AC12 | Activity and M03 History use exact runtime_id + request_id and typed grep evidence; durable storage contains no raw query/filter/match preview/content and requires no M03 schema migration. |
| M05-AC13 | Tools UI exposes grep enabled/default Policy/effective state/restart semantics with complete en/zh-CN strings. |
| M05-AC14 | M04 baseline mode/dataset/prompts/fixtures/scorers remain identity compatible and continue rejecting grep catalog drift. |
| M05-AC15 | m05-grep-candidate-v1 runs unchanged m04-core-v1 at 24×3 with exact five-Tool candidate catalog and one stable candidate identity. |
| M05-AC16 | Existing M04 compare_m05 thresholds/category/stable-case/security guards pass; M05 does not reimplement/weaken comparator. |
| M05-AC17 | If accepted M04 resolved Model identity drifts, m05-paired-control-v1 uses exact M04 Tool catalog and existing drift guard; exceeding drift yields INCONCLUSIVE. |
| M05-AC18 | m05-grep-admission-v1 supplies 4 neutral supplemental cases ×3 with separate metrics and zero security violations without changing strict regression denominators. |
| M05-AC19 | Packaged real ChatGPT→Tunnel→Runtime discovers/calls grep for literal/regex/filter/truncation scenarios and Activity/History agrees with actual results. |
| M05-AC20 | M05 preserves read_file/glob/write/edit/bash, M03 evidence/history, M04 baseline mode, M01/M02 desktop/diagnostic behavior and V3 security semantics. |

---

# 29. Reserved Validation Traceability

Later validation documents keep these IDs.

| Upstream | M05 AC | Task | Integration | E2E | Manual |
| --- | --- | --- | --- | --- | --- |
| F10-AC01 | AC01–AC04, AC11 | T1/T2/T3 | M05-INT-01/02/03/04/05/06 | M05-E2E-01 | M05-MAN-01/02 |
| F10-AC02, A12 | AC05–AC08 | T1/T3/T4 | M05-INT-07/08/09/10 | M05-E2E-01/02 | M05-MAN-02/03 |
| F10-AC03 | AC03/04/09/10/11 | T2/T3 | M05-INT-04/05/11/12 | M05-E2E-02 | M05-MAN-02/03 |
| SEC-AC01/02/03 | AC12 | T3 | M05-INT-13 | M05-E2E-01/02 | M05-MAN-03 |
| A15/config | AC06/13 | T1/T4 | M05-INT-07/14 | M05-E2E-03 | M05-MAN-01/04 |
| F12 regression | AC14–AC18 | T5 | M05-INT-15/16/17 | M05-E2E-03 | M05-MAN-04 |
| External product + regression | AC19/20 | T6 | M05-INT-18 | M05-E2E-04 | M05-MAN-05 |

Reserved suites:

- Integration: M05-INT-01 through M05-INT-18.
- E2E: M05-E2E-01 through M05-E2E-04.
- Manual: M05-MAN-01 through M05-MAN-05.

---

# 30. Development Entry Gate

M05 implementation may begin only when:

1. all six M05 contract/validation documents exist and are reviewed;
2. M03 is ACCEPTED / FROZEN;
3. M04 is ACCEPTED / FROZEN;
4. accepted M04 baseline identity/result required by §23 is available;
5. no unresolved M04 comparator/correlation/privacy blocker exists.

The user's statement that M04 Development is complete permits M05 design work, not implementation.

---

# 31. Exit Gate

M05 may be marked ACCEPTED / FROZEN only when:

- M05-AC01 through M05-AC20 pass;
- grep function/schema/limits/truncation pass;
- Windows reparse + TOCTOU security tests pass;
- disabled/DENY/ASK Runtime pipeline tests pass;
- safe Activity/M03 History evidence passes;
- config compatibility/new-install behavior passes;
- packaged real ChatGPT grep E2E passes;
- strict 72-run m04-core-v1 candidate regression passes frozen M04 comparator;
- provider/model drift is within guard or exact model identity is used;
- supplemental grep-admission results are valid/reported separately;
- security violations are zero;
- no raw query/match content leaks into durable evidence;
- affected M03/M04/V3/M01/M02 regressions pass;
- Integration/E2E/Manual all PASS;
- no unresolved blocker remains.

A registered Tool, local executor test, shell grep, MCP 200 response or Eval-only synthetic success is insufficient.
