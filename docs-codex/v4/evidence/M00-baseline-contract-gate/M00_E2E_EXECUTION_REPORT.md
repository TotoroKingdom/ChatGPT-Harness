# M00 E2E Execution Report

> 本文件是独立 QA 执行记录，不是 `ACCEPTANCE.md`，不声明 `ACCEPTED / FROZEN`。

## Execution header

| Field | Value |
| --- | --- |
| Execution time | 2026-10-24 Asia/Shanghai |
| Candidate commit | `5fb8ca2d4286bcf5e201d8e1e4d2dfbd525c7e53` |
| Branch | `main` |
| Dirty state | `true`; only existing untracked POC target output remained in Git status |
| App / Runtime | `0.1.3` |
| Protocol | `1` |
| Tunnel | `0.0.13`; tunnel-client and cloudflared hashes recorded in the portable manifest |
| Portable | `target/release/bundle/portable/ChatGPT-Harness_0.1.3_x64-portable` |
| Portable verifier | version PASS; six icon entries; Runtime smoke PASS |
| Source state SHA-256 | `A2FE344BF9C29B86F5622A4D73891E4F9B5CE91FBC15C3082E5BA45DA1532A32` |
| Runtime SHA-256 | `E81F98BEB611BE262C60A85448DBFA7E6E103BDA08CDD519B7EFB873B332F98D` |
| UI SHA-256 | `CFC8B4FBDD935FAA1AEEC0833CA30724849E9BC05D47B5AF7C526639C983D6DE` |

## M00-E2E-01 — Real ChatGPT → Tunnel → Runtime MCP

**Verdict: `BLOCKED`**

Executed preparation and live-instance observation against the current candidate:

```powershell
npm run portable:windows
Start-Process <current portable> -WorkingDirectory <portable directory>
```

The candidate UI was observed through its accessibility tree. Runtime, Tunnel and MCP were all `已停止`. The action to start Harness returned:

```text
coordinate input geometry is unavailable
```

After re-observation the state remained stopped. A fresh screenshot retry returned:

```text
FrameArrived timed out: timed out waiting on channel
```

The test-owned UI process was stopped during cleanup. No Runtime/Tunnel process, external ChatGPT tool call, Tunnel `/poll`/`/response` evidence, Runtime `/mcp` request, Activity event, or Workspace result was produced in this execution.

No local MCP client, historical result, mock, or deterministic adapter was used to claim this case.

## M00-E2E-02 — Real controllable Agent/Model → Evaluation runner → Runtime MCP

**Verdict: `BLOCKED`**

No real provider/Agent/Model endpoint, model identity, authorized credential, or actual model turn was available for this execution. The local deterministic adapter from `runtime/tests/m00/agent-evaluation-poc/` was not used as a substitute for this case. No network or paid-model request was made.

Therefore there is no actual model tool-call, Runtime result, independent final-state scorer result, or negative-control result to record.

## M00-E2E-03 boundary

Not re-executed in this run, per QA request. The previously recorded E2E-03 PASS is not replaced or expanded by this report.

## Verdict and next gate

M00 E2E remains `BLOCKED` because E2E-01 and E2E-02 are not PASS. The conditions for entering Manual Acceptance are not satisfied. Manual Acceptance was not executed.

