# Repository Guidelines

## 项目结构与职责

- `protocol/`：共享控制协议、生成的 TypeScript 类型、JSON Schema（结构约束）、样例与契约测试。
- `runtime/`：MCP 服务、工作区隔离、工具执行、策略、审批和审计；集成测试位于 `runtime/tests/`。
- `ui/src/`：React 界面及前端测试；`ui/src-tauri/`：Tauri 桌面宿主、配置、凭据和进程管理。
- `ui/scripts/`：Windows 打包与验证；`fixtures/`、`assets/`：测试数据和媒体资源。
- `docs-codex/`：当前需求及里程碑文档；`wiki/`：技术参考；`docs/archive/`：历史资料。

UI 展示状态并表达操作意图，Tauri 管理桌面进程，Runtime 负责授权和执行；Tunnel 只负责传输。

## 构建、测试与本地开发

准备 Node.js 20+、Rust 工具链及 Tauri 2 的 Windows 构建依赖。

仓库根目录执行：

```powershell
cargo check --workspace        # 检查 Rust 工作区
cargo fmt --all -- --check     # 检查 Rust 格式
cargo test --workspace         # 运行 Rust 测试
```

进入 `ui/` 后执行：

```powershell
npm install                   # 安装依赖
npm run dev                   # 启动浏览器前端
npm run tauri:dev              # 构建调试 Runtime 并启动桌面应用
npm run typecheck             # TypeScript 类型检查
npm test                      # 单次运行前端测试
npm run build                 # 类型检查并构建前端
npm run portable:windows      # 构建 Windows 便携版
npm run tauri:build            # 构建并验证 NSIS 安装包
```

打包还需构建脚本指定的 Runtime/Tunnel 分发资源。

## 代码风格与命名

Rust 使用 rustfmt 默认格式、四空格缩进；模块和函数采用 `snake_case`，类型采用 `PascalCase`。TypeScript 使用严格模式、两空格缩进、双引号和分号；React 组件采用 `PascalCase`，函数及变量采用 `camelCase`。沿用 `@/` 和 `@protocol/` 导入别名。当前未配置 ESLint 或 Prettier 脚本。协议变更需同步生成类型、结构约束、样例和契约测试。

## 测试要求

前端使用 Vitest、React Testing Library 和 jsdom；测试命名为 `*.test.ts` 或 `*.test.tsx`，通常与功能代码相邻。Rust 使用内置测试框架，协议契约测试位于 `protocol/tests/`。可用 `npm test -- src/App.test.tsx` 或 `cargo test -p chatgpt-harness-runtime` 缩小验证范围，再执行受影响的回归检查。当前未设置统一覆盖率门槛。

本地测试通过不能证明外部 ChatGPT 连接成功；分别记录自动化验证、真实外部调用和人工验收证据，明确未执行项。

## 提交与合并请求

沿用历史提交格式，如 `fix(tunnel): ...`、`feat(scope): ...`、`docs: ...`、`test: ...` 和 `release: ...`。每次提交聚焦一个变更。合并请求说明问题、行为变化、关联 Issue（如有）、验证命令及结果；界面变更附截图，协议或安全变更说明兼容性和风险。保留无关本地修改，仅暂存明确路径。

## 安全与文档依据

不得绕过工作区隔离、策略或审批；凭据沿用 SecretStore（凭据存储）及引用流程，不写入代码、日志、配置样例或验证证据。测试使用临时目录和合成凭据。

实现状态以源码、测试、清单和构建脚本为准；规划文档不代表功能已发布。里程碑开发先阅读当前需求与验收计划，不为通过测试降低标准。README 保持中文优先、面向用户；阶段性规则和验收记录放入对应里程碑文档。

## 里程碑交付硬约束

- 仓库顶级目录结构固定，以 `ui/scripts/check-repository-layout.ps1` 白名单为准。未经用户明确授权，不得新增顶级目录，也不得自行扩展白名单；技术方案或 TASKS 中建议的路径不等于目录变更授权。优先使用既有模块目录。
- 新增代码先参照项目已有布局，按所属模块就近存放：Runtime 测试放 `runtime/tests/` 或相应 `runtime/src/` 测试模块，Tauri 测试放 `ui/src-tauri/` 对应模块，打包验证脚本放 `ui/scripts/`。M00 的两个独立验证原型放 `runtime/tests/m00/`，保持独立 Cargo workspace，不得成为生产依赖；不把这一特例扩展成所有验证代码都归 Runtime 的规则。
- 需保留和维护的验证源码、脚本及测试输入不得放入 `target/` 或 `artifacts/`。`target/` 仅放可重新生成、可丢弃的编译缓存、临时测试副本和临时日志；不得通过搬到缓存目录来代替源码归属整理。固定测试数据沿用 `fixtures/` 或模块既有测试数据位置。不得在根目录散落临时文件、日志和报告。
- `artifacts/` 专用于按里程碑交付的 Portable 便携包及其随包来源清单、文件散列和包校验记录。禁止存放原型源码、测试临时目录、一般执行日志、截图、审计报告、目录清理归档或其他杂项；不得把它当作通用输出目录。
- 每条实现任务开始前明确将新增/修改的文件路径；清理归并本任务产物并运行目录检查后才能收尾。发现历史目录混乱时先核对用途与引用，禁止擅自删除无关内容。

- 一次只推进用户指定的当前里程碑；先冻结 SPEC、PLAN、TASKS 及三份测试/验收计划，再实施任务。未获用户明确人工验收通过，不得启动下一里程碑，也不得自行宣告 ACCEPTED / FROZEN。
- `docs-codex/v4/milestones/<milestone>/` 只允许六份文件：`SPEC.md`、`PLAN.md`、`TASKS.md`、`INTEGRATION_TEST_PLAN.md`、`E2E_TEST_PLAN.md`、`MANUAL_ACCEPTANCE_CHECKLIST.md`；不允许报告、日志、截图、fixtures 或子目录。
- 报告、ADR、证据索引及最终验收记录统一保存到 `docs-codex/v4/evidence/<milestone>/`，更新已有记录，禁止每次重试新增总结报告。需保留的脱敏验收日志/截图放该证据目录；临时日志/截图放 `target/`；测试数据放 `fixtures/v4/<milestone>/`。计划只引用结果，不追加执行流水。
- 开发与适用的本地自动化检查完成后，必须运行 `ui/scripts/deliver-milestone.ps1 -Milestone <Mxx-short-name>`，从当前代码重新构建独立 Portable（便携包），交付到 `artifacts/V4-<milestone>/<candidate>/`。禁止仅交付 `target/` 路径、复用旧包或覆盖已有候选。
- 交付记录必须包含源码 commit、dirty 状态、源码状态散列、文件 SHA-256、构建/验证结果、验收入口和未执行项；修改代码后必须重新构建。Portable 验证通过不代表真实 External ChatGPT E2E 或人工验收通过。
- E2E BLOCKED 时仍可交付明确标记为 BLOCKED 的候选供排障，但不得标记为待正式人工验收或解锁下游。外部调用及受管实例重启继续遵守当前授权边界。
- 收尾必须运行 `ui/scripts/check-milestone-layout.ps1`，报告候选绝对路径、验证结果、阻塞项及等待用户验收的状态。不得用新增报告代替打包交付。
