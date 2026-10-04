# M7 configuration fixture provenance

These JSON files are disposable, source-controlled migration fixtures. They
contain only fake paths, IDs, opaque references, and sentinel values; they do
not contain credentials.

The field combinations follow the actual schema evolution in Git:

| Historical source | Schema evidence | Fields represented by the fixtures |
|---|---|---|
| V2-era baseline | `18f0e86` (`release: ChatGPT-Harness_0.1.1_x64-portable`) | `workspace`, `tunnel`, `security`, `ui.theme`, and `configRevision` |
| M2 | `ceb91cc` (`feat:M2 — Tool Enable / Disable`) | additive `tools` enabled states |
| M3 | `b49c7a8` (`feat:M3 — Configurable Policy`) | additive `policies.toolDefaults` and `policies.userRules` |
| M5 | `df586c4` (`feat:M5 Controlled Bash`) | additive `bash.autoDetect` and `bash.executablePath` |
| M6 | `7083fbc` (`feat:M6：docs-codex/feat1-M6-i18n`) | additive `ui.language` (`en` / `zh-CN`) |

`v1-v2-era.json` is the pure V2-era shape. The staged positive fixtures
`v1-transitional-m2-tools.json`, `v1-transitional-m3-policy.json`,
`v1-transitional-m5-bash.json`, and `v1-transitional-m6-i18n.json` add the
fields at the corresponding historical boundary. `v1-transitional-m6.json`
is the combined transitional shape after all additive M2–M6 fields existed.
The negative fixtures are controlled mutations of those source shapes:

- `v1-invalid-policy.json`: unsupported policy matcher tool.
- `v1-invalid-proxy.json`: proxy URL containing credentials.
- `v1-unsupported-field.json`: unsupported raw API-key-like field with a
  synthetic sentinel.
- `v1-invalid-secret-like-field.json`: same negative boundary with the M7
  sentinel required by the Local Black-box plan.
- `v1-future-schema.json`: complete current-shape payload marked
  `schemaVersion=3` to prove future data is never downgraded.
- `v2-current.json`: complete current schema-2 representation for save and
  revision checks.

The Integration and Local Black-box QA tests create their disposable config
directories at runtime and never mutate these fixture files.
