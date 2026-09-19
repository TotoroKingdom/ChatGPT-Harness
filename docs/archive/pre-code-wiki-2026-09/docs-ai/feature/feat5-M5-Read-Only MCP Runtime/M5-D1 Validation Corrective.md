实施 M5-D1 Validation Corrective。

目标：

只清除 M5-D1 当前剩余的 validation blocker，使：

- `cargo test --workspace`
- `git diff --check`

真正通过。

不要进入 M5-D2。
不要实现 HTTP。
不要修改 MCP 功能语义。
不要修改 WorkspaceGuard 安全语义。
不要进入 M6。

---

## 1. 先检查当前 diff

检查本轮 M5-D1 全部修改，尤其：

- `runtime/src/mcp/**`
- `runtime/src/state.rs`
- `runtime/src/main.rs`
- `runtime/Cargo.toml`
- 两处 Windows test 修正

确认 Windows test 修正：

- 没有删除安全测试
- 没有弱化 assertion
- 没有通过 skip/ignore 绕过测试
- 没有放宽 symlink/junction/workspace escape 语义

如果存在上述情况：

停止 corrective，
汇报具体问题。

---

## 2. 修复 generated JSON Schema CRLF/LF 不一致

当前：

`cargo test --workspace`

仅因 protocol generated JSON Schema 的 line-ending mismatch 失败。

先定位：

- 哪些 generated 文件 mismatch
- generator 实际生成内容
- repository committed 内容
- mismatch 是否仅为 CRLF/LF

要求：

如果语义内容完全一致，仅换行符不同：

修复生成/测试流程，使 repository checkout 在 Windows 下仍能得到稳定 deterministic comparison。

优先原则：

**generated artifact 必须具有 canonical line ending。**

推荐统一 generated protocol artifacts 为 LF。

不要：

- 在测试里简单 trim 全文件掩盖真实 schema 差异
- 删除 generated consistency test
- ignore test
- 让 test 永远跳过 Windows
- 手工修改 generated 文件但不解决生成器稳定性

检查：

- protocol export writer
- `.gitattributes`
- generated artifact normalization

选择最小、长期稳定的修复。

如果 `.gitattributes` 已经规定 generated files 使用 LF：

优先确保 working tree / generator 遵守该约束。

如果仓库没有规则且 generator 是根因：

在 generator 输出处固定 `\n`。

不要扩大到无关文件的全仓库换行重写。

---

## 3. 重新生成 protocol artifacts

使用仓库现有官方生成命令。

例如当前已有：

`protocol-export generate-all`

则使用该流程。

生成后检查：

- JSON Schema 语义没有非预期变化
- TypeScript contract 没有非预期变化
- fixtures 没有非预期变化

如果只是 line ending corrective：

diff 不应出现 schema semantic change。

---

## 4. 处理 Git safe.directory

`git diff --check` 当前因为 unsafe-directory 无法执行。

不要修改仓库代码来绕过。

先报告当前：

- repository path
- Git 判定的 owner / safe-directory 问题

然后在当前开发环境中使用标准 Git safe.directory 配置解决，使 Git 能正常读取仓库。

只处理当前仓库。

不要使用过度宽泛的：

`safe.directory=*`

除非环境本身明确要求，并且不要把这种全局设置写进项目脚本或仓库配置。

目标只是：

`git diff --check`

能够真正执行。

---

## 5. 检查 MCP dependency

保持：

`rmcp = 3.2.0`

features 仍仅为本阶段需要的：

- server
- macros

不要因为 corrective 引入：

- HTTP
- stdio
- SSE
- client
- reqwest
- hyper/tower transport feature

SDK 本身的 transitive dependency 不算主动启用 transport。

---

## 6. 不允许修改功能范围

本 corrective 禁止修改：

- MCP read_file schema
- MCP glob schema
- MCP result semantics
- MCP error category
- ToolRegistry catalog
- read_file executor
- glob executor
- WorkspaceGuard security rules
- Runtime control protocol
- UI
- tunnel-client

除非发现当前实现存在真实 bug。

如果发现真实 bug：

停止扩大 corrective，
先汇报 bug 和最小修复建议。

---

## 7. 最终验证

必须真正执行并通过：

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
git diff --check
```

同时再次执行 protocol generated consistency command/test。

验收标准：

- Runtime tests 全绿
- UI Tauri tests 全绿
- Protocol tests 全绿
- generated contract consistency 全绿
- `git diff --check` 真正运行并返回成功
- 不存在 skipped validation

---

## 8. 完成后只汇报

1. generated schema mismatch 的准确根因
2. 采用的 line-ending corrective
3. 修改文件
4. 两处 Windows test 修正审查结论
5. Git safe.directory 的处理方式
6. protocol generation / consistency 结果
7. `cargo fmt`
8. `cargo check`
9. `cargo test --workspace`
10. `git diff --check`
11. rmcp features 是否保持不变
12. 明确声明：
    - 未实现 HTTP
    - 未进入 M5-D2
    - 未修改 tunnel-client
    - 未进入 M6