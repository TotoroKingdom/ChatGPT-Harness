# V4 archive — 2026-11

本目录完整保留 ChatGPT-Harness V4 的产品规划、里程碑实施与验证计划、固定证据和开发记录，原有目录层级与历史内容均保留；仅修复因迁移导致失效的相对链接。归档文档仅供历史参考，不是当前实现证据。

> 路径说明：归档正文中仍出现的 `docs-codex/...` 是归档前的历史位置；M02 证据中的绝对路径也保持原样，以避免改变冻结哈希。实际文件均位于本目录。

## Root documents

- [V4 PRD](./ChatGPT-Harness_V4_PRD.md)
- [V4 ARCHITECTURE](./ChatGPT-Harness_V4_ARCHITECTURE.md)
- [V4 ROADMAP](./ChatGPT-Harness_V4_ROADMAP.md)

## Milestone documents

- [M00 — Baseline Contract Gate](./v4/milestones/M00-baseline-contract-gate/)
- [M01 — Tray, Background Lifecycle and Notifications](./v4/milestones/M01-tray-background-notifications/)
- [M02 — Startup Performance and Diagnostics](./v4/milestones/M02-startup-performance-diagnostics/)
- [M03 — Shared Evidence and SQLite History](./v4/milestones/M03-shared-evidence-sqlite-history/)
- [M04 — Offline Controlled Agent Evaluation Baseline](./v4/milestones/M04-offline-controlled-agent-evaluation-baseline/)
- [M05 — grep Tool and Quality Regression](./v4/milestones/M05-grep-tool-quality-regression/)
- [M06 — V4 Core Release Gate](./v4/milestones/M06-v4-core-release-gate/)
- [M07 — Auto Start on App Launch](./v4/milestones/M07-auto-start-on-app-launch/)
- [M08 — Tool Metrics and MCP Tool Request Trace](./v4/milestones/M08-tool-metrics-request-trace/)
- [M09 — File Tool Admission and Qualified Expansion](./v4/milestones/M09-file-tool-admission-qualified-expansion/)
- [M10 — MCP Protocol Compatibility and Conformance](./v4/milestones/M10-mcp-protocol-compatibility-conformance/)
- [M11 — V4 Full Release Integrated Acceptance](./v4/milestones/M11-v4-full-release-integrated-acceptance/)

## Evidence

- [M02 startup performance diagnostics](./v4/evidence/M02-startup-performance-diagnostics/)
- [M08 tool metrics and request trace](./v4/evidence/M08-tool-metrics-request-trace/)
- [M09 file tool admission](./v4/evidence/M09-file-tool-admission-qualified-expansion/)

## Historical status boundary

各 Milestone 的验收结论、未执行项和阻塞项以对应历史文件为准，归档过程未重写这些结论。归档时 M11 文档仍记录 `V4 — ACCEPTED / FROZEN: NO`，其 Integration / E2E / Manual 计划仍为 `PLAN ONLY / NOT EXECUTED`；M09 仍记录 admission blockers。本目录中的文档数量也保持归档时状态，未补造缺失的验证计划。

本次归档不代表产品验收、发布或版本冻结，也不创建 Release 或版本 tag。当前实现以源码、Tests、Manifests 和最新 `wiki/**` 为准；下一版本入口见 [`docs-codex/README.md`](../../../docs-codex/README.md)。
