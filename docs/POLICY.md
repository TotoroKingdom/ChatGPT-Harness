# M6-A Policy Engine

## 实现与边界

`runtime/src/policy` 是 Runtime 内部的确定性策略引擎，直接复用协议中的
`PolicyRule`、`PolicyDecision`、`PolicySource` 和 `SecurityMode`，不改变 wire contract（线协议）。

本阶段没有实例化或接入生产请求执行链：MCP HTTP → McpService → read_file/glob
adapter → 现有 executor；RuntimeCore 内部入口也继续调用现有 executor。
两条入口均保持既有 WorkspaceGuard 检查。`policies.list`、`tools.list` 和描述符中的
`effective_policy` 不因引擎的存在而成为实时策略计算结果。

M6-B 才负责统一执行管线的 enforcement（强制执行）和 Request State Machine（请求状态机）接线。
M6-A 不实现 approval（批准）、audit（审计）、有效工具目录推导或新工具，也不代表 M6 完成。

## 输入与能力来源

`PolicyContext` 只包含工具名、安全模式和解析后的逻辑 scope（范围），不包含路径、秘密或任意 arguments。
未来受验证的请求管线负责提供真实上下文；`active_workspace` 声明不等价于路径已通过安全检查。
WorkspaceGuard 始终是独立的硬安全边界，策略允许不能绕过它。

能力类型与现有 ToolRegistry 的工具描述符存放在同一 registration（注册项）中，
引擎不维护第二份工具名清单，也不读取描述符中的占位 `effective_policy`。
只有 implemented、enabled、exposed 均为 true 且 readiness 为 Ready 的注册项才能解析出能力。
当前只有 WorkspaceRead，对应既有 read_file/glob；未知、缺失或不可用工具拒绝。

## Scope 匹配

| 输入字符串 | 内部语义 |
| --- | --- |
| `active_workspace`、`workspace` | 当前工作区；后者是 M5 registry 的显式兼容别名 |
| `outside_active_workspace` | 当前工作区之外 |
| `developer_test_capabilities` | 已知 M0 范围，但当前没有实现对应能力 |

除此之外不接受其他字符串，不裁剪空格、不忽略大小写、不支持通配符或模糊匹配。
未知上下文范围产生 DENY / InvalidContext。
Normal 和 DeveloperTest 下的 WorkspaceRead 都仅匹配工作区内/外逻辑范围；
切换模式不会把读取工具变为 developer/test 能力，也不会自动放宽规则。

## 决策与证据

- `DENY > ASK > ALLOW`；规则 source 和 editable 不增加优先级。
- disabled 规则完全忽略，包括其无效 scope 和重复 ID。
- 先校验所有 enabled 规则，再匹配：任一未知 scope、空白 ID 或重复 enabled ID
  都使整个快照产生 DENY / InvalidRuleSet，即使该规则本来不匹配当前上下文。
- 合法范围精确匹配；没有规则、只有 disabled 规则或没有适用规则均产生 DENY / NoApplicableRules。
- 未实现能力或能力与范围不兼容产生 DENY / UnsupportedCapability。
- 匹配成功返回最严格的 canonical PolicyDecision、全部命中规则的 ID（字典序排序）和 MatchedRules。
  无效输入或无匹配返回空命中列表及对应原因，不返回敏感参数。
- 同一上下文和规则集合不受输入排列影响；评估不修改规则或注册表。
- ASK 仅是决策结果，不创建待批准请求或批准流程。

## System baseline（系统基线）

`PolicyEngine::system_rules()` 返回固定、enabled、不可编辑的两条规则：

- `system-read-workspace`：`active_workspace` → ALLOW。
- `system-deny-outside`：`outside_active_workspace` → DENY。

在现有能力约束下，基线允许工作区内 read_file/glob，拒绝工作区外请求。
用户 ALLOW 或 ASK 不能覆盖同范围的系统 DENY；用户 ASK 可以收紧工作区内 ALLOW。

`PolicyEngine::new(rules)` 使用精确传入的不可变快照，不隐式追加默认 ALLOW。
后续加载/合并用户策略的调用方必须保留固定系统基线，不能把任意用户规则集当成完整系统配置。
系统基线缺失与规则快照为空不是同一个概念：空快照拒绝；任意合法快照按其适用规则评估。
工作区外的硬拒绝仍由 WorkspaceGuard 独立保证，而非依赖调用方是否携带策略基线。
