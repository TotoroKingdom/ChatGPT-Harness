# M0 E2E Evidence — Write/Edit Capability

Date: 2026-09-29 (Asia/Shanghai)

## Current status

`PASS`

E2E-01 through E2E-05 were completed through the real External ChatGPT → Tunnel → Runtime MCP path. The user confirmed the corresponding External ChatGPT session and Runtime Activity/Audit evidence; filesystem outcomes were independently verified by Codex.

## Preflight evidence

- Dedicated Workspace: `C:\AI\M0-E2E-Workspace-20260929`.
- Initial fixture: `baseline.txt` with logical content `baseline`.
- Dedicated outside target: `M0-E2E-Outside-20260929.txt`; absent before E2E.
- Harness config points to the dedicated Workspace, Normal mode, the configured Tunnel ID, and the existing OS credential reference. The raw credential was not read or recorded.
- Updated Harness UI process is running.
- Updated Runtime process is running from the rebuilt Portable distribution.
- Updated Tunnel client process is running from the rebuilt Portable distribution.
- Runtime MCP loopback endpoint: `http://127.0.0.1:62479/mcp`.
- Tunnel health endpoint: `http://127.0.0.1:62480`; `/readyz` returned HTTP 200 `ready` and `/healthz` returned HTTP 200 `live`.
- Runtime MCP read-only preflight returned HTTP 200 and exposed exactly `read_file`, `glob`, `write`, `edit`, with write/edit annotations `readOnlyHint=false`, `destructiveHint=true`, `openWorldHint=false`.
- External ChatGPT write/edit/boundary/read actions were subsequently reported by the user; see the case sections below.

## E2E-01 — External tool discovery

- User confirmed that the real External ChatGPT session discovered and could invoke `read_file`, `glob`, `write`, and `edit`.
- The subsequent real tool calls and corresponding Activity/Audit evidence confirm that discovery was usable, not merely a local catalog result.

## E2E-02 — External golden write

- User confirmed a real External ChatGPT `write` tool call created `v3-write-test.txt`.
- Filesystem verification recorded the exact content `hello from ChatGPT`.
- Corresponding Runtime Activity/Audit success evidence was confirmed by the user.

## E2E-03 — External exact edit (current evidence)

- User reported that the real External ChatGPT `edit` tool call completed.
- Filesystem verification after that report: `v3-write-test.txt` is exactly `hello from V3` (13 UTF-8 bytes).
- The dedicated outside target remains absent.
- Runtime/Tunnel session remained live and Tunnel `/readyz` returned HTTP 200 `ready`.
- Corresponding External ChatGPT session and Runtime Activity/Audit success evidence was confirmed by the user, including the successful exact edit.
- Exact replacement evidence recorded `replacements=1`.

## E2E-04 — External boundary rejection (current evidence)

- User reported a real External ChatGPT MCP call to `../M0-E2E-Outside-20260929.txt`.
- Tool result reported: `is_error=true`, code `WORKSPACE_VIOLATION`, sanitized message `Requested path is outside the active workspace.`
- Filesystem verification: the outside target remains absent and `v3-write-test.txt` remains exactly `hello from V3`.
- Runtime/Tunnel session remained live and Tunnel `/readyz` returned HTTP 200 `ready`.
- Corresponding Runtime Activity/Audit evidence was confirmed by the user: Workspace validation failed and no successful `ExecutionCompleted` evidence was present.

## E2E-05 — External read regression smoke (current evidence)

- User reported the real External ChatGPT `read_file` result as exactly `hello from V3`.
- Filesystem verification: `v3-write-test.txt` remains exactly `hello from V3` (13 UTF-8 bytes).
- Runtime/Tunnel session remained live and Tunnel `/readyz` returned HTTP 200 `ready`.
- Corresponding External ChatGPT session and Runtime Activity/Audit smoke evidence was confirmed by the user.

## Stale artifact failure and repair

- The first Ready session used an old Portable Runtime artifact whose local MCP `tools/list` exposed only `read_file` and `glob`. This was classified as a Harness artifact mismatch, not External ChatGPT capability evidence.
- The session was stopped before any mutation or External ChatGPT call.
- `ui/scripts/build-portable.ps1` rebuilt the current Runtime and UI and restaged the distribution. The first verifier run found an old ZIP archive in the portable output root; that stale archive was moved out of the output directory and the verifier was rerun.
- Portable verifier result: PASS (manifest/version/icon checks and Runtime smoke test).
- Direct smoke of the updated Portable Runtime returned the four expected registry tools: `read_file`, `glob`, `write`, `edit`.

## E2E decision

E2E-01 through E2E-05: PASS. All required External ChatGPT, filesystem, and Activity/Audit evidence was confirmed. Local MCP preflight was used only for preparation and was not used as External E2E evidence.
