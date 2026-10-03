# M5 Final Acceptance — Controlled Bash

Status: **M5 — ACCEPTED / FROZEN**
Date: 2026-10-03

## Final gates

| Gate | Final result | Evidence |
| --- | --- | --- |
| Development / Unit | PASS | DEVELOPMENT_REPORT.md; artifacts/M5-Controlled-Bash/development/; final QA regression logs |
| Integration | PASS | artifacts/M5-Controlled-Bash/qa/integration-*-final.log: Rust test summaries report 0 failed; UI 5 files / 42 tests passed |
| Local Black-box E2E | PASS | artifacts/M5-Controlled-Bash/qa/m5-e2e-run-10.log: E2E-01–25 PASS; LOCAL_M5_BLACK_BOX_E2E=PASS |
| Manual Acceptance | PASS | Human Reviewer（用户）明确确认“M5 Manual Acceptance 已通过”；MANUAL_ACCEPTANCE_CHECKLIST.md |

无未关闭 FAIL / BLOCKED。早期调试/失败日志保留为历史证据，不代表最终 gate 状态。本次只核对已有自动化证据与产物哈希，不重跑 QA，不代替人工或 External ChatGPT 验收。

## Accepted Portable identity

- Portable: `artifacts/M5-Human-Approval/ChatGPT-Harness_0.1.1_x64-portable`（目录包，无 ZIP）。
- Acceptance build Git HEAD / automated evidence commit: `bc3c4a20f9ab2a699ba4790287771145579dac15`。
- Production implementation commit: `df586c489d7d49776694555987a0f5285e139979`。
- UI `chatgpt-harness-ui.exe` SHA-256: `7A2140D2E3A1ED368D5EFB349E88EBD4A1F519F601E745CF3D417FF181B98D65`。
- Runtime `runtime/chatgpt-harness-runtime.exe` SHA-256: `31404FD9EF3F3B2966B004BB2008BE015A0B18F80C7B95EF2E53413FBAB3708F`。
- QA Git Bash: `C:\Program Files\Git\bin\bash.exe`；SHA-256: `04FB7475D5F43BFCCD31A64587BD123AEC81E3863B2AC6B8AD08F8EF38E847B3`。
- Preparation/build/smoke evidence: `artifacts/M5-Human-Approval/PREPARATION_RECORD.md`, `HEAD.txt`, `build.log`, `package-smoke.log`, `runtime-smoke.jsonl`, `ui-startup-smoke.json`。
- Full package file inventory and per-file SHA-256: `artifacts/M5-Human-Approval/package-files.json`（10 files）。本次重新计算 UI / Runtime 哈希，与该清单及 E2E Runtime 身份一致。

上述 HEAD 是人工验收使用的构建来源；最终收口提交只修改文档，不改变该产物身份。DEVELOPMENT_REPORT.md 与 preparation record 的原始阶段描述保留；当前最终结论以本记录及已完成的 checklist 为准。

## Freeze boundary

M5 产品实现未修改。Integration、Local Black-box E2E、Manual Acceptance 均已 PASS；M5 — ACCEPTED / FROZEN。本次不启动后续 Milestone。
