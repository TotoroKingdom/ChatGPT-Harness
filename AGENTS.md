# ChatGPT-Harness 工作规则

## Source of truth

判断当前系统实际行为时，按以下优先级：

```text
源码 / Tests / Manifests
        ↓
wiki/**
        ↓
docs-codex/**
        ↓
docs/archive/**
```

- `wiki/**` 描述 HEAD 当前系统事实。
- `docs-codex/**` 保存 SPEC / PLAN / TASKS 等开发规划，不得作为已经实现的证据。
- `docs/archive/**` 仅保存历史上下文；除非用户明确要求历史审计，否则不要读取。

## Working rules

- 默认只完成当前明确任务，不主动扩大范围。
- 未经用户明确授权，不使用 Computer Use；能通过代码、日志、CLI、接口或 MCP 获取的信息直接获取。
- 启动、构建、部署或验收 Runtime、Portable、服务实例、二进制前，必须确认实际运行产物来自本次修改，不能根据源码或旧进程推断验证成功。
- Windows 环境使用 PowerShell 兼容命令；优先复用已经验证可用的构建、启动和测试命令。命令失败后先定位原因，不原样重复。
- 主 Agent 保留核心问题、关键路径判断和最终整合。子 Agent 仅用于独立且适合并行的任务，不并行修改同一文件或强耦合模块。
- 不覆盖、不删除与当前任务无关的工作区修改。