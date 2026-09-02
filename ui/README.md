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

## Checks

```bash
npm run typecheck
npm run test
npm run build
```

## Mock client architecture

Pages consume data through the `HarnessControlClient` interface. The provider owns async loading state and subscription cleanup; the default factory injects `MockHarnessControlClient`, which returns deterministic M0-shaped data with a small delay.

The mock scenario can be selected for local state checks with a query string such as `?mock=empty`, `?mock=error`, or `?mock=disconnected`. The selector is intentionally not exposed as a product control.

## Replacing the mock

A future `TauriHarnessControlClient` can be wired in `src/services/harness/factory.ts`. Pages and the provider continue to use the same typed client boundary, so no page-level RPC or Tauri `invoke` knowledge is required.
