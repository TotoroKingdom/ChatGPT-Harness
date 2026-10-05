import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AppConfig,
  ApprovalRequest,
  BashApprovalDetails,
  PolicyEffectiveSnapshot,
  RuntimeState,
  ToolDescriptor,
} from "@protocol/v1";
import en from "@/i18n/en.json";
import zhCN from "@/i18n/zh-CN.json";
import { bootstrapPresentation, i18n, initializeLanguage } from "@/i18n";
import { resolveLanguage } from "@/i18n/resolve-language";
import {
  activityCategoryKey,
  activityEventKey,
  activityFailureCategoryKey,
  activityLifecycleStateKey,
  activityOutcomeKey,
  bashClassificationKey,
  bashRiskFlagKey,
  errorCodeKey,
  policyReasonKey,
  presentError,
} from "@/i18n/presentation";
import { AppShell } from "@/components/layout/AppShell";
import { LoadingState, EmptyState, ErrorState, DisconnectedState } from "@/components/status/AsyncStates";
import { ControlReadView } from "@/components/status/ControlReadView";
import { FirstRunWizard, SettingsPage } from "@/pages/settings/SettingsPage";
import { OverviewPage } from "@/pages/overview/OverviewPage";
import { ToolsPage } from "@/pages/tools/ToolsPage";
import { PoliciesPage } from "@/pages/policies/PoliciesPage";
import { ApprovalsPage } from "@/pages/approvals/ApprovalsPage";
import { ActivityPage } from "@/pages/activity/ActivityPage";
import type { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import { HarnessClientError } from "@/services/harness/errors";

/*
 * This file is intentionally an independent M6 Integration gate.  It uses
 * real React components and the real resources, with only the harness
 * boundary replaced by typed deterministic fixtures.  Runtime and ConfigStore
 * semantics are covered by the host Integration suite; these cases prove the
 * presentation boundary does not rewrite those values.
 */
const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => state.context,
}));

function flatten(value: Record<string, unknown>, prefix = ""): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof child === "string") result[path] = child;
    else Object.assign(result, flatten(child as Record<string, unknown>, path));
  }
  return result;
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{\{\s*([^},\s]+)(?:,[^}]*)?\s*\}\}/g)].map((match) => match[1]).sort();
}

function config(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    schemaVersion: 2,
    configRevision: "7",
    workspace: { rootPath: "C:\\workspace\\project" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: {
      readFile: { enabled: true },
      glob: { enabled: true },
      write: { enabled: true },
      edit: { enabled: true },
      bash: { enabled: false },
    },
    bash: { autoDetect: true, executablePath: "C:\\Program Files\\Git\\bin\\bash.exe" },
    policies: {
      toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
      userRules: [],
    },
    ...overrides,
  };
}

function runtime(): RuntimeState {
  return {
    runtimeId: "runtime-m6",
    startedAt: "2026-10-04T00:00:00Z",
    lifecycle: "ready",
    readiness: "ready",
    configRevision: "7",
    activeWorkspace: "C:\\workspace\\project",
    securityMode: "normal",
    exposedToolCatalog: [],
    activeRequests: 0,
    pendingApprovals: 1,
    runningExecutions: 0,
  };
}

type HarnessDataFixture = ReturnType<typeof useHarnessData>;

const tools: ToolDescriptor[] = [
  { name: "read_file", description: "Read a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
  { name: "glob", description: "Find workspace paths.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
  { name: "write", description: "Write a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
  { name: "edit", description: "Edit a workspace text file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
  { name: "bash", description: "Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.", implemented: true, enabled: false, exposed: true, effectivePolicy: "ask", scope: "active_workspace", readiness: "unavailable", riskLevel: "high" },
];

function makeDataContext(overrides: Partial<HarnessDataFixture> = {}): HarnessDataFixture {
  const client = new MockHarnessControlClient("ready");
  return {
    client,
    config: config(),
    configMetadata: { configDirectory: "C:\\config", configFile: "C:\\config\\config.json" },
    configError: null,
    eligibility: { status: "ready_to_start", canStart: true, message: "Local Runtime is ready to start." },
    setupRequired: false,
    apiKeyConfigured: false,
    apiKeyObserved: true,
    components: { runtime: "ready", tunnel: "ready", mcp: "ready" },
    harness: { lifecycle: "ready", runtime: "ready", tunnel: "ready", mcp: "ready", activeSessionId: "session-m6", configRevision: "7", lastError: null, tunnelReadiness: { controlPlaneConnected: true, mcpTargetReachable: true, healthCheckPassed: true, controlPlaneRoute: null, controlPlaneError: null } },
    harnessObserved: true,
    overall: "ready",
    restartRequired: false,
    activeRevision: "7",
    lastDiagnostics: null,
    runtime: { status: "success", data: runtime() },
    tools: { status: "success", data: { items: tools } },
    policies: { status: "success", data: { items: [], effective: policySnapshot() } },
    activity: { status: "empty", data: { items: [], latestSequence: "0" } },
    approvals: { status: "empty", data: { items: [] } },
    chooseWorkspace: vi.fn().mockResolvedValue("C:\\workspace\\project"),
    chooseTunnelClient: vi.fn().mockResolvedValue("C:\\workspace\\tunnel-client"),
    detectTunnelClient: vi.fn().mockResolvedValue(null),
    deleteApiKey: vi.fn().mockResolvedValue(undefined),
    refreshConfig: vi.fn().mockResolvedValue(undefined),
    refreshHarness: vi.fn().mockResolvedValue(undefined),
    saveConfig: vi.fn().mockResolvedValue({ ok: true }),
    mutateConfig: vi.fn().mockResolvedValue({ ok: true }),
    setUiLanguage: vi.fn().mockResolvedValue({ ok: true }),
    resetConfig: vi.fn().mockResolvedValue({ ok: true }),
    restartHarness: vi.fn().mockResolvedValue({ ok: true }),
    startHarness: vi.fn().mockResolvedValue({ ok: true }),
    stopHarness: vi.fn().mockResolvedValue({ ok: true }),
    runDiagnostics: vi.fn().mockResolvedValue({ runId: "diagnostic-m6", startedAt: "2026-10-04T00:00:00Z", finishedAt: "2026-10-04T00:00:01Z", overallStatus: "pass", checks: [] }),
    appendActivity: vi.fn(),
    subscribeToActivity: vi.fn().mockResolvedValue(() => undefined),
    refreshApprovals: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function makeSettingsContext(overrides: Partial<HarnessDataFixture> = {}): HarnessDataFixture {
  return makeDataContext(overrides);
}

function makeToolsContext(overrides: Partial<HarnessDataFixture> = {}): HarnessDataFixture {
  const client = new MockHarnessControlClient("ready");
  const effective = vi.fn().mockResolvedValue(tools.map((tool) => ({
    name: tool.name,
    availability: tool.name === "bash" ? "unavailable_by_mode" : "available",
    decision: tool.name === "bash" ? "deny" : "allow",
    readiness: tool.name === "bash" ? "unavailable" : "ready",
  })));
  vi.spyOn(client, "runtimeGetEffectiveTools").mockImplementation(effective);
  return makeDataContext({
    client,
    config: config(),
    tools: { status: "success", data: { items: tools } },
    components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
    runtime: { status: "success", data: runtime() },
    restartRequired: false,
    ...overrides,
  });
}

function policySnapshot(): PolicyEffectiveSnapshot {
  return {
    valid: true,
    status: "valid",
    systemRules: [{ id: "system-deny-outside", name: "System raw name", decision: "deny", reason: "System raw reason", source: "system", editable: false, enabled: true }],
    userRules: [{ id: "user-rule-1", name: "Keep this exact user name", decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "Keep this exact user reason", source: "user", editable: true, enabled: true }],
    toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
    perTool: [{ tool: "read_file", decision: "allow", matchedRuleIds: ["user-rule-1"], reason: "matched_user_rules", availability: "available" }],
  };
}

function approval(status: ApprovalRequest["status"] = "pending"): ApprovalRequest {
  const now = Date.now();
  return {
    requestId: "request-m6-uuid",
    runtimeId: "runtime-m6",
    generationId: "generation-m6",
    tool: "write",
    sanitizedArguments: { path: "C:\\workspace\\x", content: "<redacted>" },
    scopeSummary: "C:\\workspace\\x",
    matchedRuleIds: ["user-rule-1"],
    policyReason: "matched_user_rules",
    riskLevel: "medium",
    createdAt: new Date(now - 60_000).toISOString(),
    expiresAt: new Date(now + 300_000).toISOString(),
    status,
    decision: status === "approved" ? "allow_once" : undefined,
  };
}

function bashApproval(): ApprovalRequest {
  const details: BashApprovalDetails = {
    commandPreview: "git status --short",
    classification: "classified",
    normalizedSegments: [{ executable: "git", argv: ["status", "--short"], argvRedacted: false }],
    riskFlags: [],
    cwd: "active_workspace",
    previewRedacted: false,
    previewTruncated: false,
  };
  return {
    ...approval(),
    requestId: "bash-request-m6",
    tool: "bash",
    scopeSummary: "git status --short",
    sanitizedArguments: { command: "git status --short" },
    riskLevel: "high",
    bashDetails: details,
    allowForSessionEligible: true,
  };
}

function unclassifiedBashApproval(): ApprovalRequest {
  const base = bashApproval();
  return {
    ...base,
    requestId: "bash-unclassified-m6",
    bashDetails: {
      ...base.bashDetails!,
      commandPreview: "git $(dynamic) --short",
      classification: "unclassified_high_risk",
      normalizedSegments: [{ executable: "git", argv: [], argvRedacted: true }],
      riskFlags: ["command_substitution"],
    },
    allowForSessionEligible: false,
    allowForSessionUnavailableReason: "unclassified_high_risk",
  };
}

beforeEach(async () => {
  await initializeLanguage("en");
  document.documentElement.lang = "en";
  state.context = undefined;
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("M6 Integration — locale precedence and bootstrap", () => {
  it("INT-12: saved English overrides a Simplified Chinese system locale", () => {
    expect(resolveLanguage("en", ["zh-CN"])).toBe("en");
  });

  it("INT-13: saved Simplified Chinese overrides an English system locale", () => {
    expect(resolveLanguage("zh-CN", ["en-US"])).toBe("zh-CN");
  });

  it("INT-14: maps supported Simplified Chinese locale variants", () => {
    for (const locale of ["zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN", "zh"]) {
      expect(resolveLanguage(undefined, [locale]), locale).toBe("zh-CN");
    }
  });

  it("INT-15: maps Traditional Chinese locale variants to English", () => {
    for (const locale of ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-CN"]) {
      expect(resolveLanguage(undefined, [locale]), locale).toBe("en");
    }
  });

  it("INT-16: unsupported, missing, and malformed locales fall back to English", () => {
    for (const locales of [["fr-FR"], ["ja-JP"], [""], [], ["invalid!"]]) {
      expect(resolveLanguage(undefined, locales), JSON.stringify(locales)).toBe("en");
    }
  });

  it("INT-17: navigator locale order is deterministic and uses only the first preference", () => {
    expect(resolveLanguage(undefined, ["zh-TW", "zh-CN"])).toBe("en");
    expect(resolveLanguage(undefined, ["zh-CN", "zh-TW"])).toBe("zh-CN");
  });

  it("INT-18: bootstrap resolves i18n and html lang before the App render callback", async () => {
    const renderApp = vi.fn(() => {
      expect(i18n.language).toBe("zh-CN");
      expect(document.documentElement.lang).toBe("zh-CN");
    });
    await bootstrapPresentation(async () => ({ ...config(), ui: { language: "zh-CN" } }), renderApp, ["en-US"]);
    expect(renderApp).toHaveBeenCalledOnce();
  });

  it("INT-19: config read failure still initializes system locale and reaches recovery render", async () => {
    const renderRecovery = vi.fn();
    await bootstrapPresentation(async () => { throw { code: "CONFIG_INVALID" }; }, renderRecovery, ["zh-CN"]);
    expect(renderRecovery).toHaveBeenCalledOnce();
    expect(i18n.language).toBe("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
  });

  it("INT-20: html lang follows initialization and live language changes", async () => {
    await initializeLanguage("en");
    expect(document.documentElement.lang).toBe("en");
    await i18n.changeLanguage("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    await i18n.changeLanguage("en");
    expect(document.documentElement.lang).toBe("en");
  });
});

describe("M6 Integration — resource and shell presentation", () => {
  it("INT-21/22/23: resources have exact key, nonempty value, and placeholder parity", () => {
    const english = flatten(en);
    const chinese = flatten(zhCN);
    expect(Object.keys(chinese).sort()).toEqual(Object.keys(english).sort());
    for (const [key, value] of Object.entries(english)) {
      expect(value.trim(), key).not.toBe("");
      expect(chinese[key].trim(), key).not.toBe("");
      expect(placeholders(chinese[key]), key).toEqual(placeholders(value));
    }
  });

  it("INT-24: all required resource groups exist in both supported locales", () => {
    const groups = ["app", "common", "nav", "status", "overview", "tools", "policies", "activity", "approvals", "settings", "firstRun", "errors", "policy", "bash", "validation"];
    for (const group of groups) {
      expect(en, group).toHaveProperty(group);
      expect(zhCN, group).toHaveProperty(group);
    }
  });

  it("INT-25: AppShell navigation and shared copy switch in place while route and child identity stay stable", async () => {
    state.context = { client: { adapterKind: "real" }, components: { runtime: "ready" } };
    const navigate = vi.fn();
    const { container } = render(<AppShell page="overview" onNavigate={navigate}><input aria-label="raw request id" defaultValue="request-m6-uuid" /></AppShell>);
    const child = screen.getByDisplayValue("request-m6-uuid");
    expect(screen.getByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByRole("heading", { name: "概览", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主导航" })).toBeInTheDocument();
    expect(container).toHaveTextContent("仅本地");
    expect(screen.getByDisplayValue("request-m6-uuid")).toBe(child);
    fireEvent.click(screen.getByRole("button", { name: /工具$/ }));
    expect(navigate).toHaveBeenCalledWith("tools");
  });
});

describe("M6 Integration — primary React surfaces", () => {
  it("INT-26: Settings and First Run expose bilingual labels, help, validation, and language selector", async () => {
    state.context = makeSettingsContext();
    render(<FirstRunWizard onDone={vi.fn()} />);
    expect(screen.getByText("Set up your local Harness.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply and continue" })).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("配置本地 Harness。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "应用并继续" })).toBeInTheDocument();
    cleanup();

    await initializeLanguage("en");
    state.context = makeSettingsContext();
    render(<SettingsPage />);
    expect(screen.getByRole("heading", { name: "General", level: 2 })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Configure" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Control-plane HTTP proxy (optional)" }), { target: { value: "ftp://proxy" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText("Proxy URL must use http:// or https://.")).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("代理 URL 必须使用 http:// 或 https://。" )).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "语言" })).toBeInTheDocument();
  });

  it("INT-26: confirmations, backend errors and save notices follow both mounted page locales", async () => {
    for (const locale of ["en", "zh-CN"] as const) {
      await initializeLanguage(locale);
      const resources = locale === "en" ? en : zhCN;
      const context = makeSettingsContext({ configError: new HarnessClientError("error", "Configuration needs attention.", undefined, "CONFIG_INVALID") });
      state.context = context;
      render(<SettingsPage />);
      expect(screen.getByRole("alert")).toHaveTextContent(resources.errors.codes.configInvalid);
      fireEvent.click(screen.getByRole("button", { name: resources.settings.actions.reset }));
      expect(screen.getByRole("dialog", { name: resources.settings.actions.reset })).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: resources.common.cancel }));
      expect(context.resetConfig).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: resources.m8Settings.configure }));
      fireEvent.change(screen.getByRole("textbox", { name: locale === "en" ? "Tunnel ID (required)" : "Tunnel ID（必填）" }), { target: { value: "confirmed-draft" } });
      fireEvent.click(screen.getByRole("button", { name: resources.m8Settings.apply }));
      expect(await screen.findByText(resources.m8.saved)).toBeInTheDocument();
      expect(context.mutateConfig).toHaveBeenCalledOnce();
      expect(context.saveConfig).not.toHaveBeenCalled();
      cleanup();
    }
  });

  it("INT-27: Tools localizes presentation and preserves tool/path identities plus Bash safety semantics", async () => {
    state.context = makeToolsContext();
    render(<ToolsPage />);
    expect(await screen.findByText("read_file")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^bash/ }));
    expect(screen.getByDisplayValue("C:\\Program Files\\Git\\bin\\bash.exe")).toBeInTheDocument();
    expect(screen.getByText(/Saved here does not mean Runtime validated it/)).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByRole("heading", { name: "工具" })).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText(/这里保存成功不代表 Runtime 已完成校验/)).toBeInTheDocument();
    expect(screen.getByText(/Bash 使用当前工作区作为初始工作目录/)).toBeInTheDocument();
    expect(screen.getByDisplayValue("C:\\Program Files\\Git\\bin\\bash.exe")).toBeInTheDocument();
  });

  it("INT-38: Bash approval warnings keep unclassified/session-ineligible semantics in both locales", async () => {
    const request = unclassifiedBashApproval();
    const approvalsResolve = vi.fn();
    const refreshApprovals = vi.fn().mockResolvedValue(undefined);
    const approvalClient = new MockHarnessControlClient("ready");
    vi.spyOn(approvalClient, "approvalsResolve").mockImplementation(approvalsResolve);
    vi.spyOn(approvalClient, "runtimeStatus").mockResolvedValue(runtime());
    vi.spyOn(approvalClient, "approvalsList").mockResolvedValue({ items: [request] });
    state.context = makeDataContext({ approvals: { status: "success", data: { items: [request] } }, client: approvalClient, refreshApprovals });
    render(<ApprovalsPage />);
    expect(screen.getByText(/Unclassified high risk/)).toBeInTheDocument();
    expect(screen.getByText(/Allow for session is disabled for unclassified high-risk commands/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeDisabled();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText(/未分类高风险/)).toBeInTheDocument();
    expect(screen.getByText(/未分类的高风险命令不能使用/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "本次会话允许" })).toBeDisabled();
    expect(screen.getAllByText("git $(dynamic) --short").length).toBeGreaterThan(0);
    expect(approvalsResolve).not.toHaveBeenCalled();
  });

  it("INT-28: Policies localizes friendly labels while preserving IDs, matchers, and authored text", async () => {
    state.context = makeDataContext({
      config: config(),
      policies: { status: "success", data: { items: [], effective: policySnapshot() } },
      saveConfig: vi.fn().mockResolvedValue({ ok: true }),
      restartHarness: vi.fn(),
    });
    render(<PoliciesPage />);
    expect(screen.getByText("system-deny-outside")).toBeInTheDocument();
    expect(screen.getByText("Keep this exact user name")).toBeInTheDocument();
    expect(screen.getByText("Keep this exact user reason")).toBeInTheDocument();
    expect(screen.getAllByText("read_file", { exact: true }).length).toBeGreaterThan(0);
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getAllByText("拒绝工作区外访问").length).toBeGreaterThan(0);
    expect(screen.getByText("Keep this exact user name")).toBeInTheDocument();
    expect(screen.getByText("Keep this exact user reason")).toBeInTheDocument();
    expect(screen.getAllByText("read_file", { exact: true }).length).toBeGreaterThan(0);
    expect(screen.getAllByText("允许").length).toBeGreaterThan(0);
  });

  it("INT-29: pending Approval presentation changes without resolving or changing request identity", async () => {
    const request = approval();
    const approvalsResolve = vi.fn();
    const refreshApprovals = vi.fn().mockResolvedValue(undefined);
    const approvalClient = new MockHarnessControlClient("ready");
    vi.spyOn(approvalClient, "approvalsResolve").mockImplementation(approvalsResolve);
    vi.spyOn(approvalClient, "runtimeStatus").mockResolvedValue(runtime());
    vi.spyOn(approvalClient, "approvalsList").mockResolvedValue({ items: [request] });
    state.context = makeDataContext({ approvals: { status: "success", data: { items: [request] } }, client: approvalClient, refreshApprovals });
    render(<ApprovalsPage />);
    expect(screen.getAllByText("write", { exact: true }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Allow once" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: /^write/ }));
    fireEvent.click(screen.getByText("Advanced evidence"));
    expect(screen.getAllByText("request-m6-uuid").length).toBeGreaterThan(0);
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getAllByText("request-m6-uuid").length).toBeGreaterThan(0);
    expect(screen.getAllByText("write", { exact: true }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "仅允许本次" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "创建规则" }).length).toBeGreaterThan(0);
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(refreshApprovals).toHaveBeenCalledTimes(1);
  });

  it("INT-30/35: Activity localizes typed values while keeping raw tools, request IDs, and timestamps", async () => {
    const entry = {
      eventId: "event-m6",
      requestId: "request-m6-uuid",
      timestamp: "2026-10-04T00:00:00Z",
      category: "execution",
      eventType: "execution_completed",
      lifecycleState: "completed",
      toolName: "read_file",
      decisionSummary: "allow",
      outcomeSummary: "success",
      failureCategory: "bash_timeout",
    };
    const originalEntry = JSON.stringify(entry);
    const typedKeys = [
      activityCategoryKey("execution"),
      activityEventKey("execution_completed"),
      activityLifecycleStateKey("completed"),
      policyReasonKey("matched_user_rules"),
      activityOutcomeKey("success"),
      activityFailureCategoryKey("bash_timeout"),
      bashClassificationKey("unclassified_high_risk"),
      bashRiskFlagKey("command_substitution"),
    ];
    const read = vi.fn().mockResolvedValue([entry]);
    const activityClient = new MockHarnessControlClient("ready");
    vi.spyOn(activityClient, "runtimeGetActivitySnapshot").mockImplementation(read);
    state.context = makeDataContext({ client: activityClient });
    render(<ActivityPage />);
    expect(await screen.findByText("Execution completed")).toBeInTheDocument();
    expect(screen.getAllByText("read_file").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Success").length).toBeGreaterThan(0);
    const englishMappings = typedKeys.map((key) => i18n.t(key));
    typedKeys.forEach((key) => expect(i18n.t(key)).not.toBe(key));
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("执行已完成")).toBeInTheDocument();
    expect(screen.getAllByText("read_file").length).toBeGreaterThan(0);
    expect(screen.getAllByText("成功").length).toBeGreaterThan(0);
    expect(screen.getByText("2026-10-04T00:00:00Z")).toBeInTheDocument();
    expect(read).toHaveBeenCalledOnce();
    expect(entry.requestId).toBe("request-m6-uuid");
    typedKeys.forEach((key, index) => expect(i18n.t(key)).not.toBe(englishMappings[index]));
    expect(JSON.stringify(entry)).toBe(originalEntry);
  });
});

describe("M6 Integration — typed errors and shared states", () => {
  it("INT-31: representative machine errors resolve to localized explanations", async () => {
    const codes = ["CONFIG_INVALID", "CONFIG_WRITE_FAILED", "RUNTIME_UNAVAILABLE", "RUNTIME_NOT_READY", "TOOL_DISABLED", "POLICY_DENIED", "APPROVAL_DENIED", "BASH_UNAVAILABLE", "BASH_TIMEOUT"];
    for (const code of codes) {
      const key = errorCodeKey(code);
      expect(key).not.toBe("errors.unknown");
      expect(i18n.t(key)).not.toBe(key);
      const english = i18n.t(key);
      await i18n.changeLanguage("zh-CN");
      expect(i18n.t(key)).not.toBe(english);
      await i18n.changeLanguage("en");
    }
  });

  it("INT-32: unknown errors render a localized generic failure and retain only safe detail", async () => {
    expect(presentError({ code: "NEW_UNKNOWN_MACHINE_CODE", message: "secret" }).summaryKey).toBe("errors.unknown");
    expect(presentError({ code: "NEW_UNKNOWN_MACHINE_CODE", message: "secret" }, "safe detail")).toEqual({
      summaryKey: "errors.unknown",
      code: "NEW_UNKNOWN_MACHINE_CODE",
      detail: "safe detail",
    });
    expect(presentError({ code: "NEW_UNKNOWN_MACHINE_CODE", message: "secret" }, "safe detail").summaryKey).not.toContain("allow");
  });

  it("INT-33: loading, empty, error, disconnected, and ControlRead shared states switch together", async () => {
    render(<div>
      <LoadingState />
      <EmptyState />
      <ErrorState message="ignored" code="RUNTIME_NOT_READY" onRetry={vi.fn()} />
      <DisconnectedState />
      <ControlReadView state={{ status: "error", error: { code: "RUNTIME_UNAVAILABLE" } }}>{() => null}</ControlReadView>
    </div>);
    expect(screen.getByText("Loading data")).toBeInTheDocument();
    expect(screen.getByText("No data available")).toBeInTheDocument();
    expect(screen.getByText("Could not load data")).toBeInTheDocument();
    expect(screen.getByText("Runtime unavailable")).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("正在加载数据")).toBeInTheDocument();
    expect(screen.getByText("暂无数据")).toBeInTheDocument();
    expect(screen.getByText("数据加载失败")).toBeInTheDocument();
    expect(screen.getByText("Runtime 不可用")).toBeInTheDocument();
  });

  it("INT-34: accessibility labels and focus identity update with language", async () => {
    state.context = makeSettingsContext();
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Configure" }));
    const input = screen.getByRole("textbox", { name: "Tunnel ID (required)" });
    input.focus();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    const translatedInput = screen.getByRole("textbox", { name: "Tunnel ID（必填）" });
    expect(translatedInput).toBe(input);
    expect(document.activeElement).toBe(input);
    expect(screen.getByRole("combobox", { name: "语言" })).toBeInTheDocument();
  });

  it("INT-35: identifiers are rendered verbatim and never used as translation keys", async () => {
    state.context = makeToolsContext();
    render(<ToolsPage />);
    expect(await screen.findByText("read_file")).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getAllByText("read_file").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /^bash/ }));
    expect(screen.getByDisplayValue("C:\\Program Files\\Git\\bin\\bash.exe")).toBeInTheDocument();
    expect(i18n.t("read_file")).toBe("read_file");
    expect(i18n.t("system-deny-outside")).toBe("system-deny-outside");
    expect(i18n.t("request-m6-uuid")).toBe("request-m6-uuid");
    expect(i18n.t("git status --short")).toBe("git status --short");
  });

  it("INT-37: live language switching emits no Runtime or Harness lifecycle mutation", async () => {
    const runtimeStart = vi.fn();
    const runtimeStop = vi.fn();
    const harnessStart = vi.fn();
    const harnessStop = vi.fn();
    const harnessRestart = vi.fn();
    const setUiLanguage = vi.fn().mockResolvedValue({ ok: true });
    const lifecycleClient = new MockHarnessControlClient("ready");
    vi.spyOn(lifecycleClient, "runtimeStart").mockImplementation(runtimeStart);
    vi.spyOn(lifecycleClient, "runtimeStop").mockImplementation(runtimeStop);
    vi.spyOn(lifecycleClient, "harnessStart").mockImplementation(harnessStart);
    vi.spyOn(lifecycleClient, "harnessStop").mockImplementation(harnessStop);
    vi.spyOn(lifecycleClient, "harnessRestart").mockImplementation(harnessRestart);
    state.context = makeSettingsContext({
      setUiLanguage,
      client: lifecycleClient,
      startHarness: harnessStart,
      stopHarness: harnessStop,
      restartHarness: harnessRestart,
    });
    render(<SettingsPage />);
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await waitFor(() => expect(setUiLanguage).toHaveBeenCalledWith("zh-CN"));
    expect(runtimeStart).not.toHaveBeenCalled();
    expect(runtimeStop).not.toHaveBeenCalled();
    expect(harnessStart).not.toHaveBeenCalled();
    expect(harnessStop).not.toHaveBeenCalled();
    expect(harnessRestart).not.toHaveBeenCalled();
  });
});

describe("M6 Integration — M0–M5 callback regression", () => {
  it("INT-40: both locales preserve lifecycle, config, policy, approval, Bash, and control-read callbacks", async () => {
    const saveConfig = vi.fn().mockResolvedValue({ ok: true });
    const setUiLanguage = vi.fn().mockResolvedValue({ ok: true });
    const startHarness = vi.fn().mockResolvedValue({ ok: true });
    const stopHarness = vi.fn().mockResolvedValue({ ok: true });
    const restartHarness = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeSettingsContext({ saveConfig, setUiLanguage, startHarness, stopHarness, restartHarness });
    render(<SettingsPage />);
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await waitFor(() => expect(setUiLanguage).toHaveBeenCalledWith("zh-CN"));
    expect(saveConfig).not.toHaveBeenCalled();
    expect(startHarness).not.toHaveBeenCalled();
    expect(stopHarness).not.toHaveBeenCalled();
    expect(restartHarness).not.toHaveBeenCalled();

    cleanup();
    const runtimeGetDiagnostics = vi.fn().mockResolvedValue({
      capturedAt: "2026-10-04T00:00:00Z",
      runtime: { readiness: "ready" },
      security: { workspaceValidation: "validated_at_initialization", policyEngineLoaded: true },
      tools: { effectiveCount: 1, declaredCount: 1 },
    });
    const overviewClient = new MockHarnessControlClient("ready");
    vi.spyOn(overviewClient, "runtimeGetDiagnostics").mockImplementation(runtimeGetDiagnostics);
    state.context = makeDataContext({
      overall: "stopped",
      eligibility: { status: "ready_to_start", canStart: true, message: "Harness is ready to start." },
      components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
      harness: { lifecycle: "stopped", runtime: "stopped", tunnel: "stopped", mcp: "stopped", activeSessionId: null, configRevision: null, lastError: null, tunnelReadiness: null },
      runtime: { status: "idle" },
      client: overviewClient,
      startHarness,
      stopHarness,
      restartHarness,
    });
    render(<OverviewPage />);
    fireEvent.click(screen.getByRole("button", { name: /Start Harness|启动 Harness/ }));
    await waitFor(() => expect(startHarness).toHaveBeenCalledOnce());

    cleanup();
    state.context = makeDataContext({
      overall: "ready",
      eligibility: { status: "ready_to_start", canStart: true, message: "Harness is ready to start." },
      components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
      restartRequired: true,
      harness: { lifecycle: "ready", runtime: "ready", tunnel: "stopped", mcp: "ready", activeSessionId: "session-m6", configRevision: "7", lastError: null, tunnelReadiness: null },
      runtime: { status: "success", data: runtime() },
      client: overviewClient,
      startHarness,
      stopHarness,
      restartHarness,
    });
    render(<OverviewPage />);
    fireEvent.click(screen.getByRole("button", { name: /Stop Harness|停止 Harness/ }));
    await waitFor(() => expect(stopHarness).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: /Restart Harness|重启 Harness/ }));
    await waitFor(() => expect(restartHarness).toHaveBeenCalledOnce());

    cleanup();
    const toolSave = vi.fn().mockResolvedValue({ ok: true });
    const toolContext = makeToolsContext({ mutateConfig: toolSave });
    state.context = toolContext;
    render(<ToolsPage />);
    await screen.findByText("read_file");
    fireEvent.click(screen.getByRole("button", { name: "刷新工具" }));
    await waitFor(() => expect(toolContext.client.runtimeGetEffectiveTools).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole("switch", { name: /read_file/ }));
    await waitFor(() => expect(toolSave).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: /^bash/ }));
    fireEvent.click(screen.getByRole("switch", { name: /Auto detect Git Bash|自动检测 Git Bash/ }));
    await waitFor(() => expect(toolSave).toHaveBeenCalledTimes(2));
    const secondIntent = toolSave.mock.calls[1][0] as (latest: AppConfig) => AppConfig;
    expect(secondIntent(config()).bash.autoDetect).toBe(false);

    cleanup();
    const policySave = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeDataContext({
      config: config(),
      policies: { status: "success", data: { items: [], effective: policySnapshot() } },
      mutateConfig: policySave,
      restartHarness,
    });
    render(<PoliciesPage />);
    const readDefault = screen.getByRole("radiogroup", { name: /read_file/ });
    fireEvent.click(within(readDefault).getByRole("radio", { name: /DENY|拒绝/ }));
    await waitFor(() => expect(policySave).toHaveBeenCalledOnce());
    const policyIntent = policySave.mock.calls[0][0] as (latest: AppConfig) => AppConfig;
    expect(policyIntent(config()).policies.toolDefaults.readFile).toBe("deny");

    cleanup();
    const callbackRequest = approval();
    const resolve = vi.fn().mockResolvedValue({ approval: callbackRequest });
    const refresh = vi.fn().mockResolvedValue(undefined);
    const callbackApprovalClient = new MockHarnessControlClient("ready");
    vi.spyOn(callbackApprovalClient, "approvalsResolve").mockImplementation(resolve);
    vi.spyOn(callbackApprovalClient, "runtimeStatus").mockResolvedValue(runtime());
    vi.spyOn(callbackApprovalClient, "approvalsList").mockResolvedValue({ items: [callbackRequest] });
    state.context = makeDataContext({ approvals: { status: "success", data: { items: [callbackRequest] } }, client: callbackApprovalClient, refreshApprovals: refresh });
    render(<ApprovalsPage />);
    fireEvent.click(screen.getByRole("button", { name: /Allow once|仅允许本次/ }));
    await waitFor(() => expect(resolve).toHaveBeenCalledWith("request-m6-uuid", "allow_once"));
    expect(refresh).toHaveBeenCalled();

    cleanup();
    const activityRead = vi.fn().mockResolvedValue([]);
    const callbackActivityClient = new MockHarnessControlClient("ready");
    vi.spyOn(callbackActivityClient, "runtimeGetActivitySnapshot").mockImplementation(activityRead);
    state.context = makeDataContext({ client: callbackActivityClient });
    render(<ActivityPage />);
    await screen.findByText(/No retained activity in this snapshot|此快照中没有保留的活动记录/);
    fireEvent.click(screen.getByRole("button", { name: /Refresh activity|刷新活动记录/ }));
    await waitFor(() => expect(activityRead).toHaveBeenCalledTimes(2));
  });

  it("INT-39: primary JSX source audit and resource key checks remain green in the same npm test gate", () => {
    // The bounded AST audit is executed by source-coverage.test.ts in this
    // same command.  Keep this named case as the explicit M6 gate marker.
    expect(Object.keys(flatten(en)).sort()).toEqual(Object.keys(flatten(zhCN)).sort());
  });
});
