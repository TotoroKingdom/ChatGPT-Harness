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

## 沿用既有开发与测试布局

| 内容 | 沿用的位置与做法 |
| --- | --- |
| Runtime 正式测试源码 | 对应源码的测试模块，或 `runtime/tests/`；参照 `runtime/src/m3_integration.rs`、`runtime/tests/m1_e2e.rs` |
| Tauri 正式测试源码 | `ui/src-tauri/` 对应模块；参照 `ui/src-tauri/src/runtime/m2_e2e.rs` |
| 前端正式测试源码 | 与功能代码相邻的 `*.test.ts` / `*.test.tsx` |
| 固定测试数据 | 复用 `fixtures/`、`protocol/fixtures/` 或模块已有测试数据位置 |
| 测试运行产生的临时工作区、文件、数据库 | 使用系统临时目录（Rust `std::env::temp_dir()` / `tempfile`，Windows 通常为 `%TEMP%`），创建唯一子目录，并在成功、失败及资源释放时清理；不得默认写入仓库或真实用户工作区 |
| 编译和打包中间产物 | 沿用构建脚本：Rust `target/`、前端 `ui/dist/`、分发暂存 `target-exe/distribution/`；不得扩展成一般杂项归档位置 |
| 交付验收的 Portable 便携包 | `artifacts/<里程碑>/<候选>/`，仅随附来源清单、散列和包校验记录 |

## 里程碑交付硬约束

- 仓库顶级目录结构固定，以 `ui/scripts/check-repository-layout.ps1` 白名单为准。未经用户明确授权，不得新增顶级目录，也不得自行扩展白名单；技术方案或 TASKS 中建议的路径不等于目录变更授权。优先使用既有模块目录。
- `runtime/`、`protocol/` 和 `ui/` 是正式产品模块，不是实验或杂项收纳目录。
-  已冻结的 SPEC、PLAN、TASKS 和验证计划属于实施契约。除非发现文档本身无法实施、与代码事实冲突，或用户明确批准需求/架构变更，否则 Coding/QA Agent 不得为记录执行结果、修复测试失败或达成验收而修改这些文档。