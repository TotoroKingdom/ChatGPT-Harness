# ChatGPT-Harness

ChatGPT-Harness 是一个本机运行的安全执行 Runtime，通过 MCP 向 ChatGPT 暴露受控能力。

```yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN

V3 — Writable Agent Runtime
Status: CURRENT DEVELOPMENT / PLANNED
```

V2 的当前产品边界是 `read_file`、`glob`、Workspace confinement、Policy / Audit、Runtime MCP、Tunnel、Tauri lifecycle、Tool exposure，以及已人工验收的 External ChatGPT E2E。V3 只描述后续开发计划，不代表当前已经实现的能力。

## 文档优先级

```text
源码 / Tests / Manifests
        ↓
wiki/**
        ↓
docs-codex/v3/**
        ↓
docs/archive/**
```

- [`wiki/`](./wiki/README.md)：当前系统事实（current truth）。
- [`docs-codex/v3/ROADMAP.md`](./docs-codex/v3/ROADMAP.md)：当前 V3 开发计划；所有 V3 能力明确标记为 `PLANNED / NOT IMPLEMENTED`。
- [`docs/archive/`](./docs/archive/README.md)：历史上下文，不得作为当前实现证据。

V2 的人工验收边界和能力清单见 [`wiki/V2-ACCEPTANCE.md`](./wiki/V2-ACCEPTANCE.md)。
