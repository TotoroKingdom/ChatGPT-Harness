# Repository Guidelines

## Project Structure & Module Organization

The repository combines a Rust workspace with a React/Vite frontend and Tauri desktop shell:

- `protocol/` contains shared protocol types, schemas, fixtures, and contract tests.
- `runtime/` contains the Runtime process, MCP/Control, workspace security, activity, and audit logic; black-box tests live in `runtime/tests/`.
- `ui/src/` contains React UI and colocated tests. `ui/src-tauri/` contains the Tauri host and backend.
- `ui/scripts/` contains Windows packaging scripts. `fixtures/`, `assets/`, `wiki/`, and `docs-codex/` contain test data, media, architecture notes, and milestone plans.

Use source code, tests, and manifests as the behavioral source of truth. `wiki/**` documents current design; `docs-codex/**` describes planned work, not implemented behavior.

## Build, Test, and Development Commands

Run frontend commands from `ui/`:

```powershell
npm install                 # install dependencies
npm run dev                 # start browser UI
npm run tauri:dev           # build debug Runtime and start Tauri
npm run typecheck           # run TypeScript checks
npm test                    # run Vitest
npm run build               # build frontend
```

At the root, `cargo test --workspace` runs Rust tests and `cargo fmt --all -- --check` verifies formatting. In `ui/`, `npm run portable:windows` builds and verifies the Windows portable distribution; `npm run tauri:build` produces the NSIS bundle.

## Coding Style & Naming Conventions

Use rustfmt-compatible 4-space Rust indentation and the existing 2-space TypeScript/JSON style. Use `PascalCase` for React components/types, `camelCase` for TypeScript values, `snake_case` for Rust modules/functions, and descriptive `kebab-case` script names. Synchronize protocol changes with fixtures, schemas, and tests. Store credentials through `SecretStore`; never commit API keys or machine configuration.

## Testing Guidelines

Name frontend tests `*.test.ts` or `*.test.tsx` and keep them near the implementation. Rust unit tests may be inline; protocol contracts belong in `protocol/tests/`, and runtime integration tests in `runtime/tests/`. Run the narrowest relevant test first, then the applicable suite. Report local automated results separately from manual or external ChatGPT acceptance; do not label unrun external checks as passing.

## Commit & Pull Request Guidelines

Follow observed prefixes: `feat(scope):`, `fix(scope):`, `docs:`, `test:`, and `release:`. Keep subjects short and behavior-focused. PRs should describe scope and design impact, link the issue/spec/TASKS, list verification commands and outcomes, and include screenshots for UI changes. Packaging changes should include artifact path/version and verification result; call out non-blocking observations and skipped manual checks.
