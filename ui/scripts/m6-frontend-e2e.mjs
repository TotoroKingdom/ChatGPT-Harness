/**
 * M6 production frontend/config E2E runner.
 *
 * The runner executes the shipped Vite bundle in a fresh jsdom WebView-like
 * environment.  Tauri `invoke` calls for config operations are forwarded to
 * the Rust m6_config_bridge example, which includes the production
 * ConfigStore, atomic save, RuntimeSupervisor and control-read mapping.  The
 * other read-only provider responses are bounded production-equivalent
 * fixtures so the real App/routes can render without starting an external
 * tunnel.  Runtime process/MCP lifecycle assertions remain in the companion
 * Runtime E2E runner.
 */

import { createHash } from "node:crypto";
import { execFile, spawnSync } from "node:child_process";
import { existsSync, promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { JSDOM } from "jsdom";

const scriptPath = fileURLToPath(import.meta.url);
const uiRoot = path.resolve(path.dirname(scriptPath), "..");
const repoRoot = path.resolve(uiRoot, "..");
const evidencePath = path.join(repoRoot, "artifacts", "M6-i18n", "qa", "frontend-e2e-evidence.md");
const bridgeExe = path.join(
  repoRoot,
  "target",
  "debug",
  "examples",
  process.platform === "win32" ? "m6_config_bridge.exe" : "m6_config_bridge",
);

const args = process.argv.slice(2);
const arg = (name, fallback = undefined) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : fallback;
};
const has = (name) => args.includes(name);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitFor(predicate, label, timeout = 4000) {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    try {
      const value = await predicate();
      if (value) return value;
    } catch {
      // The React tree can be between renders; retry until the bounded timeout.
    }
    await sleep(15);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function runSync(command, commandArgs, options = {}) {
  // This host resolves `npm` through a PowerShell shim.  spawnSync cannot
  // execute that shim directly with the Windows process API, so invoke npm's
  // CLI through the same Node installation explicitly.
  const executable = command === "npm" ? process.execPath : command;
  const effectiveArgs = command === "npm"
    ? [path.join(path.dirname(process.execPath), "node_modules", "npm", "bin", "npm-cli.js"), ...commandArgs]
    : commandArgs;
  return spawnSync(executable, effectiveArgs, {
    cwd: repoRoot,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });
}

function execFileAsync(file, fileArgs, options = {}) {
  return new Promise((resolve, reject) => {
    execFile(file, fileArgs, {
      cwd: repoRoot,
      windowsHide: true,
      maxBuffer: 16 * 1024 * 1024,
      ...options,
    }, (error, stdout, stderr) => {
      if (error) {
        error.stdout = stdout;
        error.stderr = stderr;
        reject(error);
      } else {
        resolve({ stdout, stderr });
      }
    });
  });
}

async function runBridge(directory, operation, values = {}) {
  if (!existsSync(bridgeExe)) {
    throw new Error(`ConfigStore bridge is missing: ${bridgeExe}`);
  }
  const bridgeArgs = ["--dir", directory, "--op", operation];
  if (Object.hasOwn(values, "language")) {
    bridgeArgs.push("--language", values.language === null ? "null" : values.language);
  }
  if (Object.hasOwn(values, "config")) {
    bridgeArgs.push("--config-json", JSON.stringify(values.config));
  }
  let response;
  try {
    const result = await execFileAsync(bridgeExe, bridgeArgs, {
      env: { ...process.env, ...(values.env ?? {}) },
    });
    const line = result.stdout.trim().split(/\r?\n/).filter(Boolean).at(-1);
    response = JSON.parse(line ?? "{}");
  } catch (error) {
    const stdout = error?.stdout ?? "";
    const line = stdout.trim().split(/\r?\n/).filter(Boolean).at(-1);
    try {
      response = JSON.parse(line ?? "{}");
    } catch {
      throw new Error(`ConfigStore bridge process failed: ${error?.message ?? String(error)}`);
    }
  }
  if (!response.ok) {
    const bridgeError = new Error(response.message ?? response.code ?? "ConfigStore bridge failed");
    bridgeError.code = response.code;
    bridgeError.bridgeResponse = response;
    throw bridgeError;
  }
  return response.value;
}

function baseConfig(language) {
  return {
    schemaVersion: 1,
    configRevision: "0",
    workspace: { rootPath: repoRoot },
    tunnel: { tunnelId: "m6-e2e-tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: language ? { language } : {},
    tools: {
      readFile: { enabled: true },
      glob: { enabled: true },
      write: { enabled: true },
      edit: { enabled: true },
      bash: { enabled: false },
    },
    bash: { autoDetect: true },
    policies: {
      toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
      userRules: [],
    },
  };
}

async function prepareFixture(directory, language, overrides = {}) {
  const config = { ...baseConfig(language), ...overrides };
  return runBridge(directory, "save", { config });
}

async function rawConfig(directory) {
  const file = path.join(directory, "config.json");
  return fs.readFile(file, "utf8");
}

async function disposableFixture(language, overrides = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "m6-i18n-frontend-"));
  await prepareFixture(directory, language, overrides);
  return directory;
}

function toolFixtures() {
  return {
    items: [
      { name: "read_file", description: "Read a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
      { name: "glob", description: "Find workspace paths.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
      { name: "write", description: "Write a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
      { name: "edit", description: "Edit a workspace text file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
      { name: "bash", description: "Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.", implemented: true, enabled: false, exposed: true, effectivePolicy: "ask", scope: "active_workspace", readiness: "unavailable", riskLevel: "high" },
    ],
  };
}

function policyFixtures() {
  return {
    items: [
      { id: "system-deny-outside", name: "System workspace guard", decision: "deny", scope: "outside workspace", reason: "System guardrail", source: "system", editable: false, enabled: true },
      { id: "user-rule-1", name: "Review writes", decision: "ask", scope: "write", reason: "Writes need review.", source: "user", editable: true, enabled: true },
    ],
    effective: {
      valid: true,
      status: "valid",
      systemRules: [{ id: "system-deny-outside", name: "System workspace guard", decision: "deny", reason: "System guardrail", source: "system", editable: false, enabled: true }],
      userRules: [{ id: "user-rule-1", name: "Review writes", decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "Writes need review.", source: "user", editable: true, enabled: true }],
      toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
      perTool: [{ tool: "read_file", decision: "allow", matchedRuleIds: [], reason: "tool_default", availability: "available" }],
    },
  };
}

function approvalFixtures() {
  return {
    items: [
      {
        requestId: "request-m6-uuid",
        runtimeId: "runtime-m6",
        generationId: "generation-m6",
        tool: "write",
        sanitizedArguments: { path: path.join(repoRoot, "fixture.txt"), content: "<redacted>" },
        scopeSummary: path.join(repoRoot, "fixture.txt"),
        matchedRuleIds: ["user-rule-1"],
        policyReason: "matched_user_rules",
        riskLevel: "medium",
        createdAt: "2026-10-04T00:00:00Z",
        expiresAt: "2026-10-04T00:05:00Z",
        status: "pending",
      },
      {
        requestId: "bash-m6-uuid",
        runtimeId: "runtime-m6",
        generationId: "generation-m6",
        tool: "bash",
        sanitizedArguments: { command: "git status --short" },
        scopeSummary: "git status --short",
        matchedRuleIds: [],
        policyReason: "tool_default",
        riskLevel: "high",
        bashDetails: {
          commandPreview: "git status --short",
          classification: "classified",
          normalizedSegments: [{ executable: "git", argv: ["status", "--short"], argvRedacted: false }],
          riskFlags: [],
          cwd: "active_workspace",
          previewRedacted: false,
          previewTruncated: false,
        },
        allowForSessionEligible: true,
        createdAt: "2026-10-04T00:00:00Z",
        expiresAt: "2026-10-04T00:05:00Z",
        status: "pending",
      },
    ],
  };
}

function harnessFixture() {
  return {
    lifecycle: "stopped",
    runtime: "stopped",
    tunnel: "stopped",
    mcp: "stopped",
    activeSessionId: null,
    configRevision: null,
    lastError: null,
    tunnelReadiness: null,
  };
}

function runtimeFixture() {
  return {
    runtimeId: "runtime-m6",
    startedAt: "2026-10-04T00:00:00Z",
    lifecycle: "ready",
    readiness: "ready",
    configRevision: "1",
    activeWorkspace: repoRoot,
    securityMode: "normal",
    exposedToolCatalog: toolFixtures().items,
    activeRequests: 0,
    pendingApprovals: 1,
    runningExecutions: 0,
  };
}

function effectiveToolFixtures() {
  return toolFixtures().items.map((item) => ({
    name: item.name,
    availability: item.name === "bash" ? "unavailable_by_runtime" : "available",
    decision: item.name === "bash" ? "deny" : "allow",
    declaredCapability: item.name === "bash" ? "developer_shell" : item.name === "read_file" ? "workspace_read" : "workspace_write",
    readiness: item.name === "bash" ? "unavailable" : "ready",
    ...(item.name === "bash" ? { reason: "Runtime is stopped.", readinessReason: "Runtime is stopped." } : {}),
  }));
}

function configureDom(dom, systemLocale, directory, calls) {
  const window = dom.window;
  Object.defineProperty(window.navigator, "languages", { configurable: true, value: [systemLocale] });
  Object.defineProperty(window.navigator, "language", { configurable: true, value: systemLocale });
  window.confirm = () => true;
  if (!window.crypto?.getRandomValues) {
    window.crypto = { getRandomValues: (array) => { array[0] = Math.floor(Math.random() * 0xffffffff); return array; } };
  }

  const callbacks = new Map();
  const listeners = new Map();
  let callbackId = 0;
  const registerCallback = (callback, once = false) => {
    const id = ++callbackId;
    callbacks.set(id, (value) => {
      if (once) callbacks.delete(id);
      return callback?.(value);
    });
    return id;
  };
  const unregisterCallback = (id) => callbacks.delete(id);
  const invoke = async (command, payload = {}) => {
    calls.push({ command, payload });
    switch (command) {
      case "config_get": return runBridge(directory, "get");
      case "config_metadata": return runBridge(directory, "metadata");
      case "ui_language_set": return runBridge(directory, "set", { language: payload.language ?? null });
      case "config_save": return runBridge(directory, "save", { config: payload.config });
      case "config_reset": await runBridge(directory, "reset"); return null;
      case "secret_api_key_status": return false;
      case "secret_api_key_delete": return null;
      case "workspace_choose": return repoRoot;
      case "tunnel_client_choose": return null;
      case "tunnel_client_detect": return null;
      case "validate_path": return true;
      case "runtime_process_status": return { phase: "stopped", last_error: null };
      case "harness_status": return harnessFixture();
      case "harness_start_eligibility": return { status: "ready_to_start", canStart: false, message: "Runtime lifecycle is owned by the Runtime E2E runner." };
      case "runtime_start_eligibility": return { status: "ready_to_start", canStart: false, message: "Runtime lifecycle is owned by the Runtime E2E runner." };
      case "runtime_status": return runtimeFixture();
      case "runtime_tools_list": return toolFixtures();
      case "runtime_policies_list": return policyFixtures();
      case "runtime_activity_list": return { items: [], latestSequence: "0" };
      case "runtime_approvals_list": return approvalFixtures();
      case "runtime_activity_subscribe": return { latestSequence: "0" };
      case "runtime_get_diagnostics": return { schemaVersion: 1, capturedAt: "2026-10-04T00:00:00Z", runtime: { readiness: "not_ready" }, security: { workspaceValidation: "not_initialized", policyEngineLoaded: false }, tools: { declaredCount: 5, effectiveCount: 0 } };
      case "runtime_get_activity_snapshot": return [];
      case "runtime_get_effective_tools": {
        if (process.env.M6_CONTROL_ERROR === "1") {
          try {
            return await runBridge(directory, "control-error");
          } catch (error) {
            throw { code: error.code, message: error.message };
          }
        }
        return effectiveToolFixtures();
      }
      case "runtime_approvals_resolve": throw { code: "APPROVAL_NOT_FOUND", message: "Approval request is not available." };
      case "local_diagnostics_run": return { runId: "m6-diagnostics", startedAt: "2026-10-04T00:00:00Z", finishedAt: "2026-10-04T00:00:00Z", overallStatus: "pass", checks: [] };
      case "runtime_activity_subscribe": return { latestSequence: "0" };
      case "plugin:event|listen":
        if (!listeners.has(payload.event)) listeners.set(payload.event, new Set());
        listeners.get(payload.event).add(payload.handler);
        return payload.handler;
      case "plugin:event|unlisten":
        listeners.get(payload.event)?.delete(payload.eventId);
        return null;
      default: throw { code: "COMMAND_NOT_IMPLEMENTED", message: `Fixture does not implement ${command}` };
    }
  };
  window.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: registerCallback,
    unregisterCallback,
    runCallback: (id, value) => callbacks.get(id)?.(value),
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (_event, id) => unregisterCallback(id),
  };

  for (const name of ["window", "document", "HTMLElement", "HTMLSelectElement", "HTMLInputElement", "Node", "Element", "Event", "MouseEvent", "KeyboardEvent", "MutationObserver", "requestAnimationFrame", "cancelAnimationFrame", "getComputedStyle"]) {
    if (name in window) globalThis[name] = window[name];
  }
  globalThis.window = window;
  globalThis.document = window.document;
  // Node 22 exposes a read-only global navigator.  Replace its value through
  // the configurable property so the production resolver sees the jsdom
  // system-locale fixture rather than the host process locale.
  Object.defineProperty(globalThis, "navigator", { configurable: true, writable: true, value: window.navigator });
}

async function bootstrapApp({ directory, systemLocale, bundle }) {
  const html = await fs.readFile(path.join(uiRoot, "dist", "index.html"), "utf8");
  const dom = new JSDOM(html, { url: "tauri://localhost/", pretendToBeVisual: true });
  const calls = [];
  const errors = [];
  dom.window.addEventListener("error", (event) => errors.push(event.error?.message ?? event.message));
  configureDom(dom, systemLocale, directory, calls);
  let firstRender;
  const firstRenderObserver = new dom.window.MutationObserver(() => {
    if (!firstRender && dom.window.document.querySelector(".app-shell")) {
      firstRender = {
        htmlLang: dom.window.document.documentElement.lang,
        text: pageText(dom),
      };
    }
  });
  firstRenderObserver.observe(dom.window.document.documentElement, { childList: true, subtree: true });
  const oldUnhandled = process.listeners("unhandledRejection");
  const onUnhandled = (error) => errors.push(error?.message ?? String(error));
  process.on("unhandledRejection", onUnhandled);
  try {
    await import(`${pathToFileURL(bundle).href}?m6=${encodeURIComponent(`${directory}-${systemLocale}`)}`);
    await waitFor(() => dom.window.document.querySelector(".app-shell"), "production AppShell");
    await sleep(40);
    firstRenderObserver.disconnect();
    firstRender ??= { htmlLang: dom.window.document.documentElement.lang, text: pageText(dom) };
    return { dom, calls, errors, firstRender };
  } finally {
    firstRenderObserver.disconnect();
    process.removeListener("unhandledRejection", onUnhandled);
    for (const listener of oldUnhandled) {
      if (!process.listeners("unhandledRejection").includes(listener)) process.on("unhandledRejection", listener);
    }
  }
}

function pageText(dom) {
  return dom.window.document.body.textContent?.replace(/\s+/g, " ").trim() ?? "";
}

function assertLocale(dom, expected, labels) {
  if (dom.window.document.documentElement.lang !== expected) {
    throw new Error(`expected html lang ${expected}, got ${dom.window.document.documentElement.lang}`);
  }
  const text = pageText(dom);
  for (const label of labels) {
    if (!text.includes(label)) throw new Error(`missing ${expected} shell label: ${label}`);
  }
  if (/\b(?:app|nav|overview|tools|policies|activity|approvals|settings)\.[A-Za-z][A-Za-z0-9_.-]*/.test(text)) {
    throw new Error("raw translation key rendered in production shell");
  }
  if (text.includes("[object Object]") || text.includes("undefined")) throw new Error("invalid placeholder rendered in production shell");
}

function navButtons(dom) {
  return [...dom.window.document.querySelectorAll("nav button")];
}

async function clickNav(dom, index) {
  const buttons = navButtons(dom);
  if (!buttons[index]) throw new Error(`navigation button ${index} missing`);
  buttons[index].dispatchEvent(new dom.window.MouseEvent("click", { bubbles: true }));
  await sleep(40);
}

async function openSettings(app) {
  await clickNav(app.dom, 5);
  await waitFor(() => app.dom.window.document.querySelector(".settings-page select"), "Settings language selector");
  return app.dom.window.document.querySelector(".settings-page select");
}

async function changeLanguage(app, value) {
  const select = await openSettings(app);
  select.value = value;
  select.dispatchEvent(new app.dom.window.Event("change", { bubbles: true }));
  await waitFor(() => select.disabled === true, "language mutation pending");
  await waitFor(() => select.disabled === false && select.value === value, "language mutation completion");
  await sleep(30);
  return select;
}

async function renderAllRoutes(app) {
  const texts = [];
  for (let index = 0; index < 6; index += 1) {
    await clickNav(app.dom, index);
    const text = pageText(app.dom);
    if (text.length < 20) throw new Error(`route ${index} rendered no meaningful content`);
    if (/\b(?:app|nav|overview|tools|policies|activity|approvals|settings)\.[A-Za-z][A-Za-z0-9_.-]*/.test(text)) throw new Error(`route ${index} rendered raw translation key`);
    texts.push(text);
  }
  if (app.errors.length) throw new Error(`production App route emitted error(s): ${app.errors.join(" | ")}`);
  return texts;
}

function machineTokens(text) {
  const expected = [
    "read_file", "glob", "write", "edit", "bash", "system-deny-outside", "user-rule-1",
    "request-m6-uuid", "runtime-m6", "generation-m6", path.join(repoRoot, "fixture.txt"),
    "bash-m6-uuid", "git status --short", "active_workspace",
  ];
  return expected.filter((token) => text.includes(token)).sort();
}

async function freshBootstrapCheck(directory, systemLocale, bundle) {
  const result = runSync(process.execPath, [scriptPath, "--child", "--case", "bootstrap-check", "--dir", directory, "--system-locale", systemLocale, "--bundle", bundle]);
  const line = result.stdout?.trim().split(/\r?\n/).filter(Boolean).at(-1);
  if (!line) throw new Error(`restart child emitted no result: ${result.stderr ?? ""}`);
  const parsed = JSON.parse(line);
  if (parsed.status !== "PASS") throw new Error(`restart child failed: ${parsed.summary}`);
  return parsed.evidence;
}

async function caseResult(caseId, fn) {
  try {
    const evidence = await fn();
    return { caseId, status: "PASS", evidence };
  } catch (error) {
    return { caseId, status: "FAIL", summary: error?.stack ?? String(error) };
  }
}

async function runChildCase(caseId, directory, systemLocale, bundle) {
  const app = await bootstrapApp({ directory, systemLocale, bundle });
  return { app, config: await runBridge(directory, "get") };
}

async function childMain() {
  const caseId = arg("--case");
  const systemLocale = arg("--system-locale", "en-US");
  const bundle = arg("--bundle");
  let directory = arg("--dir");
  let ownedDirectory = false;
  if (!directory) {
    directory = await fs.mkdtemp(path.join(os.tmpdir(), "m6-i18n-child-"));
    ownedDirectory = true;
  }
  const result = await caseResult(caseId, async () => {
    if (caseId === "bootstrap-check") {
      const app = await runChildCase(caseId, directory, systemLocale, bundle);
      const config = app.config;
      const expected = config.ui.language ?? (systemLocale.toLowerCase().startsWith("zh-") && !/^zh-(tw|hk|mo)/i.test(systemLocale) ? "zh-CN" : "en");
      assertLocale(app.app.dom, expected, expected === "zh-CN" ? ["概览", "工具", "策略", "活动记录", "审批", "设置"] : ["Overview", "Tools", "Policies", "Activity", "Approvals", "Settings"]);
      return { resolved: expected, htmlLang: app.app.dom.window.document.documentElement.lang, configRevision: config.configRevision, invokeCommands: app.app.calls.map((call) => call.command) };
    }
    if (caseId === "E2E-02") {
      const directory = await disposableFixture();
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        assertLocale(app.app.dom, "en", ["Overview", "Tools", "Policies", "Activity", "Approvals", "Settings"]);
        return { resolved: "en", htmlLang: app.app.dom.window.document.documentElement.lang, savedLanguage: app.config.ui.language ?? null, configRevision: app.config.configRevision };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-03") {
      const directory = await disposableFixture();
      try {
        const app = await runChildCase(caseId, directory, "zh-CN", bundle);
        assertLocale(app.app.dom, "zh-CN", ["概览", "工具", "策略", "活动记录", "审批", "设置"]);
        return { resolved: "zh-CN", htmlLang: app.app.dom.window.document.documentElement.lang, savedLanguage: app.config.ui.language ?? null, configRevision: app.config.configRevision };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-04") {
      const checks = {};
      for (const locale of ["ja-JP", "fr-FR", "zh-TW"]) {
        const directory = await disposableFixture();
        try {
          const app = await runChildCase(caseId, directory, locale, bundle);
          assertLocale(app.app.dom, "en", ["Overview", "Tools", "Policies", "Activity", "Approvals", "Settings"]);
          checks[locale] = { resolved: "en", htmlLang: app.app.dom.window.document.documentElement.lang };
        } finally { await fs.rm(directory, { recursive: true, force: true }); }
      }
      return checks;
    }
    if (caseId === "E2E-05" || caseId === "E2E-06") {
      const saved = caseId === "E2E-05" ? "en" : "zh-CN";
      const locale = caseId === "E2E-05" ? "zh-CN" : "en-US";
      const directory = await disposableFixture(saved);
      try {
        const app = await runChildCase(caseId, directory, locale, bundle);
        assertLocale(app.app.dom, saved, saved === "zh-CN" ? ["概览", "工具", "策略", "活动记录", "审批", "设置"] : ["Overview", "Tools", "Policies", "Activity", "Approvals", "Settings"]);
        const labels = saved === "zh-CN" ? ["概览", "工具", "策略", "活动记录", "审批", "设置"] : ["Overview", "Tools", "Policies", "Activity", "Approvals", "Settings"];
        if (app.app.firstRender.htmlLang !== saved || labels.some((label) => !app.app.firstRender.text.includes(label))) throw new Error(`saved language was not resolved before first App render: ${JSON.stringify(app.app.firstRender)}`);
        return { systemLocale: locale, savedLanguage: saved, firstRender: { htmlLang: app.app.firstRender.htmlLang, labelsVisible: labels.every((label) => app.app.firstRender.text.includes(label)) }, resolved: app.app.dom.window.document.documentElement.lang, configRevision: app.config.configRevision };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-07") {
      const directory = await disposableFixture();
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        await changeLanguage(app.app, "zh-CN");
        const saved = await runBridge(directory, "get");
        app.app.dom.window.close();
        const restart = await freshBootstrapCheck(directory, "en-US", bundle);
        if (saved.ui.language !== "zh-CN" || restart.resolved !== "zh-CN" || restart.htmlLang !== "zh-CN") throw new Error(`language was not persisted across restart: ${JSON.stringify({ saved, restart })}`);
        return { savedLanguage: saved.ui.language, savedRevision: saved.configRevision, restartResolved: restart.resolved, restartHtmlLang: restart.htmlLang };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-08") {
      const directory = await disposableFixture("zh-CN");
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        await changeLanguage(app.app, "");
        const saved = await runBridge(directory, "get");
        app.app.dom.window.close();
        const enRestart = await freshBootstrapCheck(directory, "en-US", bundle);
        const zhRestart = await freshBootstrapCheck(directory, "zh-CN", bundle);
        if (saved.ui.language !== undefined || enRestart.resolved !== "en" || zhRestart.resolved !== "zh-CN") throw new Error(`follow-system clear/restart failed: ${JSON.stringify({ saved, enRestart, zhRestart })}`);
        return { afterClearLanguage: saved.ui.language ?? null, enSystemResolved: enRestart.resolved, enRestartHtmlLang: enRestart.htmlLang, zhSystemResolvedAfterRestart: zhRestart.resolved, zhRestartHtmlLang: zhRestart.htmlLang };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-09") {
      const directory = await disposableFixture("en");
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        const revisions = [];
        for (const value of ["zh-CN", "en", ""]) {
          await changeLanguage(app.app, value);
          revisions.push((await runBridge(directory, "get")).configRevision);
        }
        const final = JSON.parse(await rawConfig(directory));
        if (!revisions.every((revision) => revision === revisions[0]) || final.ui.language !== undefined) throw new Error(`language-only revision invariant failed: ${JSON.stringify({ revisions, final })}`);
        return { revisions, finalLanguage: final.ui.language ?? null, allEqual: revisions.every((revision) => revision === revisions[0]) };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-10" || caseId === "E2E-11") {
      const directory = await disposableFixture("en");
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        await changeLanguage(app.app, "zh-CN");
        const after = await runBridge(directory, "get");
        const lifecycleCommands = app.app.calls.map((call) => call.command).filter((command) => ["runtime_start", "runtime_stop", "runtime_restart", "harness_start", "harness_stop", "harness_restart"].includes(command));
        if (lifecycleCommands.length !== 0) throw new Error(`language mutation caused lifecycle command(s): ${lifecycleCommands.join(", ")}`);
        return { configRevision: after.configRevision, language: after.ui.language, lifecycleCommands, runtimeLifecycleAssertion: "delegated to real Runtime E2E" };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-12") {
      const directory = await disposableFixture();
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        const select = await openSettings(app.app);
        const tunnel = [...app.app.dom.window.document.querySelectorAll("input")].find((input) => input.value === "m6-e2e-tunnel");
        if (!tunnel) throw new Error("persisted tunnel draft input missing");
        const setNativeValue = Object.getOwnPropertyDescriptor(app.app.dom.window.HTMLInputElement.prototype, "value")?.set;
        setNativeValue?.call(tunnel, "unsaved-draft-tunnel");
        tunnel.dispatchEvent(new app.app.dom.window.Event("input", { bubbles: true }));
        tunnel.dispatchEvent(new app.app.dom.window.Event("change", { bubbles: true }));
        await waitFor(() => tunnel.value === "unsaved-draft-tunnel", "dirty Settings draft");
        select.value = "zh-CN";
        select.dispatchEvent(new app.app.dom.window.Event("change", { bubbles: true }));
        await waitFor(() => select.disabled === true, "language save serialization guard");
        const saveButton = [...app.app.dom.window.document.querySelectorAll(".settings-actions button")].find((button) => button.classList.contains("button-primary"));
        const pendingSaveButtonDisabled = saveButton?.disabled ?? false;
        const draftDuringLanguageSave = tunnel.value;
        if (!pendingSaveButtonDisabled) throw new Error("full Settings Save remained enabled during language persistence");
        await waitFor(() => select.disabled === false && select.value === "zh-CN", "language save completion");
        const persistedBeforeFullSave = await runBridge(directory, "get");
        if (persistedBeforeFullSave.tunnel.tunnelId !== "m6-e2e-tunnel") throw new Error("language mutation committed dirty tunnel draft");
        if (draftDuringLanguageSave !== "unsaved-draft-tunnel") throw new Error("language mutation replaced dirty draft");
        const fullSave = [...app.app.dom.window.document.querySelectorAll(".settings-actions button")].find((button) => button.classList.contains("button-primary"));
        if (!fullSave || fullSave.disabled) throw new Error("full Settings Save was unavailable after language persistence");
        fullSave.dispatchEvent(new app.app.dom.window.MouseEvent("click", { bubbles: true }));
        await waitFor(async () => {
          const current = await runBridge(directory, "get");
          return app.app.calls.some((call) => call.command === "config_save") && current.ui.language === "zh-CN" && current.tunnel.tunnelId === "unsaved-draft-tunnel";
        }, "post-language full Settings save");
        const persistedAfterFullSave = await runBridge(directory, "get");
        if (persistedAfterFullSave.tunnel.tunnelId !== "unsaved-draft-tunnel" || persistedAfterFullSave.ui.language !== "zh-CN") throw new Error(`full Settings save lost the language or draft: ${JSON.stringify(persistedAfterFullSave)}`);
        return { pendingSaveButtonDisabled, draftDuringLanguageSave, persistedBeforeFullSave: { tunnelId: persistedBeforeFullSave.tunnel.tunnelId, language: persistedBeforeFullSave.ui.language ?? null }, persistedAfterFullSave: { tunnelId: persistedAfterFullSave.tunnel.tunnelId, language: persistedAfterFullSave.ui.language ?? null } };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-13") {
      const directory = await disposableFixture("en");
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        const beforeBytes = await rawConfig(directory);
        await runBridge(directory, "block-temp");
        try { await changeLanguage(app.app, "zh-CN"); } catch { /* the UI rolls back after the failed command */ }
        await waitFor(() => app.app.dom.window.document.documentElement.lang === "en", "language rollback");
        const afterBytes = await rawConfig(directory);
        const saved = await runBridge(directory, "get");
        await runBridge(directory, "unblock-temp");
        if (beforeBytes !== afterBytes || saved.ui.language !== "en" || app.app.dom.window.document.documentElement.lang !== "en") throw new Error(`failure rollback did not preserve config/locale: ${JSON.stringify({ saved, configUnchanged: beforeBytes === afterBytes, htmlLang: app.app.dom.window.document.documentElement.lang })}`);
        return { errorExpected: "CONFIG_WRITE_FAILED", htmlLangAfterFailure: app.app.dom.window.document.documentElement.lang, configUnchanged: beforeBytes === afterBytes, savedLanguage: saved.ui.language, atomicTempFixture: true };
      } finally { await runBridge(directory, "unblock-temp").catch(() => undefined); await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-14" || caseId === "E2E-15" || caseId === "E2E-16") {
      const directory = await disposableFixture();
      try {
        const app = await runChildCase(caseId, directory, "en-US", bundle);
        const englishRoutes = await renderAllRoutes(app.app);
        await clickNav(app.app.dom, 4);
        const approvalBefore = pageText(app.app.dom);
        const beforeTokens = machineTokens(englishRoutes.join(" "));
        await changeLanguage(app.app, "zh-CN");
        const chineseRoutes = await renderAllRoutes(app.app);
        await clickNav(app.app.dom, 4);
        const approvalAfter = pageText(app.app.dom);
        const afterTokens = machineTokens(chineseRoutes.join(" "));
        if (caseId === "E2E-14") return { englishRoutes: englishRoutes.map((text) => text.length), chineseRoutes: chineseRoutes.map((text) => text.length), bothRendered: true };
        if (caseId === "E2E-15") {
          const requiredIdentity = ["read_file", "glob", "write", "edit", "bash", "system-deny-outside", "user-rule-1", "request-m6-uuid", "runtime-m6", "generation-m6", path.join(repoRoot, "fixture.txt")];
          const missing = requiredIdentity.filter((token) => !beforeTokens.includes(token) || !afterTokens.includes(token));
          if (missing.length) throw new Error(`machine identity fixture was not rendered in both locales: ${missing.join(", ")}`);
          if (JSON.stringify(beforeTokens) !== JSON.stringify(afterTokens)) throw new Error(`machine identity changed across locales: ${JSON.stringify({ beforeTokens, afterTokens })}`);
          return { beforeTokens, afterTokens, identical: true };
        }
        const unresolved = app.app.calls.filter((call) => call.command === "runtime_approvals_resolve").length;
        const beforeHasPending = approvalBefore.toLowerCase().includes("pending") || approvalBefore.includes("待处理");
        const afterHasPending = approvalAfter.toLowerCase().includes("pending") || approvalAfter.includes("待处理");
        if (!approvalBefore.includes("request-m6-uuid") || !approvalAfter.includes("request-m6-uuid") || !beforeHasPending || !afterHasPending || unresolved !== 0) throw new Error(`pending approval changed/resolved during locale switch: ${JSON.stringify({ beforeHasPending, afterHasPending, unresolved })}`);
        return { requestId: "request-m6-uuid", beforeHasPending, afterHasPending, afterHasApprovalIdentity: approvalAfter.includes("request-m6-uuid"), resolveCalls: unresolved, actionsRemainReadOnly: true };
      } finally { await fs.rm(directory, { recursive: true, force: true }); }
    }
    if (caseId === "E2E-19") {
      const codes = {};
      for (const locale of ["en-US", "zh-CN"]) {
        const directory = await disposableFixture();
        try {
          try { await runBridge(directory, "control-error"); } catch (error) { codes[locale] = { code: error.code, message: error.message }; }
        } finally { await fs.rm(directory, { recursive: true, force: true }); }
      }
      if (!codes["en-US"] || !codes["zh-CN"] || codes["en-US"].code !== codes["zh-CN"].code) throw new Error(`control error machine code changed: ${JSON.stringify(codes)}`);
      return { source: "real RuntimeSupervisor::get_effective_tools with no child process", codes: { en: codes["en-US"].code, zhCN: codes["zh-CN"].code }, sameMachineCode: true };
    }
    throw new Error(`unknown child case ${caseId}`);
  });
  if (ownedDirectory) await fs.rm(directory, { recursive: true, force: true });
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

function flattenLeaves(value, prefix = "") {
  const leaves = {};
  for (const [key, child] of Object.entries(value)) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) Object.assign(leaves, flattenLeaves(child, next));
    else leaves[next] = child;
  }
  return leaves;
}

function placeholders(value) {
  return [...String(value).matchAll(/\{\{\s*([^},\s]+)(?:,[^}]*)?\s*\}\}/g)].map((match) => match[1]).sort();
}

async function buildIdentity(bundle) {
  const files = [path.join(uiRoot, "dist", "index.html"), bundle, ...((await fs.readdir(path.join(uiRoot, "dist", "assets"))).filter((name) => name.endsWith(".css")).map((name) => path.join(uiRoot, "dist", "assets", name)))];
  const entries = [];
  const digest = createHash("sha256");
  for (const file of files) {
    const bytes = await fs.readFile(file);
    const hash = createHash("sha256").update(bytes).digest("hex");
    entries.push({ path: path.relative(repoRoot, file).replaceAll("\\", "/"), bytes: bytes.length, sha256: hash });
    digest.update(`${file}:${hash}:${bytes.length}\n`);
  }
  return { bundle: path.relative(repoRoot, bundle).replaceAll("\\", "/"), sha256: digest.digest("hex"), files: entries };
}

async function fileIdentity(file) {
  const bytes = await fs.readFile(file);
  return {
    path: path.relative(repoRoot, file).replaceAll("\\", "/"),
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

async function e2e01(bundle) {
  const js = await fs.readFile(bundle, "utf8");
  const [en, zh] = await Promise.all([fs.readFile(path.join(uiRoot, "src", "i18n", "en.json"), "utf8"), fs.readFile(path.join(uiRoot, "src", "i18n", "zh-CN.json"), "utf8")]);
  if (!js.includes("Overview") || !js.includes("概览") || !js.includes("Settings") || !js.includes("设置")) throw new Error("production bundle does not contain both locale sentinel resources");
  return { bundleContainsEnglish: true, bundleContainsZhCN: true, englishSourceBytes: en.length, zhCNSourceBytes: zh.length };
}

async function e2e21(bundle) {
  const en = JSON.parse(await fs.readFile(path.join(uiRoot, "src", "i18n", "en.json"), "utf8"));
  const zh = JSON.parse(await fs.readFile(path.join(uiRoot, "src", "i18n", "zh-CN.json"), "utf8"));
  const enLeaves = flattenLeaves(en), zhLeaves = flattenLeaves(zh);
  const enKeys = Object.keys(enLeaves).sort(), zhKeys = Object.keys(zhLeaves).sort();
  const empty = Object.entries({ en: enLeaves, zhCN: zhLeaves }).flatMap(([locale, leaves]) => Object.entries(leaves).filter(([, value]) => typeof value !== "string" || value.trim() === "").map(([key]) => `${locale}:${key}`));
  const mismatchedPlaceholders = enKeys.filter((key) => JSON.stringify(placeholders(enLeaves[key])) !== JSON.stringify(placeholders(zhLeaves[key])));
  const js = await fs.readFile(bundle, "utf8");
  const supportedConfig = js.match(/supportedLngs:\[\.\.\.([A-Za-z_$][\w$]*)\]/);
  const localeIdentifier = supportedConfig?.[1];
  const localeArray = localeIdentifier ? new RegExp(`${localeIdentifier.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}=\\["en","zh-CN"\\]`) : null;
  const bundleHasEnglishAndZhCNConfig = js.includes('fallbackLng:"en"') && Boolean(supportedConfig && localeArray?.test(js));
  if (JSON.stringify(enKeys) !== JSON.stringify(zhKeys) || empty.length || mismatchedPlaceholders.length || !js.includes("概览") || !js.includes("Overview") || !bundleHasEnglishAndZhCNConfig) throw new Error(`resource integrity failure: keys=${enKeys.length}/${zhKeys.length}, empty=${empty.length}, placeholders=${mismatchedPlaceholders.length}, bundleLocaleConfig=${bundleHasEnglishAndZhCNConfig}`);
  return { supportedLocales: ["en", "zh-CN"], keyCount: enKeys.length, keyParity: true, placeholderParity: true, emptyLeaves: 0, fallback: "en", bundleLocaleConfig: `fallbackLng=en; supportedLngs=[en,zh-CN] via ${localeIdentifier}`, shippedBundleSentinels: true };
}

async function parentMain() {
  const build = runSync("npm", ["run", "build"], { cwd: uiRoot });
  if (build.status !== 0) {
    const result = { status: "FAIL", summary: `npm run build failed\n${build.stdout ?? ""}\n${build.stderr ?? ""}` };
    await fs.mkdir(path.dirname(evidencePath), { recursive: true });
    await fs.writeFile(evidencePath, `# M6 production frontend E2E evidence\n\nBuild: **FAIL**\n\n\`npm run build\`\n\n\`\`\`text\n${result.summary}\n\`\`\`\n`);
    process.stdout.write(JSON.stringify(result));
    process.exitCode = 1;
    return;
  }
  const bundle = (await fs.readdir(path.join(uiRoot, "dist", "assets"))).filter((name) => /^index-.*\.js$/.test(name)).map((name) => path.join(uiRoot, "dist", "assets", name)).sort().at(-1);
  if (!bundle) throw new Error("Vite production JS bundle not found");
  // Always rebuild the bridge from the current checkout before invoking it.
  // A previously existing binary is not evidence that this QA run used the
  // current ConfigStore/RuntimeSupervisor source.
  const bridgeBuild = runSync("cargo", ["build", "-p", "chatgpt-harness-ui", "--example", "m6_config_bridge"]);
  if (bridgeBuild.status !== 0) throw new Error(`ConfigStore bridge build failed\n${bridgeBuild.stdout}\n${bridgeBuild.stderr}`);
  if (!existsSync(bridgeExe)) throw new Error(`ConfigStore bridge build produced no executable: ${bridgeExe}`);
  const identity = await buildIdentity(bundle);
  const bridgeIdentity = {
    binary: await fileIdentity(bridgeExe),
    source: await Promise.all([
      fileIdentity(path.join(uiRoot, "src-tauri", "examples", "m6_config_bridge.rs")),
      fileIdentity(path.join(uiRoot, "src-tauri", "src", "config", "mod.rs")),
      fileIdentity(path.join(uiRoot, "src-tauri", "src", "runtime", "mod.rs")),
    ]),
  };
  const results = [];
  results.push(await caseResult("E2E-01", () => e2e01(bundle)));
  for (const caseId of ["E2E-02", "E2E-03", "E2E-04", "E2E-05", "E2E-06", "E2E-07", "E2E-08", "E2E-09", "E2E-10", "E2E-11", "E2E-12", "E2E-13", "E2E-14", "E2E-15", "E2E-16", "E2E-19"]) {
    const child = runSync(process.execPath, [scriptPath, "--child", "--case", caseId, "--bundle", bundle], { cwd: repoRoot });
    const line = child.stdout?.trim().split(/\r?\n/).filter(Boolean).at(-1);
    if (line) {
      try { results.push(JSON.parse(line)); continue; } catch { /* fall through to a structured failure */ }
    }
    results.push({ caseId, status: "FAIL", summary: `child runner emitted no JSON\nstdout=${child.stdout ?? ""}\nstderr=${child.stderr ?? ""}` });
  }
  results.push(await caseResult("E2E-21", () => e2e21(bundle)));
  const passed = results.filter((result) => result.status === "PASS").length;
  const failed = results.filter((result) => result.status !== "PASS");
  const lines = [
    "# M6 production frontend E2E evidence",
    "",
    `Result: **${failed.length === 0 ? "LOCAL_M6_FRONTEND_E2E=PASS" : "LOCAL_M6_FRONTEND_E2E=FAIL"}** (${passed}/${results.length} cases PASS)`,
    "",
    "## Commands",
    "",
    "- `npm run build`",
    "- `cargo build -p chatgpt-harness-ui --example m6_config_bridge`",
    "- `node ui/scripts/m6-frontend-e2e.mjs`",
    "",
    `Production bundle identity: \`${identity.bundle}\`, aggregate SHA-256 \`${identity.sha256}\`.`,
    "",
    `ConfigStore bridge was freshly rebuilt from the current checkout; binary SHA-256: \`${bridgeIdentity.binary.sha256}\`.`,
    "",
    "The runner executes the built Vite JS in jsdom with the real `TauriHarnessControlClient`. Config commands cross a subprocess boundary into the production `ConfigStore`; read-only Runtime data used to render all routes is a bounded production-equivalent fixture. E2E-10/11 Runtime PID/generation invariants and MCP cases are owned by the companion Runtime E2E runner.",
    "",
    "## Production identity",
    "",
    "| File | Bytes | SHA-256 |",
    "| --- | ---: | --- |",
    ...identity.files.map((file) => `| \`${file.path}\` | ${file.bytes} | \`${file.sha256}\` |`),
    "",
    "Bridge source identity:",
    "",
    ...[bridgeIdentity.binary, ...bridgeIdentity.source].map((file) => `- \`${file.path}\`: ${file.bytes} bytes, SHA-256 \`${file.sha256}\``),
    "",
    "## Case results",
    "",
    ...results.flatMap((result) => [
      `### ${result.caseId} — ${result.status}`,
      "",
      result.evidence ? `\`\`\`json\n${JSON.stringify(result.evidence, null, 2)}\n\`\`\`` : `\`\`\`text\n${result.summary}\n\`\`\``,
      "",
    ]),
  ];
  await fs.mkdir(path.dirname(evidencePath), { recursive: true });
  await fs.writeFile(evidencePath, `${lines.join("\n")}\n`, "utf8");
  process.stdout.write(`${JSON.stringify({ status: failed.length === 0 ? "PASS" : "FAIL", passed, failed: failed.length, evidencePath, identity, bridgeIdentity, results })}\n`);
  if (failed.length) process.exitCode = 1;
}

if (has("--child")) await childMain();
else await parentMain();
