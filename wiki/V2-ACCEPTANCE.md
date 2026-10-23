# V2 Acceptance — Historical Record

> **历史文档。** 本页只记录 V2 当时的验收边界，**不能用于判断当前 main / HEAD 的功能**。
>
> 当前实现已经包含 `write`、`edit`、`bash`、Tool Configuration、typed Policy、Approval、Session Grant、中文/英文与 light/dark/system theme 等后续能力。当前事实请从 [Wiki README](./README.md) 开始阅读。

```yaml
Historical milestone: V2 — Read-only Harness
Status at that time: ACCEPTED / FROZEN
```

## V2 当时的能力边界

V2 验收时的产品边界是只读：

- `read_file`
- `glob`
- Workspace confinement
- Policy / Audit
- Runtime MCP
- Tunnel
- Tauri lifecycle
- Tool exposure
- External ChatGPT E2E

当时的 External ChatGPT E2E 属于 **manual acceptance（人工验收）**，不是自动化测试声明。

验收链路：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ read_file / glob
→ WorkspaceGuard / Policy / Execution
→ Activity
```

并确认停止时由 Tauri 收敛 Tunnel 与 Runtime 生命周期。

## 如何使用这份历史记录

这份文件只适合回答：

- V2 当时验收了什么；
- V2 的只读安全边界是什么；
- V2 的 External ChatGPT E2E 是否经过人工验收。

它不适合回答：

- 当前有哪些工具；
- 当前 Policy / Approval 怎么工作；
- 当前 UI 有哪些页面；
- 当前版本是否仍是只读；
- 当前 V4 实现进度。

当前实现请看：

- [README.md](./README.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [IMPLEMENTATION-MATRIX.md](./IMPLEMENTATION-MATRIX.md)
- [MCP-AND-TUNNEL.md](./MCP-AND-TUNNEL.md)

历史 V3 文档位于 `docs/archive/`；V4 开发文档位于 `docs-codex/`。历史 milestone 结论不覆盖当前源码事实。
