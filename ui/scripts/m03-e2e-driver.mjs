/**
 * M03 packaged local E2E driver.
 *
 * This driver deliberately uses only Node built-ins. It attaches to the real
 * WebView2 target through Chromium DevTools Protocol (CDP), invokes the real
 * Tauri commands from the React WebView, discovers the loopback MCP endpoint
 * owned by the packaged Runtime, and calls the real filesystem tools over
 * Streamable HTTP. It does not use a mock client and it never starts Tunnel.
 *
 * External ChatGPT is a Manual Acceptance boundary for this revised local
 * suite. The result therefore records it as MANUAL_ONLY_NOT_EXECUTED rather
 * than treating the local MCP caller as an external request.
 */

import { createHash } from "node:crypto";
import { execFile, spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import { createServer } from "node:net";
import path from "node:path";
import process from "node:process";

const argv = process.argv.slice(2);

function option(name, fallback = undefined) {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
}

function hasOption(name) {
  return argv.includes(name);
}

const candidateDirectory = path.resolve(option("--candidate", ""));
const appDataRoot = path.resolve(option("--appdata", ""));
const workspaceRoot = path.resolve(option("--workspace", ""));
const resultDirectory = path.resolve(option("--result-dir", ""));
const requestedCase = (option("--case", "all") ?? "all").toUpperCase();
const timeoutMs = Math.max(30_000, Number.parseInt(option("--timeout-seconds", "240"), 10) * 1000);
const runRoot = path.resolve(option("--run-root", path.dirname(appDataRoot)));
const webviewDataRoot = path.join(runRoot, "webview2");
const databasePath = path.join(appDataRoot, "history", "history.sqlite3");
const candidateExecutable = path.join(candidateDirectory, "chatgpt-harness-ui.exe");
const packagedRuntimeExecutable = path.join(candidateDirectory, "runtime", "chatgpt-harness-runtime.exe");
const packagedTunnelExecutable = path.join(candidateDirectory, "tunnel", "tunnel-client.exe");

const cases = [];
const refs = {
  runtimeId: null,
  secondRuntimeId: null,
  read: null,
  policyDenied: null,
  approvalDenied: null,
  postMaintenanceRead: null,
};

let appProcess = null;
let appExit = null;
let cdp = null;
let mcp = null;
let mcpEndpoint = null;
let cdpPort = null;
let currentCase = null;
let generatedSecretSentinel = "M03_SECRET_SENTINEL_NOT_DURABLE";
let generatedApiKeySentinel = "M03_API_KEY_SENTINEL_NOT_DURABLE";
let databaseBackupPath = null;
let retentionObservation = null;

class QaError extends Error {
  constructor(message, code = "QA_ASSERTION_FAILED") {
    super(message);
    this.name = "QaError";
    this.code = code;
  }
}

class BlockedError extends QaError {
  constructor(message) {
    super(message, "QA_BLOCKED");
    this.blocked = true;
  }
}

function normalizePath(value) {
  return path.normalize(String(value)).replace(/[\\/]+$/, "").toLowerCase();
}

function samePath(left, right) {
  return normalizePath(left) === normalizePath(right);
}

function safePath(value) {
  if (typeof value !== "string") return value;
  let result = value;
  const replacements = [
    [workspaceRoot, "<isolated-workspace>"],
    [appDataRoot, "<isolated-appdata>"],
    [candidateDirectory, "<qa-candidate>"],
    [runRoot, "<isolated-run>"],
  ];
  for (const [source, replacement] of replacements) {
    if (!source) continue;
    result = result.replaceAll(source, replacement).replaceAll(source.replaceAll("\\", "/"), replacement);
  }
  result = result.replaceAll(generatedSecretSentinel, "<redacted-sentinel>");
  result = result.replaceAll(generatedApiKeySentinel, "<redacted-secret>");
  return result;
}

function safeValue(value) {
  if (typeof value === "string") return safePath(value);
  if (Array.isArray(value)) return value.map(safeValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, safeValue(item)]));
  }
  return value;
}

function errorInfo(error) {
  if (error && typeof error === "object") {
    const code = typeof error.code === "string" ? error.code : undefined;
    const message = typeof error.message === "string" ? error.message : undefined;
    if (code || message) return safeValue({ code: code ?? "ERROR", message: message ?? String(error) });
    if (error.error && typeof error.error === "object") return errorInfo(error.error);
  }
  return { code: "ERROR", message: safePath(String(error)) };
}

function assertCondition(condition, message, details = undefined) {
  if (!condition) {
    const error = new QaError(message);
    if (details !== undefined) error.details = safeValue(details);
    throw error;
  }
}

function note(label, value) {
  if (currentCase) currentCase.observations.push({ label, value: safeValue(value) });
}

function passAssertion(label, details = undefined) {
  if (!currentCase) return;
  currentCase.assertions.push({ label, ...(details === undefined ? {} : { details: safeValue(details) }) });
}

function requireReference(value, message) {
  if (!value) throw new BlockedError(message);
  return value;
}

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitFor(predicate, label, waitMs = timeoutMs) {
  const started = Date.now();
  let lastError = null;
  while (Date.now() - started < waitMs) {
    try {
      const value = await predicate();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  const suffix = lastError ? `: ${errorInfo(lastError).message}` : "";
  throw new QaError(`Timed out waiting for ${label}${suffix}`, "QA_TIMEOUT");
}

function execFileAsync(file, arguments_, options = {}) {
  return new Promise((resolve, reject) => {
    execFile(file, arguments_, {
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

async function freeTcpPort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : null;
  await new Promise((resolve) => server.close(resolve));
  if (!port) throw new QaError("Could not reserve a free CDP port.", "CDP_PORT_FAILED");
  return port;
}

async function fetchWithTimeout(url, options = {}, waitMs = 10_000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), waitMs);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

class CdpClient {
  constructor(webSocketUrl) {
    this.webSocketUrl = webSocketUrl;
    this.socket = null;
    this.nextId = 1;
    this.pending = new Map();
  }

  async connect() {
    if (typeof WebSocket !== "function") {
      throw new BlockedError("This Node runtime does not provide the built-in WebSocket API.");
    }
    const socket = new WebSocket(this.webSocketUrl);
    this.socket = socket;
    socket.addEventListener("message", (event) => {
      let message;
      try {
        message = JSON.parse(String(event.data));
      } catch (error) {
        this.rejectAll(new QaError(`CDP returned invalid JSON: ${error.message}`, "CDP_PROTOCOL_ERROR"));
        return;
      }
      if (!message.id || !this.pending.has(message.id)) return;
      const pending = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new QaError(message.error.message ?? "CDP command failed", "CDP_COMMAND_FAILED"));
      } else {
        pending.resolve(message.result);
      }
    });
    socket.addEventListener("close", () => this.rejectAll(new QaError("CDP WebSocket closed.", "CDP_CLOSED")));
    socket.addEventListener("error", () => this.rejectAll(new QaError("CDP WebSocket error.", "CDP_SOCKET_ERROR")));
    await new Promise((resolve, reject) => {
      const onOpen = () => {
        socket.removeEventListener("error", onError);
        resolve();
      };
      const onError = () => {
        socket.removeEventListener("open", onOpen);
        reject(new QaError("Could not connect to the real WebView2 CDP target.", "CDP_CONNECT_FAILED"));
      };
      socket.addEventListener("open", onOpen, { once: true });
      socket.addEventListener("error", onError, { once: true });
    });
    await this.call("Runtime.enable");
    return this;
  }

  rejectAll(error) {
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }

  call(method, params = {}) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new QaError("CDP socket is not open.", "CDP_NOT_OPEN"));
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression) {
    const response = await this.call("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (response?.exceptionDetails) {
      throw new QaError(
        response.exceptionDetails.exception?.description
          ?? response.exceptionDetails.text
          ?? "WebView JavaScript evaluation failed.",
        "WEBVIEW_EVALUATION_FAILED",
      );
    }
    return response?.result?.value;
  }

  async invoke(command, args = undefined) {
    const commandJson = JSON.stringify(command);
    const argsJson = args === undefined ? "undefined" : JSON.stringify(args);
    const expression = `(async () => {
      const internals = globalThis.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== "function") {
        return { __m03Error: true, error: { code: "TAURI_INTERNALS_UNAVAILABLE", message: "Tauri invoke bridge is unavailable in the real WebView." } };
      }
      try {
        return await internals.invoke(${commandJson}, ${argsJson});
      } catch (error) {
        return { __m03Error: true, error };
      }
    })()`;
    const value = await this.evaluate(expression);
    if (value?.__m03Error) {
      const error = value.error ?? { code: "TAURI_INVOKE_FAILED", message: "Tauri command rejected." };
      const wrapped = new QaError(error.message ?? "Tauri command rejected.", error.code ?? "TAURI_INVOKE_FAILED");
      wrapped.value = error;
      throw wrapped;
    }
    return value;
  }

  async close() {
    if (this.socket) {
      try {
        this.socket.close();
      } catch {
        // Cleanup is best effort; the owning process is terminated below.
      }
    }
    this.socket = null;
    this.rejectAll(new QaError("CDP closed during cleanup.", "CDP_CLOSED"));
  }
}

class McpClient {
  constructor(endpoint) {
    this.endpoint = endpoint;
    this.sessionId = null;
    this.requestId = 1;
  }

  async post(message, includeSession = true, waitMs = 20_000) {
    const headers = {
      Accept: "application/json, text/event-stream",
      "Content-Type": "application/json",
    };
    if (includeSession && this.sessionId) {
      headers["Mcp-Session-Id"] = this.sessionId;
      headers["MCP-Protocol-Version"] = "2025-03-26";
    }
    const response = await fetchWithTimeout(this.endpoint, {
      method: "POST",
      headers,
      body: JSON.stringify(message),
    }, waitMs);
    const responseSessionId = response.headers.get("mcp-session-id");
    if (responseSessionId) this.sessionId = responseSessionId;
    const text = await response.text();
    if (!response.ok) {
      throw new QaError(`MCP HTTP ${response.status}: ${text.slice(0, 300)}`, "MCP_HTTP_FAILED");
    }
    if (!text.trim()) return null;
    return parseMcpBody(text);
  }

  async initialize() {
    const response = await this.post({
      jsonrpc: "2.0",
      id: this.requestId++,
      method: "initialize",
      params: {
        protocolVersion: "2025-03-26",
        capabilities: {},
        clientInfo: { name: "m03-packaged-local-e2e", version: "1" },
      },
    }, false);
    if (!this.sessionId) throw new QaError("MCP initialize did not return a session id.", "MCP_SESSION_MISSING");
    await this.post({ jsonrpc: "2.0", method: "notifications/initialized", params: {} });
    return response?.result;
  }

  async request(method, params = {}) {
    if (!this.sessionId) await this.initialize();
    const response = await this.post({
      jsonrpc: "2.0",
      id: this.requestId++,
      method,
      params,
    });
    if (response?.error) {
      throw new QaError(response.error.message ?? "MCP JSON-RPC error", "MCP_RPC_ERROR");
    }
    return response?.result;
  }

  async callTool(name, arguments_) {
    return this.request("tools/call", { name, arguments: arguments_ });
  }
}

function parseMcpBody(text) {
  const trimmed = text.trim();
  if (trimmed.startsWith("{")) return JSON.parse(trimmed);
  const dataLines = trimmed
    .split(/\r?\n/)
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).trim())
    .filter(Boolean);
  if (dataLines.length === 0) throw new QaError("MCP returned neither JSON nor SSE data.", "MCP_PROTOCOL_ERROR");
  return JSON.parse(dataLines.at(-1));
}

async function listCdpTargets(port) {
  const response = await fetchWithTimeout(`http://127.0.0.1:${port}/json/list`, {}, 2_000);
  if (!response.ok) throw new QaError(`CDP target list returned HTTP ${response.status}.`, "CDP_TARGET_LIST_FAILED");
  return response.json();
}

async function launchApp() {
  cdpPort = await freeTcpPort();
  await fs.mkdir(webviewDataRoot, { recursive: true });
  appExit = new Promise((resolve) => {
    appProcess = spawn(candidateExecutable, [], {
      cwd: candidateDirectory,
      env: {
        ...process.env,
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${cdpPort}`,
        WEBVIEW2_USER_DATA_FOLDER: webviewDataRoot,
      },
      windowsHide: true,
      stdio: "ignore",
    });
    appProcess.once("exit", (code, signal) => resolve({ code, signal }));
  });

  const target = await waitFor(async () => {
    try {
      const targets = await listCdpTargets(cdpPort);
      return targets.find((item) => item.type === "page" && item.webSocketDebuggerUrl);
    } catch {
      return null;
    }
  }, "the packaged WebView2 CDP page", Math.min(timeoutMs, 45_000));
  cdp = await new CdpClient(target.webSocketDebuggerUrl).connect();
  await waitFor(async () => {
    try {
      return await cdp.evaluate("document.readyState === 'complete' && !!document.body");
    } catch {
      return false;
    }
  }, "the React document", 30_000);
  await waitFor(async () => {
    try {
      return await cdp.evaluate("typeof globalThis.__TAURI_INTERNALS__ === 'object' && typeof globalThis.__TAURI_INTERNALS__.invoke === 'function'");
    } catch {
      return false;
    }
  }, "the real Tauri invoke bridge", 15_000);
  note("cdp", { port: cdpPort, targetType: target.type });
  return target;
}

async function terminateProcessTree(pid) {
  if (!pid) return;
  try {
    await execFileAsync("taskkill.exe", ["/PID", String(pid), "/T", "/F"]);
  } catch {
    // The process may already have exited. The PowerShell owner performs a
    // second candidate-path-scoped cleanup pass after this driver returns.
  }
}

async function closeApp() {
  const activeCdp = cdp;
  const activeProcess = appProcess;
  cdp = null;
  mcp = null;
  mcpEndpoint = null;
  if (activeCdp) {
    try {
      await activeCdp.invoke("runtime_stop");
    } catch {
      // Runtime may already be stopped or the corrupted-store case may have
      // deliberately prevented a new process from starting.
    }
    try {
      await activeCdp.invoke("harness_stop");
    } catch {
      // The local suite intentionally does not start Harness/Tunnel.
    }
    try {
      await activeCdp.invoke("desktop_quit");
    } catch {
      // Fall through to taskkill for a bounded cleanup.
    }
    await activeCdp.close();
  }
  if (activeProcess?.pid) {
    try {
      await Promise.race([appExit, sleep(5_000)]);
    } catch {
      // Fall through to taskkill.
    }
    if (!activeProcess.killed) await terminateProcessTree(activeProcess.pid);
  }
  appProcess = null;
  appExit = null;
}

async function invoke(command, args = undefined) {
  if (!cdp) throw new BlockedError(`Tauri command '${command}' requires an attached WebView.`);
  return cdp.invoke(command, args);
}

async function invokeError(command, args = undefined) {
  try {
    return { ok: true, value: await invoke(command, args) };
  } catch (error) {
    return { ok: false, error: errorInfo(error) };
  }
}

async function reloadReact() {
  assertCondition(cdp, "React reload requires CDP.");
  await cdp.evaluate("location.reload(); true");
  await waitFor(async () => {
    try {
      return await cdp.evaluate("document.readyState === 'complete' && !!document.body");
    } catch {
      return false;
    }
  }, "React reload", 30_000);
  await sleep(400);
}

function makeLocalConfig(root, overrides = {}) {
  return {
    schemaVersion: 3,
    configRevision: "0",
    workspace: { rootPath: root },
    tunnel: {
      tunnelId: "m03-local-only",
      controlPlaneHttpProxy: null,
      tunnelClientPath: null,
      autoDetect: false,
      apiKeyRef: null,
    },
    security: { mode: "normal", developerTestMode: false },
    ui: { language: "en" },
    tools: {
      readFile: { enabled: true },
      glob: { enabled: true },
      write: { enabled: true },
      edit: { enabled: true },
      bash: { enabled: false },
    },
    bash: { autoDetect: false },
    policies: {
      toolDefaults: {
        readFile: "allow",
        glob: "allow",
        write: "ask",
        edit: "allow",
        bash: "ask",
      },
      userRules: [
        {
          id: "m03-deny-edit",
          name: "M03 deny edit fixture",
          enabled: true,
          decision: "deny",
          matcher: { type: "tool", tool: "edit" },
          reason: "M03 local denial fixture.",
        },
      ],
    },
    desktop: {
      approvalNotifications: false,
      connectionNotifications: false,
      backgroundNoticeAcknowledged: true,
      autoStartHarnessOnAppLaunch: false,
    },
    history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
    ...overrides,
  };
}

async function verifyIsolation() {
  const metadata = await invoke("config_metadata");
  assertCondition(metadata && samePath(metadata.configDirectory, appDataRoot), "Tauri appDirectoriesOverride did not isolate config_directory.", metadata);
  passAssertion("Tauri config_metadata resolves inside the disposable app-data root.");
  note("isolation", {
    appDirectoriesOverrideVerified: true,
    configDirectory: "<isolated-appdata>",
    webviewUserDataFolder: "<isolated-webview2>",
  });
}

async function configureProduct() {
  await verifyIsolation();
  const saved = await invoke("config_save", { config: makeLocalConfig(workspaceRoot), apiKey: null });
  assertCondition(saved?.workspace?.rootPath && samePath(saved.workspace.rootPath, workspaceRoot), "The packaged Tauri config did not persist the isolated workspace.", saved);
  passAssertion("The real Tauri config_save command accepted the isolated workspace and local policy matrix.");
  await reloadReact();
  const observed = await invoke("config_get");
  assertCondition(samePath(observed.workspace.rootPath, workspaceRoot), "The reloaded React/Tauri boundary did not observe the isolated workspace.");
  passAssertion("React reload rehydrated the saved config through the real Tauri bridge.");
}

async function getRuntimeProcessRows() {
  const escaped = packagedRuntimeExecutable.replaceAll("'", "''");
  const script = `$rows = @(Get-CimInstance Win32_Process -Filter \"Name = 'chatgpt-harness-runtime.exe'\" | Where-Object { $_.ExecutablePath -and ([IO.Path]::GetFullPath($_.ExecutablePath) -ieq '${escaped}') } | Select-Object ProcessId,ExecutablePath,CommandLine); $rows | ConvertTo-Json -Compress -Depth 5`;
  try {
    const result = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script]);
    return parsePowerShellArray(result.stdout);
  } catch {
    return [];
  }
}

function parsePowerShellArray(text) {
  const trimmed = text.trim();
  if (!trimmed) return [];
  const value = JSON.parse(trimmed);
  return Array.isArray(value) ? value : [value];
}

async function getRuntimeListeningPorts(pid) {
  const script = `$rows = @(Get-NetTCPConnection -State Listen -OwningProcess ${Number(pid)} -ErrorAction SilentlyContinue | Where-Object { $_.LocalAddress -eq '127.0.0.1' } | Select-Object LocalAddress,LocalPort,OwningProcess); $rows | ConvertTo-Json -Compress -Depth 5`;
  try {
    const result = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script]);
    return parsePowerShellArray(result.stdout).map((row) => Number(row.LocalPort)).filter((port) => port > 0);
  } catch {
    return [];
  }
}

async function discoverMcpEndpoint() {
  const endpoint = await waitFor(async () => {
    const processes = await getRuntimeProcessRows();
    for (const runtimeProcess of processes) {
      const ports = await getRuntimeListeningPorts(runtimeProcess.ProcessId);
      for (const port of ports) {
        const candidate = `http://127.0.0.1:${port}/mcp`;
        try {
          const client = new McpClient(candidate);
          await client.initialize();
          return { client, endpoint: candidate, process: runtimeProcess };
        } catch {
          // Another loopback listener may belong to a different local app.
          // Only a successful MCP initialize is accepted as Runtime evidence.
        }
      }
    }
    return null;
  }, "the packaged Runtime loopback MCP endpoint", 30_000);
  mcp = endpoint.client;
  mcpEndpoint = endpoint.endpoint;
  note("runtimeProcess", {
    pid: endpoint.process.ProcessId,
    executable: "<qa-candidate>/runtime/chatgpt-harness-runtime.exe",
    mcpEndpoint: "http://127.0.0.1:<ephemeral>/mcp",
  });
  return endpoint;
}

async function startRuntime() {
  const state = await invoke("runtime_start");
  assertCondition(state?.lifecycle === "ready" && state?.readiness === "ready", "Packaged Runtime did not become ready.", state);
  // Keep the first generation as the primary reference. Later cases start
  // fresh generations to prove authority isolation; case04 records that
  // post-maintenance identity separately in refs.secondRuntimeId.
  if (!refs.runtimeId) refs.runtimeId = state.runtimeId;
  await discoverMcpEndpoint();
  const status = await invoke("runtime_status");
  assertCondition(status.runtimeId === state.runtimeId, "Runtime identity changed during startup.");
  passAssertion("The packaged Tauri Runtime supervisor started one real Runtime/MCP generation.", { runtimeId: state.runtimeId });
  return state;
}

async function stopRuntime() {
  if (!cdp) return;
  const result = await invokeError("runtime_stop");
  if (result.ok) {
    await waitFor(async () => (await invoke("runtime_process_status")).phase === "stopped", "Runtime to stop", 15_000);
    mcp = null;
    mcpEndpoint = null;
    passAssertion("The packaged Runtime stopped through the real Tauri lifecycle command.");
  } else {
    note("runtimeStop", result.error);
  }
}

async function historyQuery(params = {}) {
  return invoke("history_query", { params });
}

async function historyGet(summary) {
  return invoke("history_get", { params: { runtimeId: summary.runtimeId, requestId: summary.requestId } });
}

async function historyStatus() {
  return invoke("history_status");
}

async function activityList(params = { limit: 200 }) {
  return invoke("runtime_activity_list", { params });
}

async function approvalsList() {
  return invoke("runtime_approvals_list", { params: {} });
}

function structured(result) {
  return result?.structuredContent ?? result?.structured_content ?? null;
}

function toolCode(result) {
  return structured(result)?.code ?? null;
}

function resultIsError(result) {
  return result?.isError === true || result?.is_error === true;
}

function summaryKey(summary) {
  return `${summary.runtimeId}:${summary.requestId}`;
}

async function waitForHistory(predicate, label) {
  return waitFor(async () => {
    const result = await historyQuery({ limit: 200 });
    return predicate(result) ? result : null;
  }, label, 20_000);
}

async function waitForApproval(tool = undefined) {
  return waitFor(async () => {
    const result = await approvalsList();
    return result.items.find((item) => item.status === "pending" && (!tool || item.tool === tool)) ?? null;
  }, `pending ${tool ?? "approval"}`, 15_000);
}

async function clickUi(text, selector = "button") {
  const textJson = JSON.stringify(text);
  const result = await cdp.evaluate(`(() => {
    const wanted = ${textJson};
    const nodes = [...document.querySelectorAll(${JSON.stringify(selector)})];
    const target = nodes.find((node) => (node.textContent || '').trim() === wanted);
    if (!target) return false;
    target.click();
    return true;
  })()`);
  return Boolean(result);
}

async function showHistoryUi() {
  const activityClicked = await clickUi("Activity");
  assertCondition(activityClicked, "The real React sidebar did not expose the Activity page.");
  await waitFor(() => cdp.evaluate("document.querySelector('h1')?.textContent === 'Activity'"), "Activity page", 10_000);
  const historyClicked = await clickUi("History", "button[role='radio']");
  assertCondition(historyClicked, "The real React Activity page did not expose the History tab.");
  await waitFor(() => cdp.evaluate("Boolean(document.querySelector('[data-testid^=\"history-row-\"], .empty-inline, [role=\"alert\"]'))"), "History panel", 10_000);
  await sleep(250);
}

async function historyUiSnapshot() {
  return cdp.evaluate(`(() => {
    const inspector = document.querySelector('.context-surface.inspector');
    return {
      body: document.body?.innerText || '',
      rowCount: document.querySelectorAll('[data-testid^="history-row-"]').length,
      historyTab: [...document.querySelectorAll('button[role="radio"]')].find((node) => node.getAttribute('aria-checked') === 'true')?.textContent?.trim() || null,
      inspectorText: inspector?.innerText || null,
      inspectorButtons: inspector ? [...inspector.querySelectorAll('button')].map((node) => (node.textContent || '').trim()).filter(Boolean) : [],
    };
  })()`);
}

async function inspectHistoryUi(requestId) {
  const requestIdJson = JSON.stringify(requestId);
  const clicked = await cdp.evaluate(`(() => {
    const row = [...document.querySelectorAll('[data-testid^="history-row-"]')].find((node) => node.getAttribute('data-testid')?.includes(${requestIdJson}));
    const button = row?.querySelector('button.row-identity');
    if (!button) return false;
    button.click();
    return true;
  })()`);
  assertCondition(clicked, "The real History row could not be opened for detail.");
  await waitFor(() => cdp.evaluate("Boolean(document.querySelector('.context-surface.inspector'))"), "History detail inspector", 10_000);
  return historyUiSnapshot();
}

function findSummary(items, runtimeId, requestId) {
  return items.find((item) => item.runtimeId === runtimeId && item.requestId === requestId);
}

async function case01() {
  await startRuntime();
  const runtimeId = refs.runtimeId;
  const readResult = await mcp.callTool("read_file", { path: "safe-read.txt" });
  const readStructured = structured(readResult);
  assertCondition(!resultIsError(readResult) && readStructured?.content?.includes("M03_SAFE_CONTENT"), "Real MCP read_file did not return the isolated fixture content.", readStructured);
  assertCondition(readStructured?.normalizedPath && !String(readStructured.normalizedPath).includes(generatedSecretSentinel), "Read result was malformed.");

  const deniedResult = await mcp.callTool("edit", {
    path: "mutable.txt",
    oldText: "ORIGINAL_CONTENT",
    newText: "SHOULD_NOT_BE_WRITTEN",
  });
  assertCondition(resultIsError(deniedResult) && toolCode(deniedResult) === "POLICY_DENIED", "The real Runtime did not produce a policy DENY outcome for edit.", deniedResult);
  const mutableAfterDeny = await fs.readFile(path.join(workspaceRoot, "mutable.txt"), "utf8");
  assertCondition(mutableAfterDeny === "ORIGINAL_CONTENT\n", "A policy-denied edit changed the isolated fixture.");

  const askCall = mcp.callTool("write", { path: "ask.txt", content: "M03_ASK_CONTENT" });
  const approval = await waitForApproval("write");
  assertCondition(approval.runtimeId === runtimeId, "ASK approval was attached to the wrong Runtime identity.", approval);
  const approvalResolution = await invoke("runtime_approvals_resolve", {
    params: { requestId: approval.requestId, decision: "deny" },
  });
  assertCondition(approvalResolution?.approval?.status === "denied" && approvalResolution.approval.requestId === approval.requestId, "The real Tauri approval resolve path did not deny the pending request.", approvalResolution);
  const askResult = await askCall;
  assertCondition(resultIsError(askResult) && toolCode(askResult) === "APPROVAL_DENIED", "ASK → DENY did not return the terminal approval outcome.", askResult);
  assertCondition(!(await fs.stat(path.join(workspaceRoot, "ask.txt")).then(() => true).catch(() => false)), "Denied ASK write created a workspace file.");

  const history = await waitForHistory((result) => {
    const read = result.items.find((item) => item.runtimeId === runtimeId && item.toolName === "read_file" && item.outcome === "success");
    const denied = result.items.find((item) => item.runtimeId === runtimeId && item.toolName === "edit" && item.outcome === "denied" && item.policyDecision === "deny");
    const ask = result.items.find((item) => item.runtimeId === runtimeId && item.toolName === "write" && item.approvalState === "denied");
    return read && denied && ask ? { read, denied, ask } : null;
  }, "durable success, DENY, and ASK-terminal History rows");
  refs.read = history.items.find((item) => item.toolName === "read_file" && item.runtimeId === runtimeId && item.outcome === "success");
  refs.policyDenied = history.items.find((item) => item.toolName === "edit" && item.runtimeId === runtimeId && item.policyDecision === "deny");
  refs.approvalDenied = history.items.find((item) => item.toolName === "write" && item.runtimeId === runtimeId && item.approvalState === "denied");
  assertCondition(refs.read && refs.policyDenied && refs.approvalDenied, "Expected local History references were not captured.");
  const approvalDetail = await historyGet(refs.approvalDenied);
  assertCondition(approvalDetail.terminalApproval?.requestId === refs.approvalDenied.requestId, "Terminal Approval evidence was not correlated by Runtime/request identity.", approvalDetail);
  assertCondition(approvalDetail.events.every((event) => event.runtimeId === runtimeId && event.requestId === refs.approvalDenied.requestId), "History timeline contains an identity mismatch.", approvalDetail.events);

  const activity = await activityList({ limit: 200 });
  const activityForRead = activity.items.filter((item) => item.runtimeId === runtimeId && item.requestId === refs.read.requestId);
  assertCondition(activityForRead.length > 0, "Live Activity did not expose the durable request identity.");
  await showHistoryUi();
  const runningUi = await historyUiSnapshot();
  assertCondition(runningUi.rowCount >= 3, "The packaged React History UI did not show the real Runtime rows while Runtime was active.", runningUi);
  const runningDetailUi = await inspectHistoryUi(refs.read.requestId);
  assertCondition(runningDetailUi.inspectorText?.includes(refs.read.requestId)
    && runningDetailUi.inspectorText?.includes(runtimeId), "The packaged History detail UI did not show the live runtime/request identity.", runningDetailUi);
  passAssertion("Real local MCP calls produced success, policy DENY, and terminal ASK→DENY evidence.", {
    runtimeId,
    requestIds: [refs.read.requestId, refs.policyDenied.requestId, refs.approvalDenied.requestId],
  });
  passAssertion("The packaged Activity/History React UI and durable projections correlate through runtime_id + request_id and terminal approval evidence.");
  note("historyRows", history.items.filter((item) => item.runtimeId === runtimeId).map((item) => ({ tool: item.toolName, outcome: item.outcome, policy: item.policyDecision, approval: item.approvalState })));

  await stopRuntime();
  const offlineBeforeAppRestart = await historyQuery({ limit: 200 });
  assertCondition(findSummary(offlineBeforeAppRestart.items, runtimeId, refs.read.requestId), "History was not queryable with Runtime stopped.");
  passAssertion("Tauri read-only History query works after Runtime stop.");
  await closeApp();
  await launchApp();
  await verifyIsolation();
  const offlineAfterAppRestart = await historyQuery({ limit: 200 });
  assertCondition(findSummary(offlineAfterAppRestart.items, runtimeId, refs.approvalDenied.requestId), "History did not survive packaged app quit/reopen.");
  const processAfterRestart = await invoke("runtime_process_status");
  assertCondition(processAfterRestart.phase === "stopped", "Offline History reopen unexpectedly started Runtime.", processAfterRestart);
  passAssertion("The isolated packaged app reopened with durable History and Runtime stopped.");
}

async function case02() {
  requireReference(refs.read, "E2E-01 did not produce a success History reference.");
  requireReference(refs.approvalDenied, "E2E-01 did not produce a terminal approval History reference.");
  const processStatus = await invoke("runtime_process_status");
  assertCondition(processStatus.phase === "stopped", "E2E-02 must begin with Runtime stopped.");

  const all = await historyQuery({ limit: 200 });
  const readOnly = await historyQuery({ tool: "read_file", limit: 200 });
  const denied = await historyQuery({ outcome: "denied", limit: 200 });
  const askPolicy = await historyQuery({ policyDecision: "ask", limit: 200 });
  const approvalDenied = await historyQuery({ approvalState: "denied", limit: 200 });
  assertCondition(readOnly.items.every((item) => item.toolName === "read_file"), "Tool filter returned a non-read_file row.");
  assertCondition(denied.items.some((item) => item.requestId === refs.policyDenied.requestId) && denied.items.some((item) => item.requestId === refs.approvalDenied.requestId), "Outcome filter did not return both denied families.");
  assertCondition(askPolicy.items.every((item) => item.policyDecision === "ask"), "Policy filter returned a non-ASK row.");
  assertCondition(approvalDenied.items.every((item) => item.approvalState === "denied"), "Approval filter returned a non-denied row.");
  passAssertion("Offline History supports time-independent Tool/outcome/policy/approval filters.");

  const pageOne = await historyQuery({ limit: 1 });
  const pageTwo = pageOne.nextCursor ? await historyQuery({ limit: 1, cursor: pageOne.nextCursor }) : null;
  if (pageOne.nextCursor) {
    assertCondition(pageTwo && pageTwo.items.length > 0, "History next-page cursor returned no second page.");
    const firstKeys = new Set(pageOne.items.map(summaryKey));
    assertCondition(pageTwo.items.every((item) => !firstKeys.has(summaryKey(item))), "History keyset pagination duplicated a row.");
    passAssertion("History keyset pagination returns a distinct next page.");
  } else {
    note("pagination", "Only one row was available; cursor path was not emitted by the product for this fixture.");
  }

  await showHistoryUi();
  const ui = await historyUiSnapshot();
  assertCondition(ui.rowCount >= 3 || ui.body.includes("No retained History"), "React History UI did not render the offline query result.", ui);
  assertCondition(!ui.body.includes(appDataRoot) && !ui.body.includes(databasePath), "History UI exposed an isolated physical AppData/DB path.");
  const detailUi = await inspectHistoryUi(refs.approvalDenied.requestId);
  const inspectorActionText = detailUi.inspectorButtons.join(" ").toLowerCase();
  assertCondition(!/(approve|allow|deny|retry|authorize|replay)/i.test(inspectorActionText), "History detail exposed a live authorization action.", detailUi.inspectorButtons);
  passAssertion("Packaged React History list/detail is usable offline and contains no Approve/Deny/replay action.");

  const staleResolution = await invokeError("runtime_approvals_resolve", {
    params: { requestId: refs.approvalDenied.requestId, decision: "allow_once" },
  });
  assertCondition(!staleResolution.ok, "A historical approval was actionable while Runtime was stopped.", staleResolution);
  const historyAfterStaleAttempt = await historyQuery({ limit: 200 });
  assertCondition(historyAfterStaleAttempt.items.some((item) => item.requestId === refs.approvalDenied.requestId && item.approvalState === "denied"), "Historical approval state changed after a rejected stale action.");
  assertCondition(!(await fs.stat(path.join(workspaceRoot, "ask.txt")).then(() => true).catch(() => false)), "A stale historical approval created a file.");
  passAssertion("Historical approval evidence cannot restore Approval/SessionGrant authority.", staleResolution.error);
  note("offline", { runtimePhase: processStatus.phase, historyRows: all.items.length, staleApprovalError: staleResolution.error });
}

async function runPythonSqlite(database, script, extraArguments = []) {
  const arguments_ = ["-c", script, database, ...extraArguments];
  try {
    return await execFileAsync("python", arguments_, { maxBuffer: 4 * 1024 * 1024 });
  } catch (pythonError) {
    if (pythonError?.code !== "ENOENT") throw pythonError;
    return execFileAsync("py", ["-3", ...arguments_], { maxBuffer: 4 * 1024 * 1024 });
  }
}

async function corruptDatabase() {
  await fs.mkdir(path.dirname(databasePath), { recursive: true });
  databaseBackupPath = path.join(runRoot, "history.sqlite3.qa-backup");
  await fs.copyFile(databasePath, databaseBackupPath);
  await removeIfPresent(`${databasePath}-wal`);
  await removeIfPresent(`${databasePath}-shm`);
  await fs.writeFile(databasePath, Buffer.from("M03 disposable corruption fixture; never a real user database."));
}

async function restoreDatabase() {
  assertCondition(databaseBackupPath, "Corruption case has no isolated database backup.");
  await removeIfPresent(databasePath);
  await removeIfPresent(`${databasePath}-wal`);
  await removeIfPresent(`${databasePath}-shm`);
  await fs.copyFile(databaseBackupPath, databasePath);
}

async function removeIfPresent(file) {
  try {
    await fs.rm(file, { force: true });
  } catch {
    // The file is disposable and cleanup is retried by the outer owner.
  }
}

async function ageClosedHistory() {
  const script = `import sqlite3, sys\n\ndb = sys.argv[1]\nold = '2000-01-01T00:00:00.000Z'\ncon = sqlite3.connect(db)\ntry:\n    con.execute("UPDATE request_summary SET first_seen_at_utc = ?, last_seen_at_utc = ? WHERE terminal_state <> 'open'", (old, old))\n    con.execute("UPDATE evidence_events SET occurred_at_utc = ?, recorded_at_utc = ? WHERE (runtime_id, request_id) IN (SELECT runtime_id, request_id FROM request_summary WHERE terminal_state <> 'open')", (old, old))\n    con.execute("UPDATE approval_history SET requested_at_utc = ?, terminal_at_utc = ? WHERE (runtime_id, request_id) IN (SELECT runtime_id, request_id FROM request_summary WHERE terminal_state <> 'open')", (old, old))\n    con.commit()\nfinally:\n    con.close()\n`;
  await runPythonSqlite(databasePath, script);
}

async function saveHistoryConfig(retentionDays, maxStorageMiB = 32) {
  const config = await invoke("config_get");
  config.history = { enabled: true, retentionDays, maxStorageMiB };
  const saved = await invoke("config_save", { config, apiKey: null });
  assertCondition(saved.history.retentionDays === retentionDays && saved.history.maxStorageMiB === maxStorageMiB, "History configuration did not persist the requested QA bounds.", saved.history);
  return saved;
}

async function case03() {
  requireReference(refs.approvalDenied, "E2E-03 needs E2E-01 durable data.");
  await stopRuntime();
  await closeApp();

  await corruptDatabase();
  await launchApp();
  const corruptStatus = await invokeError("history_status");
  assertCondition(!corruptStatus.ok && corruptStatus.error.code === "EVIDENCE_CORRUPT", "Corrupt isolated History was not surfaced as EVIDENCE_CORRUPT.", corruptStatus);
  const corruptQuery = await invokeError("history_query", { params: { limit: 50 } });
  assertCondition(!corruptQuery.ok, "Corrupt History unexpectedly returned data instead of surfacing degradation.");
  await showHistoryUi();
  const corruptUi = await historyUiSnapshot();
  assertCondition(corruptUi.body.includes("History") && corruptUi.body.includes("EVIDENCE_CORRUPT"), "React History UI did not show the typed corrupt storage warning.", corruptUi.body);
  passAssertion("A disposable corrupt SQLite fixture is preserved and surfaced through Tauri/React without recreation.", corruptStatus.error);

  await closeApp();
  await restoreDatabase();
  await launchApp();
  const recoveredStatus = await historyStatus();
  assertCondition(recoveredStatus.state === "healthy", "Restoring the isolated recovery copy did not return History to healthy state.", recoveredStatus);
  passAssertion("The isolated database recovery path returned a healthy read-only History status.");

  await saveHistoryConfig(1, 32);
  const beforeRetention = await historyQuery({ limit: 200 });
  const beforeKeys = new Set(beforeRetention.items.map(summaryKey));
  assertCondition(beforeKeys.size > 0, "Retention fixture had no historical families before aging.");
  await closeApp();
  await ageClosedHistory();
  await launchApp();
  await startRuntime();
  const afterRetention = await waitForHistory((result) => {
    const oldRows = result.items.filter((item) => beforeKeys.has(summaryKey(item)));
    return oldRows.length === 0 ? result : null;
  }, "retention to remove aged closed request families");
  retentionObservation = { before: beforeKeys.size, after: afterRetention.items.length };
  assertCondition(afterRetention.items.every((item) => !beforeKeys.has(summaryKey(item))), "Age retention left an aged closed request family behind.", afterRetention.items);
  const retainedDatabaseText = (await fs.readFile(databasePath)).toString("utf8");
  assertCondition(!retainedDatabaseText.includes(generatedSecretSentinel), "Retention fixture leaked the privacy sentinel into SQLite.");
  passAssertion("Runtime startup retention removed whole aged request families while preserving the SQLite schema.", retentionObservation);

  const safeFileBeforeClear = await fs.readFile(path.join(workspaceRoot, "safe-read.txt"), "utf8");
  const onlineAskCall = mcp.callTool("write", { path: "online-clear-ask.txt", content: "M03_ONLINE_CLEAR" });
  const onlineApproval = await waitForApproval("write");
  const clearOnline = await invoke("history_clear");
  const pendingAfterOnlineClear = await approvalsList();
  assertCondition(pendingAfterOnlineClear.items.some((item) => item.requestId === onlineApproval.requestId && item.status === "pending"), "Online clear mutated the currently open approval request.", pendingAfterOnlineClear);
  const onlineResolution = await invoke("runtime_approvals_resolve", {
    params: { requestId: onlineApproval.requestId, decision: "deny" },
  });
  assertCondition(onlineResolution.approval.status === "denied", "Online clear fixture could not terminate its still-live approval.");
  const onlineAskResult = await onlineAskCall;
  assertCondition(toolCode(onlineAskResult) === "APPROVAL_DENIED", "Online clear changed the live Runtime policy/approval result.", onlineAskResult);
  passAssertion("Online clear respects the Runtime writer barrier and leaves the current open approval actionable only through live Approval authority.", clearOnline);

  await stopRuntime();
  const clearOffline = await invoke("history_clear");
  const afterOfflineClear = await historyQuery({ limit: 200 });
  assertCondition(afterOfflineClear.items.length === 0, "Offline clear did not remove terminal History families.", afterOfflineClear.items);
  const safeFileAfterClear = await fs.readFile(path.join(workspaceRoot, "safe-read.txt"), "utf8");
  assertCondition(safeFileAfterClear === safeFileBeforeClear, "History clear touched a Workspace fixture.");
  const afterClearStatus = await historyStatus();
  assertCondition(afterClearStatus.state === "healthy", "Offline clear left the isolated History schema unusable.", afterClearStatus);
  passAssertion("Offline clear acquires isolated ownership, leaves the schema usable, and never touches Workspace files.", clearOffline);
  note("storageMatrix", {
    corrupt: corruptStatus.error.code,
    recovered: recoveredStatus.state,
    retention: retentionObservation,
    onlineClear: clearOnline,
    offlineClear: clearOffline,
  });
}

async function processCountForExecutable(executable) {
  const escaped = executable.replaceAll("'", "''");
  const script = `$rows = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and ([IO.Path]::GetFullPath($_.ExecutablePath) -ieq '${escaped}') } | Select-Object ProcessId,ExecutablePath,CommandLine); $rows | ConvertTo-Json -Compress -Depth 5`;
  try {
    const result = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script]);
    return parsePowerShellArray(result.stdout);
  } catch {
    return [];
  }
}

async function databasePrivacyScan() {
  const bytes = await fs.readFile(databasePath);
  const text = bytes.toString("utf8");
  const forbidden = [
    generatedSecretSentinel,
    generatedApiKeySentinel,
    workspaceRoot,
    workspaceRoot.replaceAll("\\", "/"),
    "commandPreview",
    "stdout",
    "stderr",
  ];
  const leaked = forbidden.filter((sentinel) => text.includes(sentinel));
  assertCondition(leaked.length === 0, "Durable SQLite privacy scan found a forbidden sentinel.", leaked);
  return { scannedBytes: bytes.length, forbiddenSentinels: forbidden.length, leaked: [] };
}

async function case04() {
  const newRuntime = await startRuntime();
  const newRuntimeId = newRuntime.runtimeId;
  refs.secondRuntimeId = newRuntimeId;
  assertCondition(newRuntimeId && newRuntimeId !== refs.read?.runtimeId, "Post-maintenance Runtime did not receive a new identity.");
  const oldApprovalAttempt = await invokeError("runtime_approvals_resolve", {
    params: { requestId: refs.approvalDenied?.requestId ?? "m03-old-approval", decision: "allow_once" },
  });
  assertCondition(!oldApprovalAttempt.ok, "A cleared historical approval was accepted by the new Runtime generation.", oldApprovalAttempt);

  const tools = await invoke("runtime_tools_list");
  const policies = await invoke("runtime_policies_list");
  const diagnostics = await invoke("runtime_diagnostics_run");
  assertCondition(tools.items.some((item) => item.name === "read_file" && item.exposed), "Post-maintenance read_file disappeared from the live MCP catalog.", tools);
  assertCondition(policies.effective?.perTool?.some((item) => item.tool === "edit" && item.decision === "deny"), "Post-maintenance policy DENY projection changed.", policies);
  assertCondition(diagnostics.overallStatus === "pass" || diagnostics.overall_status === "pass", "Post-maintenance Runtime diagnostics did not pass.", diagnostics);
  passAssertion("Post-maintenance Runtime tools, policy authority, and diagnostics remain live and independent of History.");

  const readResult = await mcp.callTool("read_file", { path: "safe-read.txt" });
  assertCondition(!resultIsError(readResult) && structured(readResult)?.content?.includes("M03_SAFE_CONTENT"), "Post-maintenance local MCP read_file failed.", readResult);
  const history = await waitForHistory((result) => result.items.find((item) => item.runtimeId === newRuntimeId && item.toolName === "read_file" && item.outcome === "success") ?? null, "post-maintenance durable read evidence");
  refs.postMaintenanceRead = history.items.find((item) => item.runtimeId === newRuntimeId && item.toolName === "read_file" && item.outcome === "success");
  assertCondition(refs.postMaintenanceRead, "Post-maintenance read evidence was not persisted with the new Runtime identity.");
  passAssertion("A fresh local Tool call succeeds after corruption recovery, retention, and online/offline clear.", { runtimeId: newRuntimeId, requestId: refs.postMaintenanceRead.requestId });

  const privacy = await databasePrivacyScan();
  const ui = await historyUiSnapshot();
  assertCondition(!ui.body.includes(appDataRoot) && !ui.body.includes(databasePath) && !ui.body.includes(generatedSecretSentinel), "History UI exposed a physical privacy sentinel/path after maintenance.", ui.body);
  const metadata = await invoke("config_metadata");
  assertCondition(samePath(metadata.configDirectory, appDataRoot), "AppData isolation drifted after maintenance.", metadata);
  const runtimeProcesses = await processCountForExecutable(packagedRuntimeExecutable);
  const tunnelProcesses = await processCountForExecutable(packagedTunnelExecutable);
  assertCondition(runtimeProcesses.length === 1, "The local suite did not end with exactly one packaged Runtime process.", runtimeProcesses);
  assertCondition(tunnelProcesses.length === 0, "The local suite unexpectedly launched a Tunnel process.", tunnelProcesses);
  passAssertion("Privacy scan finds no secret/path/raw-output/Bash-preview sentinel in durable SQLite or History UI.", privacy);
  passAssertion("No duplicate packaged Runtime and no Tunnel process exist in the local boundary suite.", { runtimeCount: runtimeProcesses.length, tunnelCount: tunnelProcesses.length });
  note("authority", { historicalApprovalResolveRejected: true, currentRuntimeId: newRuntimeId, tunnelStarted: false });
}

async function runCase(id, name, handler) {
  const entry = {
    id,
    name,
    verdict: "INCONCLUSIVE",
    startedAtUtc: new Date().toISOString(),
    assertions: [],
    observations: [],
  };
  cases.push(entry);
  currentCase = entry;
  try {
    await handler();
    entry.verdict = "PASS";
  } catch (error) {
    entry.verdict = error?.blocked ? "BLOCKED" : "FAIL";
    entry.error = errorInfo(error);
    if (error?.details !== undefined) entry.error.details = safeValue(error.details);
  } finally {
    entry.finishedAtUtc = new Date().toISOString();
    currentCase = null;
  }
  return entry;
}

function selectedCases() {
  const definitions = [
    ["E2E-01", "local success / DENY / ASK-terminal evidence across Runtime and app restart", case01],
    ["E2E-02", "offline History filters/detail and historical approval non-actionability", case02],
    ["E2E-03", "isolated storage degradation, recovery, retention, and clear", case03],
    ["E2E-04", "privacy, authority isolation, and post-maintenance local Tool regression", case04],
  ];
  if (requestedCase === "ALL") return definitions;
  const selected = definitions.filter(([id]) => id === requestedCase);
  if (selected.length === 0) throw new QaError(`Unknown --case ${requestedCase}.`, "QA_ARGUMENT_ERROR");
  return selected;
}

function localVerdict() {
  if (cases.some((entry) => entry.verdict === "FAIL")) return "FAIL";
  if (cases.some((entry) => entry.verdict === "BLOCKED")) return "BLOCKED";
  if (cases.some((entry) => entry.verdict === "INCONCLUSIVE")) return "INCONCLUSIVE";
  return "PASS";
}

function buildResult(topLevelError = null) {
  const local = localVerdict();
  return {
    schema: "m03-packaged-local-e2e-result-v1",
    scope: "M03 revised local packaged E2E boundary",
    verdict: topLevelError ? "BLOCKED" : local,
    acceptanceClaim: "NOT_CLAIMED",
    externalChatGPT: "MANUAL_ONLY_NOT_EXECUTED",
    tunnel: "NOT_STARTED_BY_LOCAL_SUITE",
    candidate: {
      packagePath: "<qa-candidate>",
      executableSha256: null,
      appVersion: null,
      runtimeVersion: null,
      buildSha: null,
      sourceDirty: null,
    },
    isolation: {
      appDirectoriesOverride: "absolute disposable path supplied at build time",
      appDataRoot: "<isolated-appdata>",
      webviewUserDataFolder: "<isolated-webview2>",
      workspace: "<isolated-workspace>",
      realUserAppDataTouched: false,
      realUserHistoryTouched: false,
    },
    cases,
    references: safeValue(refs),
    storage: safeValue({ database: "<isolated-appdata>/history/history.sqlite3", retention: retentionObservation }),
    topLevelError: topLevelError ? errorInfo(topLevelError) : null,
    generatedAtUtc: new Date().toISOString(),
  };
}

async function loadCandidateMetadata(result) {
  try {
    const metadata = JSON.parse(await fs.readFile(path.join(candidateDirectory, "m03-build-metadata.json"), "utf8"));
    const manifest = metadata.manifest ?? {};
    result.candidate.executableSha256 = metadata.executableSha256 ?? null;
    result.candidate.appVersion = manifest.app_version ?? null;
    result.candidate.runtimeVersion = manifest.runtime_version ?? null;
    result.candidate.buildSha = manifest.build_sha ?? null;
    result.candidate.sourceDirty = manifest.source_dirty ?? null;
  } catch (error) {
    note("candidateMetadata", errorInfo(error));
  }
}

async function writeResults(result) {
  await fs.mkdir(resultDirectory, { recursive: true });
  const jsonPath = path.join(resultDirectory, "m03-e2e-result.json");
  const markdownPath = path.join(resultDirectory, "m03-e2e-result.md");
  // buildResult() runs before the cases so the runner can still emit a
  // structured failure. Refresh mutable evidence here so a successful report
  // also carries the Runtime/request references and retention observation.
  result.references = safeValue(refs);
  result.storage = safeValue({
    database: "<isolated-appdata>/history/history.sqlite3",
    retention: retentionObservation,
  });
  await fs.writeFile(jsonPath, JSON.stringify(safeValue(result), null, 2) + "\n", "utf8");
  const rows = result.cases.map((entry) => `| ${entry.id} | ${entry.verdict} | ${entry.assertions.length} | ${entry.error?.message ?? ""} |`).join("\n");
  const markdown = [
    "# M03 packaged local E2E result",
    "",
    `- Local runner verdict: **${result.verdict}**`,
    `- Scope: ${result.scope}`,
    "- External ChatGPT: **MANUAL_ONLY_NOT_EXECUTED**",
    "- Tunnel: **NOT_STARTED_BY_LOCAL_SUITE**",
    "- Acceptance claim: **NOT_CLAIMED**",
    "- AppData/WebView2: isolated disposable roots; real user AppData/History touched: **false**",
    "",
    "| Case | Verdict | Assertions | Error |",
    "| --- | --- | ---: | --- |",
    rows || "| — | — | 0 | no case selected |",
    "",
    "Machine-readable detail is in `m03-e2e-result.json`.",
    "",
  ].join("\n");
  await fs.writeFile(markdownPath, markdown, "utf8");
  process.stdout.write(JSON.stringify({ resultJson: jsonPath, resultMarkdown: markdownPath, verdict: result.verdict }) + "\n");
}

async function cleanup() {
  if (cdp || appProcess) await closeApp();
  if (databaseBackupPath) await removeIfPresent(databaseBackupPath);
}

async function main() {
  assertCondition(process.platform === "win32", "M03 packaged E2E runner requires Windows WebView2/Tauri.", { platform: process.platform });
  assertCondition(candidateDirectory && appDataRoot && workspaceRoot && resultDirectory, "--candidate, --appdata, --workspace, and --result-dir are required.");
  assertCondition(await fs.stat(candidateExecutable).then((stat) => stat.isFile()).catch(() => false), "Packaged UI executable is missing.", candidateExecutable);
  assertCondition(await fs.stat(packagedRuntimeExecutable).then((stat) => stat.isFile()).catch(() => false), "Packaged Runtime executable is missing.", packagedRuntimeExecutable);
  assertCondition(samePath(path.dirname(appDataRoot), runRoot) || normalizePath(appDataRoot).startsWith(normalizePath(runRoot) + path.sep), "AppData root must be inside the disposable run root.");
  assertCondition(normalizePath(workspaceRoot).startsWith(normalizePath(runRoot) + path.sep), "Workspace fixture must be inside the disposable run root.");
  await fs.mkdir(resultDirectory, { recursive: true });
  const result = buildResult();
  await loadCandidateMetadata(result);
  await launchApp();
  await configureProduct();
  for (const [id, name, handler] of selectedCases()) {
    await runCase(id, name, handler);
  }
  return result;
}

let result = null;
let topLevelError = null;
try {
  result = await main();
} catch (error) {
  topLevelError = error;
  result = buildResult(topLevelError);
} finally {
  try {
    await cleanup();
  } catch (error) {
    topLevelError = topLevelError ?? error;
    result = result ?? buildResult(topLevelError);
    result.cleanupError = errorInfo(error);
  }
  if (!result) result = buildResult(topLevelError ?? new QaError("Runner did not produce a result."));
  await writeResults(result);
}

if (topLevelError || result.verdict !== "PASS") process.exitCode = 1;
