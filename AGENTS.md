# ChatGPT-Harness 文档边界

## Source of truth

当前实现事实必须按以下优先级判断：

```text
源码 / Tests / Manifests
        ↓
wiki/**
        ↓
docs-codex/**
        ↓
docs/archive/**
```

- `wiki/**` 描述 HEAD 的当前系统事实，不是某个版本的历史快照。
- `docs-codex/**` 是当前开发计划与各 Milestone 的 SPEC / PLAN / TASKS，不得当作已经实现的能力。
- `docs/archive/**` 只保存历史上下文，不得作为当前实现证据。

除非用户明确要求历史审计，否则不要读取或使用 `docs/archive/**`。

## Version status

```yaml
V2 — Read-only Harness
Status: ACCEPTED / FROZEN

V3 — Writable Agent Runtime
Status: CURRENT DEVELOPMENT / PLANNED
```

V3 Roadmap 中的能力必须明确标记为 `PLANNED / NOT IMPLEMENTED`，当前 Wiki 不得把 V3 规划写成已存在的功能。
