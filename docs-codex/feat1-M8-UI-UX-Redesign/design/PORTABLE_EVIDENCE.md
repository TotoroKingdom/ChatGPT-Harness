# M8 P1 — Fresh Portable Evidence

## Result

```text
M8 P1 Portable — PASS
```

P1 仅覆盖当前源码构建、staging verifier、独立副本 provenance 和最小启动 smoke。

```text
H1 Manual UX Acceptance: NOT RUN / MANUAL_PENDING
F1 M8 ACCEPTED / FROZEN: NOT RUN
M9 External ChatGPT Acceptance: NOT STARTED
```

## 1. Source identity

- Build source HEAD: `87cfd7bef5de12060a446b27f159c4c7c27d10a8`
- Branch: `main` (`origin/main`)
- Working tree before build: clean; `git status --short` produced no output.
- QA source HEAD from `QA_EVIDENCE.md`: `506fd2a0e321682b0d8392b5984c974052377b94`
- QA result: M8 Q1 Automated QA PASS.
- QA-to-build HEAD difference: only `A docs-codex/feat1-M8-UI-UX-Redesign/QA_EVIDENCE.md`.
- Production-path difference after Q1: none (`ui/src`, `ui/src-tauri`, `runtime`, `protocol`, Cargo files, and UI package manifests were unchanged).

因此，当前 HEAD 与 Q1 QA 使用的 production source 一致；HEAD 的变化仅是将 QA evidence 记录加入仓库。

## 2. Build

- Build command: `cd ui; npm run portable:windows`
- Actual formal script: `ui/scripts/build-portable.ps1`
- Portable Tauri build: `npx tauri build --no-bundle --ci --features custom-protocol --config src-tauri/tauri.portable.conf.json`
- Build start observed: `2026-10-05 12:44:17 +08:00`
- Build command result: PASS, exit 0
- Known staging root: `C:\AI\ChatGPT-Harness\target\release\bundle\portable\`
- Staging Portable:
  `C:\AI\ChatGPT-Harness\target\release\bundle\portable\ChatGPT-Harness_0.1.1_x64-portable`
- Staging verifier command:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\ui\scripts\verify-portable.ps1 -PortableDirectory C:\AI\ChatGPT-Harness\target\release\bundle\portable\ChatGPT-Harness_0.1.1_x64-portable`
- Staging verifier result: PASS, exit 0; version `0.1.1`; icon entries `6`; Runtime smoke PASS.
- Staging verifier observed: `2026-10-05 12:46:06 +08:00`

## 3. Isolated M8 artifact

- Final artifact:
  `C:\AI\ChatGPT-Harness\artifacts\M8-UI-UX-Redesign\build-20261005-124625-87cfd7b`
- Artifact copy observed: `2026-10-05 12:46:25 +08:00`
- Existing milestone directories were not reused or overwritten.
- File count: `10`
- Staging/final relative-path list: identical
- Staging/final file sizes: identical
- Staging/final full-file SHA-256: identical
- Staging/final manifest bytes: identical
- Manifest SHA-256: `B3ADEF887386FFD8728730CA211B897E054D0CF4A8CFCFF5C0E89A48F1DCF942`

Manifest identity:

- `distribution_schema_version`: `2`
- `distribution_id`: `chatgpt-harness`
- `app_version`: `0.1.1`
- `runtime_version`: `0.1.1`
- `protocol_version`: `1`
- Runtime path: `runtime/chatgpt-harness-runtime.exe`
- Tunnel client path: `tunnel/tunnel-client.exe`
- Cloudflared path: `tunnel/cloudflared.exe`
- Tunnel client version: `0.0.13`

### Final artifact files

| Relative path | Bytes | SHA-256 |
| --- | ---: | --- |
| `chatgpt-harness-ui.exe` | 10893312 | `38E8CECA5557269CEE3FDB6D9E5898E9B6CD38CB842EF3CCA646B8CD54959ED0` |
| `distribution.json` | 518 | `B3ADEF887386FFD8728730CA211B897E054D0CF4A8CFCFF5C0E89A48F1DCF942` |
| `runtime/chatgpt-harness-runtime.exe` | 8067072 | `90CD62C0081366386A933EA919CDC490132FB76FD90C72DD033AA103EE0DB0CB` |
| `tunnel/cloudflared-manifest.json` | 1059 | `0C30BCCDA73FD6D2F81BD60F748C8DE805F09C9482364584C8C28C49EA7AD219` |
| `tunnel/cloudflared.exe` | 39751680 | `CD0BC49AEC24D0D62A37B6E842DAD46000F7B401F6167B4CBACE4E170819C9A2` |
| `tunnel/LICENSE` | 10945 | `6507C654AA26B7F8BEDA9BC86C955F8B6FF77350CEBF6663FAFA8974C8AE5A7C` |
| `tunnel/NOTICE` | 564 | `D5A275A75B91D294AF22FE98B43A122AB4510CD4F922A7910E84D1C2AD97F811` |
| `tunnel/tunnel-client-v0.0.13-windows-amd64-licenses.txt` | 13574 | `1B0D874470433A7DCC1BA1F7FD121D328D3FD8638CB9D094526CB76A2D2AF879` |
| `tunnel/tunnel-client-v0.0.13-windows-amd64.spdx.json` | 240128 | `A80D4C4C09836C97258740C4B0E975AA38BAFBA9A61F2C59DBE5D27646151E01` |
| `tunnel/tunnel-client.exe` | 21753856 | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |

## 4. Final-package launch smoke

Smoke used only the final artifact UI executable:

- UI executable:
  `C:\AI\ChatGPT-Harness\artifacts\M8-UI-UX-Redesign\build-20261005-124625-87cfd7b\chatgpt-harness-ui.exe`
- Main process path matched the final UI path exactly.
- Main UI process remained alive after 8 seconds.
- `msedgewebview2.exe` child process observed.
- Temporary CDP target URL: `http://tauri.localhost/`
- `localhost:1420` dev-server URL: not observed.
- This confirms the final executable launched with embedded frontend/custom protocol; it is not a visual or manual UX acceptance.
- The smoke process was closed after observation; post-smoke UI process count: `0`; Runtime process count: `0`.

Final-package Runtime smoke:

- Runtime executable:
  `C:\AI\ChatGPT-Harness\artifacts\M8-UI-UX-Redesign\build-20261005-124625-87cfd7b\runtime\chatgpt-harness-runtime.exe`
- initialize response: PASS, protocol version `1`, version `0.1.1`
- `runtime.status`: `lifecycle=ready`, `readiness=ready`, `activeRequests=0`, `pendingApprovals=0`
- `runtime.stop`: `accepted=true`
- Runtime process exit: `0`
- Post-smoke Runtime process count: `0`

## 5. Known observations

- Vite minified JavaScript chunk: approximately `510.72 kB` (`510715` bytes), above the existing `500 kB` warning threshold. Build and launch smoke passed; no functional or actual performance failure was observed. Non-blocking.
- Existing Rust `ts-rs`, dead-code, and linker warnings were emitted; they did not change the successful build/verifier/smoke results.
- Runtime/UI smoke does not cover viewport layout, visual hierarchy, reduced motion, complete keyboard feel, or human UX judgment.

## 6. Gate boundary

```text
M8 P1 Portable — PASS
Ready for H1 — Manual UX Acceptance
M8 is NOT ACCEPTED / FROZEN
M9 NOT STARTED
```
