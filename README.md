# ChatGPT-Harness

ChatGPT-Harness is not an autonomous agent framework. It is a secure local execution runtime that exposes controlled filesystem and process capabilities to ChatGPT through MCP.

## M3 runtime development

- Build the independent Runtime executable with `cargo build -p chatgpt-harness-runtime`. The binary is `target/debug/chatgpt-harness-runtime.exe` on Windows.
- Tauri starts a sibling Runtime binary by default. For development, set `CHATGPT_HARNESS_RUNTIME_PATH` to an explicit executable path. `npm --prefix ui run tauri:dev` builds the debug Runtime before launching Tauri. For a production build, run `npm --prefix ui run tauri:build`; both binaries are written to the same `target` profile directory.
- The Runtime can run on its own: `target/debug/chatgpt-harness-runtime.exe`. It receives UTF-8 JSONL control messages on stdin, writes only protocol JSONL to stdout, and sends logs to stderr.
- To exercise crash detection, start the Harness in the desktop UI, force-end `chatgpt-harness-runtime.exe`, and confirm the UI reports the Runtime as crashed/disconnected. Start Harness again to create a new Runtime instance.