# M7 Independent Automated QA Report

Date: **2026-10-04 (Asia/Shanghai)**
Status: **M7 — ACCEPTED / FROZEN**

严格依据本目录 `INTEGRATION_TEST_PLAN.md` 与 `E2E_TEST_PLAN.md` 执行。
Integration **40/40 PASS**；Local Black-box E2E **25/25 PASS**。
最终 **FAIL=0、BLOCKED=0，无未解决产品缺陷或自动化 QA 阻塞项**。

Human Reviewer 已完成 `MANUAL_ACCEPTANCE_CHECKLIST.md` 全部人工验收项并确认
PASS。本报告不代替 M8 External ChatGPT 验收；V3 仍未标记为 ACCEPTED / FROZEN。

## 1. 实际代码、构建与证据位置

- Git baseline：`352f2d436cfb717a4f949befb846cb054889b1cf`。
- Production implementation commit：`63c63c833528fb0560652e9d032f08563a64f453`。
- Automated evidence commit：`eb9cc67853321e21bfe68ffe6fdac34437a86bf3`。
- 被测内容为该 HEAD 上已有的 **M7 Development Complete 工作区代码**，不是
  HEAD 单独的旧实现。QA 保留了开始时已有的 staged/unstaged 修改。
- QA 没有修复或改写产品业务逻辑；新增独立测试、历史 fixtures、测试模块注册、
  黑盒证据探针，并修正两处旧回归运行器的历史 fixture/事件契约。
- 最终 source SHA-256 manifest：`qa/final-source-manifest.txt`，其 SHA-256 为
  `4C693C88101BCA200A3D40B4F12E82B12EB7185DBC280949CDC22DF2CAFB904C`。
  `qa/changes-since-entry.txt` 表明已有文件的 QA 增量仅涉及测试模块注册和 M2
  测试运行器；新文件列入最终 manifest。

独立 Portable（便携包）目录：

```text
artifacts/M7-Migration-Evidence-Hardening/ChatGPT-Harness_0.1.1_x64-portable/
```

固定被测 Runtime 位于该包的同字节构建副本：

```text
target/release/bundle/portable/M7-Migration-Evidence-Hardening/
  ChatGPT-Harness_0.1.1_x64-portable/runtime/chatgpt-harness-runtime.exe
```

| 产物 | SHA-256 |
|---|---|
| 最终包 Runtime | `90CD62C0081366386A933EA919CDC490132FB76FD90C72DD033AA103EE0DB0CB` |
| 最终包 UI | `1B4F9D89E6B3D6B5F1516E85106DC0E8115E5363CD1A2754C65745695EDCF868` |
| Distribution manifest | `B3ADEF887386FFD8728730CA211B897E054D0CF4A8CFCFF5C0E89A48F1DCF942` |

`cargo test --release` 会重新链接共享 `target/release` 中的 Runtime；过程中观察到
其 SHA 改变，因此最终黑盒和旧回归全部显式指向独立包内固定 Runtime。
`qa/packaged-runtime-before.txt`、`qa/packaged-runtime-after.txt` 以及最终
E2E run header 均为上表 Runtime SHA，artifact 副本也相同。早期不同 SHA 的
调试日志保留为历史证据，不作为最终包 identity 的证明。

全部命令、初次失败、重跑和安全 JSONL evidence 归档于：

[M7 QA evidence](../../../../artifacts/M7-Migration-Evidence-Hardening/qa/)

主要证据：

- `config-integration/INT_CONFIG_RESULTS.md`、`final-config-integration.log`。
- `runtime-integration/COMMAND.txt`、`m7-runtime-integration.log`、`evidence.jsonl`。
- `runtime-integration/real-bash-timeout-integration.log`。
- `ui/m7-qa-integration-final.log`、frontend 全量测试/typecheck/build 日志。
- `packaged-m7-e2e-evidence-complete.log`。
- `e2e/m7-e2e-1791095952165-11340.jsonl`：最终 25 条 case 记录。
- `e2e/migration-<case>-<variant>.jsonl`：组合状态 case 的每组真实配置迁移 SHA/schema/revision/temp 证据。
- `captured-evidence-validation.log`：最终 case inventory、真实 Workspace/terminal 和 hash evidence 校验。
- `e2e/control-<PID>-<case>.jsonl`：最后一次运行中真实 Control read/approval
  DTO；保存 request/approval IDs、Runtime PID/runtimeId、Activity、计数和错误。
  不记录 MCP raw payload、文件内容、Bash stdout/stderr 或环境值。
- `portable-stage.log`、`portable-build.log`、`portable-verify.log`、
  `portable-sha256.txt` 与最终 evidence SHA-256 manifest。

## 2. 执行边界和入口检查

配置 Integration 使用 production `ConfigStore`、真实临时文件、历史 fixtures
和 Windows 原子写实现。Runtime Integration 组合真实 RuntimeCore、MCP adapter、
ExecutionPipeline、PolicyEngine、ApprovalManager 和 Git Bash；过期使用确定性
clock seam，race 使用 barrier，计数来自真实 lifecycle。

UI Integration 使用生产 App/Provider/components/parser/presentation 组合和受控
client fixtures；它只证明 UI 集成，不被用于替代真实 Runtime/MCP 黑盒测试。
Local E2E 使用 production ConfigStore persistence path、production snapshot
construction、release Runtime 子进程、stdin/stdout JSONL Control 和 loopback MCP。
E2E-19 将真实 DTO 交给实际生产 TypeScript mapper/语言资源，未复制映射表。

| Gate | 最终结果 | 日志 |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | `closing-fmt.log` |
| `cargo check --workspace` | PASS | `final-check.log` |
| `cargo test --workspace` | PASS：460 passed、0 failed、5 ignored | `final-workspace-tests.log` |
| `npm test`（`ui/`） | PASS：242 tests、22 files | `ui/m7-qa-full-npm-test.log` |
| `npm run typecheck`（`ui/`） | PASS | `ui/m7-qa-final-typecheck.log` |
| `npm run build`（`ui/`） | PASS | `ui/m7-qa-full-build.log`、Portable build log |
| `git diff --check` | PASS | `closing-diff-check.log` |
| release Runtime build | PASS | `final-release-build.log`、`portable-stage.log` |
| 独立 M7 Portable build + verification | PASS | `portable-package.log`、`portable-verify.log` |

workspace 默认忽略的黑盒测试已分别显式执行。没有把 ignored 算作 PASS。
最后新增的 Control evidence recorder 只属于 `cfg(test)` 的 QA 运行器；不改变
包内生产逻辑。其最新代码经实际完整黑盒编译/运行、fmt 和 diff 检查通过。

## 3. 实际命令索引

以下命令在 repository root 执行；前端命令在 `ui/` 执行。

| ID | 实际命令/边界 |
|---|---|
| C | `cargo test -p chatgpt-harness-ui --lib config::m7_qa::m7_config_integration_cases -- --exact --nocapture --test-threads=1` |
| R | `cargo test -p chatgpt-harness-runtime m7_runtime_integration_cases -- --nocapture --test-threads=1` |
| T | `cargo test -p chatgpt-harness-runtime execution::tests::m7_bash_timeout_and_output_limit_release_running_counter -- --exact --nocapture --test-threads=1` |
| U | `npm test -- src/m7-qa-integration.test.tsx`，以及全量 `npm test` |
| B | `cargo test -p chatgpt-harness-ui --lib runtime::m7_e2e::m7_local_black_box_e2e -- --exact --ignored --nocapture --test-threads=1` |
| G1 | `cargo test -p chatgpt-harness-runtime --release --test m1_e2e -- --nocapture` |
| G2/3 | `cargo test -p chatgpt-harness-ui --lib runtime::m2_e2e:: -- --ignored --nocapture --test-threads=1` |
| G4 | `cargo test -p chatgpt-harness-runtime --release --test m4_e2e -- --ignored --nocapture --test-threads=1` |
| G6 | `cargo test -p chatgpt-harness-ui --lib runtime::m6_i18n_qa:: -- --ignored --nocapture --test-threads=1` |

最终 B/G 命令均设置：

```powershell
$env:HARNESS_E2E_RUNTIME_BIN = (Resolve-Path target/release/bundle/portable/M7-Migration-Evidence-Hardening/ChatGPT-Harness_0.1.1_x64-portable/runtime/chatgpt-harness-runtime.exe).Path
$env:HARNESS_M4_E2E_RUNTIME_BIN = $env:HARNESS_E2E_RUNTIME_BIN
```

上述 C/R/T/U/B/G 实际执行 exit code 均为 0。打包的实际步骤保存在
`qa/package-m7.ps1`：独立 M7 distribution staging、Tauri `--no-bundle --ci
--features custom-protocol` 构建、复制到独立 M7 Portable 目录、调用
`ui/scripts/verify-portable.ps1`。旧 Milestone 包没有被覆盖或复用。

## 4. Integration：每个 case 的最终状态

| Case | 状态 | 命令 / 关键断言 |
|---|---|---|
| INT-01 | PASS | C：新默认 schema 2、V3 defaults、wire v1 |
| INT-02 | PASS | C：pure V2 v1 经 load/atomic persist 成为完整 v2 |
| INT-03 | PASS | C：M2/M3/M5/M6 staged 和组合 fixtures 非默认语义保留 |
| INT-04 | PASS | C：migration 保留 revision 41，不 +1 |
| INT-05 | PASS | C：normal/DeveloperTest 两向 bool normalization |
| INT-06 | PASS | C：完整 Tunnel/proxy/client/autoDetect/opaque reference 保留；不 materialize secret |
| INT-07 | PASS | C：四文件工具 ON、Bash OFF；ALLOW/ASK、rules、Bash/language frozen defaults |
| INT-08 | PASS | C：malformed tool/policy/Bash/language 不 fallback，原 bytes 不变 |
| INT-09 | PASS | C：invalid enabled Policy 整体 fail closed，无部分 v2 |
| INT-10 | PASS | C：0/future/missing/wrong-type version 拒绝且不变 |
| INT-11 | PASS | C：owned objects unknown fields 拒绝、不静默丢字段 |
| INT-12 | PASS | C：raw-secret-like fixture 拒绝；安全错误；原 bytes 保留 |
| INT-13 | PASS | C：六个正向历史 fixtures valid v2、可重读、无 temp residue |
| INT-14 | PASS | C + workspace ConfigStore replace-failure case：真实 write/replace failure 保留源、清理本次 temp |
| INT-15 | PASS | C：六个来源第二次加载 byte/revision 稳定 |
| INT-16 | PASS | C：同语义/UI-only 不增 revision；五类 Runtime 配置变化 +1 |
| INT-17 | PASS | C：load failure 后真实 metadata directory/file 仍可读取 |
| INT-18 | PASS | C + App recovery tests：无自动 reset；取消 confirm 不 reset；显式确认后 reset |
| INT-19 | PASS | R：四个真实文件工具只记录安全 logical args，不含内容/物理根 |
| INT-20 | PASS | R：identity/enabled/capability；disabled 无 executor side effect |
| INT-21 | PASS | R：ALLOW/ASK/DENY canonical decision/rule IDs/reason |
| INT-22 | PASS | R：AllowOnce/AllowForSession/Deny 同 request/approval correlation |
| INT-23 | PASS | R：success/DENY/approval denied/execution failed terminal duration |
| INT-24 | PASS | R：bytesRead/matchCount/write disposition/bytes/replacements scalars |
| INT-25 | PASS | R + T：真实 Bash classification/risk/cwd/approval/grant/exit/duration/limit，bounded preview、无 output/env |
| INT-26 | PASS | R：超长 path/pattern/command 和 error bounded/sanitized，无 panic |
| INT-27 | PASS | R：合法 ID 保留、unsafe identifiers redacted |
| INT-28 | PASS | R：真实 active lifecycle；validation/DENY/ASK/error 后归零 |
| INT-29 | PASS | R：Pending、并发、Allow/Deny/Expire/Cancel 全部归零 |
| INT-30 | PASS | R + T：executor 前/中/后真实 running，approval 不算执行，timeout/cancel/stop 清理 |
| INT-31 | PASS | R + T：barrier resolve-vs-stop、Bash timeout/cancel、executor failure；不 underflow |
| INT-32 | PASS | R：八类以上 availability/reason machine-distinct |
| INT-33 | PASS | R + U：真实 reason identity、production remediation map、无适用操作的说明 |
| INT-34 | PASS | R + U：invalid Policy DENY/unavailable，UI error/repair，无 ALLOW fallback |
| INT-35 | PASS | U；B-20 corroboration：desired/effective revision 分离、Restart Required、restart 收敛 |
| INT-36 | PASS | C + U；B-03 corroboration：migration 不制造 revision/restart debt |
| INT-37 | PASS | U：Activity detail 展示 args/policy/approval/outcome/duration/scalars，安全渲染 |
| INT-38 | PASS | U：en/zh-CN key/placeholder parity 和 presentation coverage |
| INT-39 | PASS | R：同阶段 status/diagnostics counters、effective availability/decision/reason 全字段一致 |
| INT-40 | PASS | R + U + G1/G2/G3/G4/G6 + B-17/21/24：M0–M6 规定回归 |

Integration 未以 raw transform/helper 单测代替 ConfigStore 持久化，也未以 mock
evidence 代替 Runtime emission。T 是真实 production pipeline + Git Bash 组合，
使用短测试 deadline；B-17 另以 release Runtime 的原始 **30 秒**限值证明真实超时。

## 5. Local Black-box E2E：每个 case 的最终状态

下列全部使用 B 命令和同一固定 package Runtime，最终 JSONL 每个 case 均有
`status=PASS`，没有缺失/重复 case。

| Case | 状态 | 真实边界 / 关键 evidence |
|---|---|---|
| E2E-01 | PASS | pure v1 production load、完整 typed preservation、schema 1→2、revision 41、SHA/no temp |
| E2E-02 | PASS | transitional 全字段/Policy rules/defaults/Bash/UI/Tunnel/Security 保留 |
| E2E-03 | PASS | pure/transitional 各两次 load；真实 active Runtime revision 与 saved 一致；SHA 稳定 |
| E2E-04 | PASS | typed failure、exact bytes/SHA 不变、准确 metadata 路径、无部分 v2/temp |
| E2E-05 | PASS | future schema 不降级、不 overwrite/reset |
| E2E-06 | PASS | secret-like source 原字节保留、安全 error/evidence；load 拒绝后不构造 Runtime/Activity/Diagnostics |
| E2E-07 | PASS | migrated config → production snapshot → release process；Control/MCP/effective policies/tools/Bash/diagnostics 一致 |
| E2E-08 | PASS | migrated OFF 不在 MCP catalog；stale TOOL_DISABLED、无 side effect、Activity evidence |
| E2E-09 | PASS | migrated explicit DENY 和 explicit ASK；matched ID/reason；真实 Pending、拒绝清理 |
| E2E-10 | PASS | real read_file；safe logical path/capability/workspace/policy/request/duration/bytesRead；无 content |
| E2E-11 | PASS | real write；filesystem bytes 正确、disposition/bytes scalar；Activity/Diagnostics 无内容 |
| E2E-12 | PASS | real edit；最终 bytes 正确、safe scalar；无 old/new text |
| E2E-13 | PASS | explicit User Policy DENY；matched ID/reason、Denied/duration；无执行副作用 |
| E2E-14 | PASS | ASK `(active,pending,running)=(1,1,0)`；真实 AllowOnce 同 call 恢复、approval correlation、最终 `(0,0,0)` |
| E2E-15 | PASS | 同原 call Deny、无 side effect、approval/request correlation、最终计数归零 |
| E2E-16 | PASS | real long Bash：执行中 active≥1/running=1/pending=0，正常结束后全 0 |
| E2E-17 | PASS | real 30s timeout；failure/duration/preview；实际 owned Bash/conhost/sleep descendants 全退出；Runtime 存活时计数全 0 |
| E2E-18 | PASS | Disabled/Denied/Ask/Bash unavailable 的真实 typed state/reason 和 MCP 路径不同 |
| E2E-19 | PASS | 四类真实 Control DTO → actual production mapper → expected availability/reason/remediation/en/zh-CN |
| E2E-20 | PASS | saved 41→42、active 保持 41/effective 不假变；restart 后 active/effective 收敛 |
| E2E-21 | PASS | production language save persisted；revision/PID/runtimeId 不变、无 restart debt |
| E2E-22 | PASS | 真实 write/edit/Bash output/environment 哨兵；Activity/Diagnostics/control errors/最终 evidence 全扫描 |
| E2E-23 | PASS | 270 个真实安全请求；snapshot=256、latest 可读；后续 write 正常、计数归零 |
| E2E-24 | PASS | 同包 build 的 read/glob/write/edit、OFF stale、DENY、ASK resume、safe Bash；M6 production language 另由 E2E-21 覆盖 |
| E2E-25 | PASS | all counters=0、clean Runtime exit、endpoint released、owned child PIDs 不存活、valid migrated v2 |

Migration 每条 JSONL 包含 fixture、pre/post SHA-256、schema/revision、result code、
preserved-field assertions 和 temp residue。负向 case 原 SHA/bytes 不变。
禁止表面的值使用 synthetic sentinel；本报告不复制它们。

运行时 request/approval/counter 实际值见 case JSONL 与同 case 的 Control trace。
E2E-17 的真实 during process tree 包含 Bash、conhost 和 sleep；使用实际 PID
证明终止，未仅根据 Runtime 退出推断子进程清理。E2E-25 再核对这些 owned PIDs。

最终执行 marker：

```text
LOCAL_M7_BLACK_BOX_E2E=PASS
```

## 6. M0–M6 regression

| 范围 | 最终同包真实验证 | 结果 |
|---|---|---|
| M0 | initialize/initialized、Control reads、loopback MCP lifecycle；workspace transport regression | PASS |
| M1 | G1 全 8 case；B 文件工具、atomic write/edit filesystem | PASS |
| M2 | G2 全 12 case；OFF stale rejection、save/restart/effective snapshot | PASS |
| M3 | G3 全 14 case；Policy DENY/ASK/fail closed、matched evidence | PASS |
| M4 | G4 全 18 case；same-request AllowOnce/Deny/session grant/restart scope | PASS |
| M5 | R/T real Bash limit/cancel/process cleanup；B-16/17/22/24 final release paths | PASS |
| M6 | G6 全 22 case；U instant switch；B-21 production language/revision/PID | PASS |

日志为 `qa/packaged-regression-m1.log`、`packaged-regression-m2-m3.log`、
`packaged-regression-m4.log`、`packaged-regression-m6.log`。

## 7. 失败、修正与文档澄清

没有发现需要修复的产品业务缺陷。保留初次失败和重跑记录，最终全部闭合：

- 旧 M2 legacy fixture 从当前 safe defaults 生成后误带 schema 2，删除 tools 后
  不再是合法历史来源；明确改为 schema 1，仍验证迁移后 schema 2。
- 旧 M4 session-grant assertion 仍期待 `approval_resolved`；按 M7 的准确
  `session_grant_matched` 契约验证，授权和文件 side effects 标准未降低。
- M4 首次调用缺少运行器要求的 binary env；明确指向本轮 release 后重跑。
- 新 UI 测试最初存在多节点文本查询、Provider hydration 时序、文本节点形状
  问题；修正测试查询/等待后 7/7 通过，没有改写产品展示来迎合测试。
- 新 E2E 调试修正 DENY terminal/event selection、explicit rule assertions、
  短 Bash 命令错过 running window、fake manual Bash path 和 `printenv` 换行。
  长执行观察保留在确定性 fixture，短请求仍验证真实返回/副作用/安全证据。
- E2E-17 初版试图在 Runtime stop/exit 后读取终端取消 subscription；当前停止
  边界不提供退出后的 snapshot。改为计划允许的真实 timeout，保留 30s 产品
  限值、检查存活 Runtime 的 terminal/counters 和实际完整子进程树。
- 独立复核发现的字段、Expire/Cancel、并发 race、full diagnostics projection、
  owned child cleanup、final sentinel scan 和逐 case durable observations 均补齐。
- 最终 fmt 曾提示新测试模块排序，按 rustfmt 排序后检查通过。

测试文档只做两项必要实现对齐，原因已写入相应文档：

1. Integration fixture 文件名对齐实际源控目录，并补齐分阶段样本及历史 commit
   provenance；没有用组合 fixture 删除分阶段覆盖。
2. E2E-19：Runtime DTO 以 `statusReason` 为机器身份，UI production mapper 负责
   remediation。用真实 DTO + 真实生产 mapper 的无 GUI Node probe 验证，
   没有复制 mapper 或增加虚假 DTO 字段。

## 8. 未解决项与阶段边界

未解决产品问题：**无**。未解决 FAIL/BLOCKED：**无**。

非阻塞观察：既有 `ts-rs` 对 `deny_unknown_fields` 的生成警告、unused/dead-code
和 linker warning 保留在日志；实际 serde/migration fail-closed 边界及所有必要
构建/测试通过。没有用 `cargo fix` 扩大改动。

Portable verification 证明构建、文件/图标/manifest 和 Runtime smoke；人工验收
使用上述独立 M7 最新包并确认 checklist 全部 PASS。没有修改真实用户 production
config。
临时 Workspace/config 已清理；测试产生的旧 untracked evidence 被保留并用
SHA-256 核对搬入 QA 归档，没有删除无关工作区修改。

本次不宣称 Activity 为 durable audit storage，也不宣称 External ChatGPT/M8 已完成。
M7 Manual Acceptance 已通过；V3 ACCEPTED / FROZEN 仍留给 M8。

**M7 — ACCEPTED / FROZEN**
