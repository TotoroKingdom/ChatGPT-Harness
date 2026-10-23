# M9 Final Acceptance — Light Theme & Appearance

Status: **M9 — ACCEPTED / FROZEN**
Date: 2026-10-06
Scope: M9 Light Theme & Appearance only；不代表整个 V3 验收。

## Final gates

| Gate | Final result | Evidence |
| --- | --- | --- |
| Focused QA | PASS | `npm test`：37 个 test files / 356 个 tests PASS；`npm run typecheck` PASS；`npm run build` PASS；M9 相关测试 4 个文件 / 61 个 tests PASS |
| M9 Portable 构建 | PASS | `cd ui; npm run portable:windows`；官方 `verify-portable.ps1` PASS；version `0.1.2`；icon entries `6`；Runtime smoke PASS |
| Light Theme palette corrective | COMPLETE | `ui/src/styles/global.css`、`ui/src/theme/theme.ts`、`ui/src/theme/theme.test.ts` 的 corrective 改动已完成并通过 focused QA |
| 人工视觉验收 | PASS | 用户明确确认 M9 人工视觉验收通过；本记录不以自动化检查替代人工视觉判断 |

无未关闭的 M9 `FAIL` / `BLOCKED`。本次收口不继续修改 M9 功能，不启动 Full V3 External Acceptance。

## Accepted Portable identity

- Portable：`C:\AI\ChatGPT-Harness\artifacts\M9-Light-Theme-Appearance\ChatGPT-Harness_0.1.2_x64-portable`
- Main EXE：`C:\AI\ChatGPT-Harness\artifacts\M9-Light-Theme-Appearance\ChatGPT-Harness_0.1.2_x64-portable\chatgpt-harness-ui.exe`
- Main EXE SHA-256：`75DD459F34BCAC2126EF4A0D1E54AAEC87638515888B4E144C393FD1DD9BAE82`
- 文件数量：`10`
- `distribution.json`：`app_version=0.1.2`、`runtime_version=0.1.2`、`protocol_version=1`
- Portable 为目录包，无 ZIP。

## Smoke checks

### UI launch smoke

- 从上述最终 Portable 路径启动 `chatgpt-harness-ui.exe`。
- 主进程在 8 秒观察窗口后仍存活，进程路径与最终 artifact 主 EXE 精确匹配。
- 观察到 `1` 个 `msedgewebview2.exe` 子进程；未观察到 `localhost:1420` dev server URL。
- Smoke 结果：`PASS`；测试后匹配的 Harness UI 进程数量：`0`。

### Runtime smoke

- 从最终 Portable 路径启动 `runtime/chatgpt-harness-runtime.exe`。
- `initialize`：PASS。
- `runtime.status`：`lifecycle=ready`、`readiness=ready`。
- `runtime.stop`：`accepted=true`；退出码：`0`。
- Smoke 结果：`PASS`。

## Non-blocking observations

- Vite minified JavaScript chunk：`513.78 kB`，超过既有 `500 kB` warning threshold；构建与启动 smoke 均 PASS，属于非阻塞项。
- 既有 Rust `ts-rs`、unused/dead-code、linker warnings：不影响构建、Portable verifier 或 smoke 结果，属于非阻塞项。
- 首轮完整测试曾出现一次既有 M7 INT-33 文案断言时序失败；单独重跑该文件 `7/7 PASS`，随后完整重跑 `37/356 PASS`，未修改 M7 代码。

## Freeze boundary

```text
M9 — ACCEPTED / FROZEN
```

本结论仅冻结 M9 Light Theme & Appearance。不得据此将：

```text
V3 — ACCEPTED / FROZEN
```

写入项目状态；本次也不创建 GitHub Release 或执行发布上传。
