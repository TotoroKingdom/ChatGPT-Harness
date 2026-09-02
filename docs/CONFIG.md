# Configuration and Runtime State Contract

The Tauri backend is the only persistent configuration authority. The frontend never reads or edits configuration files directly. Machine configuration lives under the OS application-data location, outside the Git repository. Secret storage is an authority boundary; M0 defines only its opaque reference and no SecretStore backend.

`AppConfig`, `RuntimeConfigSnapshot`, and `RuntimeState` are deliberately different models.

- `AppConfig` is versioned desired state: `schemaVersion`, `configRevision`, `workspace`, `tunnel`, `security`, and `ui`. Workspace has `rootPath`; tunnel has `tunnelId`, optional `tunnelClientPath` (the tunnel-client installation/package directory; the backend resolves `tunnel-client.exe` inside it), `autoDetect`, and optional `apiKeyRef`; security has `mode` and `developerTestMode`.

On the JSON wire, `configRevision` is a non-negative decimal string (for example, `"7"`), not a JSON number.
- `apiKeyRef` is a `{ provider, key }` SecretStore reference. No model stores an API key value.
- `RuntimeConfigSnapshot` is the non-secret configuration passed in `initialize`: `configRevision`, workspace, security, and `runtimeLimits`. It excludes API keys, raw secrets, UI preferences, and tunnel credentials.

On the JSON wire, `configRevision` is a non-negative decimal string (for example, `"7"`), not a JSON number.
- `RuntimeState` is observed, ephemeral state. It includes Runtime identity, start time, lifecycle/readiness, the `configRevision` in use, workspace/security mode, exposed catalog, request/approval/execution counts, and an optional last error. The Runtime never persists or mutates `AppConfig`.

On the JSON wire, `configRevision` is a non-negative decimal string (for example, `"7"`), not a JSON number.

`configRevision` lets the control plane distinguish saved desired configuration from the snapshot running in the Runtime. If their revisions differ, the UI can report `configMismatch: true` and `restartRequired: true`; it must not present a stale Runtime as already using the updated configuration.


Normal mode contracts expose only `read_file` and `glob`. A `bash` descriptor may appear as implemented but disabled and not exposed; M0 does not implement it. Security-critical behavior such as workspace protection, policy, and fail-closed behavior is never an optional capability negotiated during initialization.


