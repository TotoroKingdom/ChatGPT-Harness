# M00 AppConfig v2 → v3 contract fixtures

这些文件是 M00 的 source-controlled compatibility fixtures。它们只描述未来 migration contract，不会让当前 `ConfigStore` 写出 schema 3。

- `v2-representative.json`：包含当前所有重要用户设置的 non-default v2 配置。
- `v2-non-defaults.json`：覆盖另一组 tool/policy/bash/UI/tunnel 选择。
- `v2-secret-reference.json`：验证 `SecretReference` 只保留 provider/key opaque reference。
- `v2-malformed.json`：已知字段类型错误，必须 fail closed。
- `v3-candidate.json`：在 v2 字段无损保留的基础上增加 host-only `desktop`/`history`，仅供 test-only validator 使用。
- `v4-unsupported.json`：未来未知 schema，当前生产加载必须非破坏性拒绝。

所有 credential-like 内容都是 synthetic opaque reference；fixture 中不包含 raw API key 或 SecretStore value。`workspace.rootPath`、tunnel client path 和 proxy 是合成测试值，不是本机可用配置。
