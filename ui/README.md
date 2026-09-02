# ChatGPT-Harness UI

## Prerequisites

- Node.js 20+
- Rust toolchain with the Tauri 2 prerequisites for your platform

## Install and run

```bash
npm install
npm run dev
```

For the desktop shell:

```bash
npm run tauri -- dev
```

## M2 First Run development flow

The first launch shows the setup screen. Choose a workspace folder, enter a Tunnel ID, use **Auto Detect** or **Choose Folder** for the `tunnel-client` package directory, enter the API Key, select Normal or Developer/Test mode, then save. The setup flow runs local diagnostics before opening the normal application shell.

To test the first-run flow in the browser mock, open the app with `?setup=first-run`. Existing M1 scenarios remain available with `?mock=empty`, `?mock=error`, or `?mock=disconnected`.

## Local configuration and secrets

The Tauri backend owns `AppConfig` and stores pretty-printed UTF-8 JSON at the OS application-data directory, on Windows under the Tauri application-data directory for identifier `com.chatgpt.harness` (typically `%APPDATA%\\com.chatgpt.harness\\config.json`). The exact resolved path is shown in Settings. Missing configuration means First Run; corrupt or unsupported configuration is reported as a structured error and can be reset from Settings. The repository never stores machine configuration.

The API Key is passed only to the `SecretStore` abstraction. Windows uses the Windows Credential Manager through the `keyring` crate; development/browser mocks use an in-memory store. `config.json`, logs, and UI state contain only an opaque `SecretReference`, never the raw API Key.

## Checks

```bash
npm run typecheck
npm run test
npm run build
```

## Mock lifecycle and M3 boundary

M2 Start/Stop/Restart exercises independent Runtime, tunnel, and MCP mock component states. It does not launch a real Runtime executable, tunnel client, or MCP server. A future M3 Runtime adapter can replace the mock implementation behind `HarnessControlClient`; pages and the provider do not need direct Tauri calls.