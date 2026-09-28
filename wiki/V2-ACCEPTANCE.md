# V2 Acceptance

```yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN
```

V2 已完成真实链路验收，进入冻结状态。V2 的能力边界保持只读：

- `read_file`
- `glob`
- Workspace confinement
- Policy / Audit
- Runtime MCP
- Tunnel
- Tauri lifecycle
- Tool exposure
- External ChatGPT E2E

External ChatGPT E2E 属于 **manual acceptance（人工验收）**，不是自动化测试通过的替代说法。验收链路覆盖 ChatGPT → Tunnel → Runtime MCP → `read_file` / `glob` → WorkspaceGuard / Policy / Execution → Activity，并确认停止时由 Tauri 收敛 Tunnel 与 Runtime 生命周期。

自动化测试只应声明其实际覆盖的 Runtime、Protocol、Tauri、UI、分发和安全边界；不能把 mock、进程存在、loopback HTTP 或本地测试冒充 External ChatGPT E2E。

V3 规划从 [`../docs-codex/v3/ROADMAP.md`](../docs-codex/v3/ROADMAP.md) 进入，所有 V3 能力仍为 `PLANNED / NOT IMPLEMENTED`。
