# M09 开发记录（2026-11-07）

本次授权允许在 M04/M05 尚未 ACCEPTED/FROZEN、M09 三份正式验证文档尚未补齐时，先实施所有独立开发工作。该授权不豁免真实模型 Admission、有效 M05 质量交接证据或 F12 回归门禁。

源码起点：`384905c`。SPEC.md、PLAN.md、TASKS.md 不修改；M04/M05 固定数据、评分语义、比较阈值和生产工具 Schema 不修改。

## 任务边界

- T1：实现评估专用 Profile（配置档）、候选 Schema（结构约束）、数据集及脚本机械检查；执行结果见补充记录。
- T2：BLOCKED。尚无有效 accepted M05 机器可读质量交接文件；未执行付费/联网真实模型调用。补充严格交接校验，复用既有 M05 比较器及 provider drift（提供方漂移）比较器。
- T3–T5：BLOCKED，等待 T2 最终 ADMIT 决定。没有依据推测准入子集，没有提前编写或暴露生产候选工具。
- T6：仅推进不依赖准入结果的既有工具回归、协议生成、布局检查及开发 Portable 构建。最终 72-run 模型质量回归、准入 Schema 对齐及 admitted 工具端到端验证仍 BLOCKED。

六个候选 mkdir / list_directory / copy / move / rename / delete 均未获得 ADMIT。`admission-blocked.json` 仅为阻塞记录，不是最终 decision-set-v1，不冻结候选最终结论，也不能被用于生产授权。

生产目录继续为 read_file / glob / grep / write / edit / bash。五工具受控质量目录排除 bash。AppConfig 保持 schemaVersion 3，Control Protocol 保持 v1。

## 验证与交付

完整开发检查及开发包记录将在构建后补充。默认正式里程碑布局检查仍要求六份文件；开发例外仅适用于本次显式授权的 M09，不能报告正式布局通过。

正式 Integration：NOT EXECUTED；正式 E2E：NOT EXECUTED；真实 External ChatGPT：NOT EXECUTED；Manual：NOT EXECUTED。

M09 Development Complete：NO（必须有真实准入及最终质量门禁）。M09 ACCEPTED/FROZEN：NO。

## 执行能力的实际边界

`m09_eval` 当前提供离线数据集校验、结构矩阵和 M05 handoff 预检。`m09_runner` 是 ScriptedAdapter 机械评分器，观察传入的调用、结果和文件系统快照，不执行候选文件操作。它不能作为真实 Runtime 候选 Admission 的替代。

真实候选 Profile 的执行管线尚未落地：现有生产 Runtime 仍只有原有六工具，无法通过候选 Profile 的精确目录校验。即使补齐模型授权和 accepted M05 文件，也不能把本次实现描述为已支持全部 108 次真实候选运行；该执行接线仍需后续实现、开发级验证，再进入 T2 真实运行。旧 `m04_eval run` 对所有 M09 Profile 提前返回 BLOCKED，避免通过旧入口误标质量结果。

## 并发集成变化

开发期间，另一个工作流把 M10/M11 文档及 M10 实现合入 `main`（观测到 `7833e8a`）；这些提交不是本次 M09 修改，予以保留。其 MCP 客户端兼容适配仍提供评分器使用的 typed in-process view；M09 不修改冻结 M04/M05 scorer。

M10/M11 当前也仅有三份规划文档。因此默认全局六文档布局检查仍会阻塞；显式 M09 开发包模式仅检查本次里程碑的 SPEC/PLAN/TASKS 及目录合法性，不把其他里程碑视为通过，也不豁免它们的正式验证门禁。该模式和交付清单均明确 development-only（仅开发），正常正式检查保持全局严格规则。

全局布局实测：默认脚本首先在 M08 缺少三份正式验证计划处停止（M09/M10/M11 亦尚未补齐），结果为 BLOCKED；M09 显式开发模式通过目标范围的目录/三份契约检查，输出明确说明未验证全局布局。没有为通过检查创建占位正式验证计划。

## 最终开发级检查（打包前）

- `cargo fmt --all -- --check` / `cargo check --workspace`：PASS。
- `cargo test --workspace`：866 PASS，27 ignored（既有显式正式 QA、发布包 E2E、性能探针，未另行执行）。包含协议生成物一致性测试。
- UI `npm run typecheck` / `npm test` / `npm run build`：PASS；41 个测试文件、385 项测试通过。
- 版本化数据集：36 cases / 108 logical runs，hash `dd81c84f8753adcb8eb69622dc414a840e80f213772e64de12a77b2c8fcafc50`，仅机械验证。
- `m09_eval prepare-fixtures`：35 PREPARED，1 BLOCKED（`list_directory-p3`；当前 Windows 无法创建 native reparse fixture）。该案例保留真实需求，没有以普通目录冒充。
- 152 份受保护文件的原始字节哈希一致。正式全局布局 BLOCKED，M09 范围的开发目录检查及 5 项隔离防护探测通过。

可复现的离线命令：

```powershell
cargo run -p chatgpt-harness-runtime --bin m09_eval -- validate
cargo run -p chatgpt-harness-runtime --bin m09_eval -- prepare-fixtures
cargo run -p chatgpt-harness-runtime --bin m09_eval -- structural
```

第二条在 reparse 权限不足时退出码为 2，并输出 BLOCKED JSON；不是准入失败，也不是准入通过。
