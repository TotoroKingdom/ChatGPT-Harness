## V3 Development Workflow

V3 按 Roadmap 的 Milestone 顺序开发。

每个 Milestone 必须使用以下文档驱动流程：

```text
PRD + ARCHITECTURE + ROADMAP
        ↓
       SPEC
        ↓
       PLAN
        ↓
       TASKS
        ↓
Integration Test Plan
E2E Test Plan
Manual Acceptance Checklist
        ↓
Development
        ↓
Unit Tests
        ↓
Integration Tests
        ↓
E2E Tests
        ↓
Manual Acceptance
        ↓
ACCEPTED / FROZEN
```

### Document responsibilities

- `PRD`：定义产品需求、范围、行为和验收目标。
- `ARCHITECTURE`：定义技术边界、组件职责、数据流和关键技术决策。
- `ROADMAP`：定义 Milestone 的目标、顺序、依赖和 Gate。
- `SPEC`：定义当前 Milestone 最终必须实现的具体系统行为和接口约束。
- `PLAN`：定义如何基于当前代码实现 SPEC。
- `TASKS`：将 PLAN 拆成 Coding Agent 可独立执行和验证的开发任务。
- `Integration Test Plan`：验证多个 Task 组合后的 Feature / 子系统行为。
- `E2E Test Plan`：验证真实端到端业务链路。
- `Manual Acceptance Checklist`：定义 Milestone 的最终人工验收步骤。

### Development rules

1. 必须先读取当前源码、Tests、Manifests 和 `wiki/**`，再设计或执行 V3 Task。
2. `docs-codex/v3/**` 是开发目标，不是当前实现事实。
3. 不得为了满足 V3 文档而假设尚未实现的接口、模块或能力已经存在。
4. 每个 Task 必须包含对应 Unit Test 或明确的自动化验证。
5. Unit Test 不单独创建测试计划，直接属于 Task 的完成标准。
6. Integration Test、E2E Test 和 Manual Acceptance 必须独立于开发 Task。
7. 测试文档应在正式编码前准备完成，使实现目标和验收标准提前明确。
8. 一个 Task 完成不代表 Feature 完成；一个 Feature 完成不代表 Milestone 完成。
9. 只有完成该 Milestone 要求的：
    - Tasks
    - Unit Tests
    - Integration Tests
    - E2E Tests
    - Manual Acceptance

   后，Milestone 才能标记为：

```text
ACCEPTED / FROZEN
```

### External E2E rule

任何要求真实 ChatGPT、Tunnel 或外部系统参与的 E2E，都必须使用真实外部链路完成。

以下结果不得冒充 External E2E：

- Unit Test
- Integration Test
- Mock
- Local HTTP call
- MCP Inspector
- Loopback test
- Runtime process started
- Tunnel process started
- Tool successfully registered

如果外部平台能力暂时不可用，应标记：

```text
BLOCKED_BY_EXTERNAL_CAPABILITY
```

不得标记为 `PASS` 或 `ACCEPTED`.

### V3 M0 special gate

V3 M0 是后续 V3 开发的 blocking gate。

在真实 External ChatGPT 完成以下链路验收前：

```text
ChatGPT
→ Tunnel
→ Runtime MCP
→ write / edit
→ Workspace filesystem
```

不得开始依赖 M0 结果的后续 V3 Milestone 开发。

M0 只有完成真实 `write` 和 `edit` External E2E 后才能标记：

```text
ACCEPTED / FROZEN
```