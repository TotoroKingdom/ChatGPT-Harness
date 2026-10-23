# M00 Controlled Agent Evaluation Feasibility POC

这是 M00 的隔离 F12 feasibility spike，不是生产 Runtime Agent Loop，也不是 M04 的完整 Evaluation dataset/runner。

## Local mechanics

```powershell
cargo test --manifest-path runtime/tests/m00/agent-evaluation-poc/Cargo.toml --target-dir target/m00-validation
```

本地测试使用 `ScriptedAgentAdapter` + in-memory transport，只验证 runner mechanics、schema validation、step limit、fixture/final-state scorer 和 cleanup。它不能作为 real model acceptance 证据。

## Real adapter path

`HttpJsonAgentAdapter` 是 provider-neutral、可控的 JSON-over-HTTP adapter。它不选择永久模型供应商，接受一个用户明确配置的 HTTP endpoint：

```text
case + tools + prior observations
→ adapter endpoint
→ {"toolCall":{"name":"read_file","arguments":{...}}}
   or {"finalAnswer":"...","stopReason":"..."}
```

`HttpMcpTransport` 使用生产 Runtime 的 loopback Streamable HTTP `/mcp`，执行 `initialize`、`tools/list`、`tools/call`。CLI 示例：

```powershell
cargo run --manifest-path runtime/tests/m00/agent-evaluation-poc/Cargo.toml --target-dir target/m00-validation -- `
  --case runtime/tests/m00/agent-evaluation-poc/cases/read-file-smoke.json `
  --mcp-url http://127.0.0.1:PORT/mcp `
  --adapter-url http://127.0.0.1:ADAPTER_PORT/turn `
  --workspace-root C:\path\to\isolated\workspace
```

只有在单独执行冻结的 M00 E2E 并得到明确授权时，才可连接真实/付费模型。`M00_AGENT_AUTH_TOKEN` 只作为 Bearer header 发送，不写入报告；本轮没有执行网络模型请求。

## Truthfulness boundary

报告分别记录 Tool selection、argument schema validity、Runtime call outcome 和 final-state task completion。Transport success 不等于 task success；runner 不推断 hidden reasoning、未发出的候选工具或 provider billed tokens。
