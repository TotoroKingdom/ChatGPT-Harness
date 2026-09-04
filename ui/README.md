# ChatGPT-Harness UI

## Prerequisites

- Node.js 20+
- Rust toolchain with the Tauri 2 prerequisites for your platform

## Install and run

```bash
npm install
npm run dev
```

For the desktop shell, use the explicit development script. It builds the debug sibling Runtime before starting Tauri:

```bash
npm run tauri:dev
```

`npm run tauri:dev` builds the debug Runtime and runs `tauri dev`; `npm run tauri:build` builds the release Runtime and runs `tauri build`. `npm run tauri` is reserved for direct Tauri CLI passthrough and has no build or preflight side effects. The Tauri backend discovers `chatgpt-harness-runtime.exe` next to its own executable. For development or diagnostics, override discovery with `CHATGPT_HARNESS_RUNTIME_PATH` set to the full Runtime executable path.

## M2 First Run development flow

The first launch shows the setup screen. Choose a workspace folder, enter a Tunnel ID, use **Auto Detect** or **Choose Folder** for the `tunnel-client` package directory, enter the API Key, select Normal or Developer/Test mode, then save. The setup flow runs local diagnostics before opening the normal application shell.

To test the first-run flow in the browser mock, open the app with `?setup=first-run`. Explicit mock scenarios remain available with `?mock=ready`, `?mock=empty`, `?mock=error`, or `?mock=disconnected`.

## Local configuration and secrets

The Tauri backend owns `AppConfig` and stores pretty-printed UTF-8 JSON at the OS application-data directory, on Windows under the Tauri application-data directory for identifier `com.chatgpt.harness` (typically `%APPDATA%\\com.chatgpt.harness\\config.json`). The exact resolved path is shown in Settings. Missing configuration means First Run; corrupt or unsupported configuration is reported as a structured error and can be reset from Settings. The repository never stores machine configuration.

The API Key is passed only to the `SecretStore` abstraction. Windows uses the Windows Credential Manager through the `keyring` crate; development/browser mocks use an in-memory store. `config.json`, logs, and UI state contain only an opaque `SecretReference`, never the raw API Key.

## Runtime JSONL smoke test

Build and run the Runtime directly:

```bash
cargo build -p chatgpt-harness-runtime
./target/debug/chatgpt-harness-runtime.exe
```

It accepts one UTF-8 JSON RPC envelope per stdin line and writes only JSONL responses/notifications to stdout. Send `initialize`, then the `initialized` notification, then `runtime.status`; send `runtime.stop` for graceful exit. Runtime logs are written to stderr.

## M3 lifecycle boundary

The normal Tauri flow uses the real Rust Runtime through the `HarnessControlClient` facade; pages still do not invoke Tauri directly. The Runtime exposes an empty Tool Catalog and empty Policies in M3. Tunnel and MCP are shown as stopped/not integrated rather than ready.

To verify crash detection, start the harness in the desktop app and force-terminate `chatgpt-harness-runtime.exe` from Task Manager or PowerShell. The UI should transition away from Ready and report the Runtime crash; Start Harness can then create a new Runtime instance.

## Checks

```bash
npm run typecheck
npm run test
npm run build
```