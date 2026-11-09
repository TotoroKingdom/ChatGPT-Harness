import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TauriHarnessControlClient } from "@/services/harness/local-client";
import { RuntimeOverview } from "./overview/RuntimeOverview";
import { OverviewPage } from "./overview/OverviewPage";
import { ActivityPage } from "./activity/ActivityPage";
import { ToolsPage } from "./tools/ToolsPage";
import { i18n } from "@/i18n";
import { toolAvailabilityKey } from "@/i18n/presentation";
import type { ToolAvailability } from "@/services/harness/control-read";
import type { AppConfig } from "@protocol/v1";

const tauri = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => context }));
let context = makeContext();
function makeContext() {
  const config: AppConfig = { schemaVersion: 3, configRevision: "saved-1", workspace: { rootPath: "D:\\example" }, tunnel: { tunnelId: "example", autoDetect: true }, security: { mode: "normal", developerTestMode: false }, ui: {}, tools: { readFile: { enabled: true }, glob: { enabled: true }, grep: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } }, bash: { autoDetect: true }, policies: { toolDefaults: { readFile: "ask", glob: "ask", grep: "ask", write: "ask", edit: "ask", bash: "ask" }, userRules: [] }, desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false }, history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 }, search: { globTimeoutSeconds: 30, grepTimeoutSeconds: 30 } };
  return { client: new TauriHarnessControlClient(), components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
    runtime: { status: "success", data: { runtimeId: "runtime-1", activeWorkspace: "D:\private\SECRET_PATH" } },
    config, tools: { status: "empty", data: { items: [] } }, mutateConfig: vi.fn(),
    overall: "degraded", restartRequired: false, harness: null as import("@/services/harness/client").HarnessStatus | null };
}
const sensitive = {
  apiKey: "API_KEY_SENTINEL", secret: "SECRET_SENTINEL", token: "TOKEN_SENTINEL",
  arguments: { path: "D:\private\SECRET_PATH" }, result: "TOOL_RESULT_SENTINEL",
  metadata: { rawArguments: "RAW_ARGUMENTS_SENTINEL" }, reason: "SECRET_REASON_SENTINEL",
};
function diagnostics() {
  return { schemaVersion: 1, capturedAt: "2026-09-08T01:02:03.000Z",
    runtime: { runtimeId: "runtime-safe", startedAt: null, lifecycle: "ready", readiness: "degraded", initializedModules: [], ...sensitive },
    security: { workspaceValidation: "validated_at_initialization", policyEngineLoaded: false, auditEnabled: true, executionPipeline: "ready", ...sensitive },
    tools: { declaredCount: 9, effectiveCount: 4, items: [] },
    requests: { observation: "not_instrumented", activeCount: null },
    lastError: { observation: "not_recorded", code: null, message: "SECRET_ERROR_SENTINEL" },
    tunnel: { configured: null, processState: "not_observed", connectionState: "not_established", blockedReason: "m7_c_blocked" }, ...sensitive };
}
function entry(toolName: string | null, timestamp: string | null = "2026-09-08T01:02:03.000Z") {
  return { eventId: "event-safe", requestId: "request-safe", timestamp, category: "policy", eventType: "policy_evaluated",
    lifecycleState: "policy_evaluating", toolName, decisionSummary: null, outcomeSummary: "in_progress", ...sensitive };
}
const tool = { name: "read_file", declaredCapability: "workspace_read", availability: "unavailable_by_policy", decision: "ask", ...sensitive };
function noSensitiveDOM(container: HTMLElement) {
  for (const text of ["API_KEY_SENTINEL", "SECRET_SENTINEL", "TOKEN_SENTINEL", "SECRET_PATH", "TOOL_RESULT_SENTINEL", "RAW_ARGUMENTS_SENTINEL", "SECRET_REASON_SENTINEL", "SECRET_ERROR_SENTINEL"]) {
    expect(container.innerHTML).not.toContain(text);
  }
}
beforeEach(() => { tauri.invoke.mockReset(); context = makeContext(); });
afterEach(cleanup);

describe("M8-D Control Plane response rendering", () => {
  it.each(["en", "zh-CN"] as const)("shows recovery cooldown and blocked details without readiness evidence in %s", async (language) => {
    await i18n.changeLanguage(language);
    tauri.invoke.mockResolvedValue(diagnostics());
    context.harness = { lifecycle: "error", runtime: "ready", tunnel: "error", mcp: "ready", tunnelReadiness: null,
      tunnelRecovery: { phase: "cooldown", reason: { code: "TUNNEL_RECOVERY_COOLDOWN", message: "Retry budget cooling down." },
        cooldownUntil: "2026-10-23T09:10:00Z", nextAttemptAt: "2026-10-23T09:10:00Z" } };
    const { rerender } = render(<OverviewPage />);
    expect(await screen.findByText(i18n.t("overview.tunnelRecovery") + ": " + i18n.t("overview.recoveryPhase.cooldown"))).toBeInTheDocument();
    expect(screen.getByText(/TUNNEL_RECOVERY_COOLDOWN/)).toBeInTheDocument();
    expect(screen.getAllByText("2026-10-23T09:10:00Z")).toHaveLength(2);
    context.harness.tunnelRecovery = { phase: "blocked", reason: { code: "CONTROL_PLANE_UNAUTHORIZED", message: "401 Unauthorized" }, cooldownUntil: null, nextAttemptAt: null };
    rerender(<OverviewPage />);
    expect(screen.getByText(/CONTROL_PLANE_UNAUTHORIZED: 401 Unauthorized/)).toBeInTheDocument();
    expect(screen.queryByText("2026-10-23T09:10:00Z")).not.toBeInTheDocument();
  });

  it.each(["en", "zh-CN"] as const)("renders Overview status and observation boundaries in %s", async (language) => {
    await i18n.changeLanguage(language);
    tauri.invoke.mockResolvedValue(diagnostics());
    render(<OverviewPage />);
    expect(await screen.findByText(language === "en" ? "Degraded" : "降级")).toBeInTheDocument();
    expect(screen.getByText(language === "en" ? "Engine not loaded" : "引擎未加载")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: language === "en" ? "Runtime Overview" : "Runtime 概览" })).toBeInTheDocument();
    expect(screen.getByText(language === "en" ? "External MCP readiness: NOT ESTABLISHED" : "外部 MCP 就绪状态：尚未证实")).toBeInTheDocument();
    expect(screen.getByText("2026-09-08T01:02:03.000Z")).toHaveAttribute("datetime", "2026-09-08T01:02:03.000Z");
  });

  it("switches Activity labels without changing raw tool, snapshot or read count", async () => {
    const entries = [{ ...entry("read_file"), eventType: "execution_completed", decisionSummary: "allow", outcomeSummary: "success", failureCategory: "bash_timeout" }];
    const original = JSON.stringify(entries);
    tauri.invoke.mockResolvedValue(entries);
    render(<ActivityPage />);
    await screen.findByText("Execution completed");
    const calls = tauri.invoke.mock.calls.length;
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getAllByText("read_file").length).toBeGreaterThan(0);
    expect(screen.getByText("执行已完成")).toBeInTheDocument();
    expect(screen.getAllByText("成功").length).toBeGreaterThan(0);
    expect(screen.queryByText("Execution completed")).not.toBeInTheDocument();
    expect(screen.queryByText("Succeeded")).not.toBeInTheDocument();
    fireEvent.click(within(screen.getByTestId("activity-row-event:event-safe")).getByRole("button"));
    const detail = screen.getByRole("dialog", { name: "活动详情" });
    expect(within(detail).getByText("允许")).toBeInTheDocument();
    expect(tauri.invoke.mock.calls).toHaveLength(calls);
    expect(JSON.stringify(entries)).toBe(original);
  });

  it("renders diagnostics authority, not config or calculated tool counts", async () => {
    tauri.invoke.mockResolvedValue(diagnostics());
    const { container } = render(<OverviewPage />);
    expect(await screen.findByText("4 directly available / 9 declared")).toBeInTheDocument();
    expect(screen.getByText("Validated at initialization")).toBeInTheDocument();
    expect(screen.getByText("Engine not loaded")).toBeInTheDocument();
    expect(screen.getByText("Degraded")).toBeInTheDocument();
    expect(screen.getByText(/External MCP readiness: NOT ESTABLISHED/)).toBeInTheDocument();
    expect(tauri.invoke.mock.calls).toEqual([["runtime_get_diagnostics", undefined]]);
    noSensitiveDOM(container);
  });

  it("renders every availability verbatim, keeps ASK, and never invents reason", async () => {
    const names = ["read_file", "glob", "grep", "write", "edit"];
    const tools = ["available", "unavailable_by_policy", "available_with_approval", "unavailable_by_runtime", "disabled"].map((availability, index) => ({ ...tool, name: names[index], availability }));
    tauri.invoke.mockResolvedValue(tools);
    const { container } = render(<ToolsPage />);
    await waitFor(() => expect(screen.getByText("Available")).toBeInTheDocument());
    tools.forEach((item) => expect(screen.getByText(i18n.t(toolAvailabilityKey(item.availability as ToolAvailability)))).toBeInTheDocument());
    expect(screen.getAllByRole("radiogroup")).toHaveLength(6);
    expect(screen.getByText("Runtime has not reported this tool.")).toBeInTheDocument();
    expect(tauri.invoke.mock.calls).toEqual([["runtime_get_effective_tools", undefined]]);
    noSensitiveDOM(container);
  });

  it("reverses reception order without timestamp sorting, mutation or implicit ALLOW", async () => {
    const entries = [entry("first", "2026-09-08T23:00:00.000Z"), entry("second", "2026-09-08T01:00:00.000Z"), entry("third", "2026-09-08T01:00:00.000Z"), entry(null, null)];
    const original = JSON.stringify(entries);
    tauri.invoke.mockResolvedValue(entries);
    const { container } = render(<ActivityPage />);
    const rows = await screen.findAllByTestId(/^activity-row-snapshot:/);
    // Feed order is newest received first; equal/older Runtime timestamps do
    // not reorder the bounded snapshot, and missing IDs remain local keys.
    expect(rows.map((row) => row.getAttribute("data-testid"))).toEqual([
      "activity-row-snapshot:1:3",
      "activity-row-snapshot:1:2",
      "activity-row-snapshot:1:1",
      "activity-row-snapshot:1:0",
    ]);
    expect(within(rows[0]).getAllByText("Not recorded").length).toBeGreaterThan(0);
    expect(within(rows[1]).getByText("third")).toBeInTheDocument();
    expect(within(rows[2]).getByText("second")).toBeInTheDocument();
    expect(within(rows[3]).getByText("first")).toBeInTheDocument();
    rows.forEach((row) => expect(within(row).getByText("Not recorded")).toBeInTheDocument());
    expect(screen.queryAllByText("Not evaluated")).toHaveLength(0);
    expect(screen.queryByText("ALLOW")).not.toBeInTheDocument();
    expect(JSON.stringify(entries)).toBe(original);
    expect(tauri.invoke.mock.calls).toEqual([["runtime_get_activity_snapshot", { limit: 100 }]]);
    noSensitiveDOM(container);
  });

  it("renders activity decisions and outcomes without deriving them", async () => {
    tauri.invoke.mockResolvedValue([
      { ...entry("read_file"), eventType: "execution_completed", decisionSummary: "allow", outcomeSummary: "success" },
      { ...entry("glob"), eventType: "request_denied", decisionSummary: "deny", outcomeSummary: "denied" },
      { ...entry("read_file"), eventType: "approval_required", decisionSummary: "ask", outcomeSummary: "approval_required" },
    ]);
    render(<ActivityPage />);
    expect((await screen.findAllByText("Success")).length).toBeGreaterThan(0);
    expect(screen.getByText("Policy denied")).toBeInTheDocument();
    expect(screen.getByText("Pending")).toBeInTheDocument();
    expect(screen.getByText("Approval required")).toBeInTheDocument();
    const decisionRows = [
      ["activity-row-snapshot:1:0", "ALLOW"],
      ["activity-row-snapshot:1:1", "DENY"],
      ["activity-row-snapshot:1:2", "ASK"],
    ] as const;
    for (const [testId, decision] of decisionRows) {
      fireEvent.click(within(screen.getByTestId(testId)).getByRole("button"));
      const detail = screen.getByRole("dialog", { name: "Activity details" });
      expect(within(detail).getAllByText(decision).length).toBeGreaterThan(0);
      fireEvent.click(within(detail).getByRole("button", { name: "Close" }));
    }
  });

  it.each([[ActivityPage, "No retained activity"], [ToolsPage, "No tools reported"]] as const)("distinguishes empty arrays from unavailable", async (Page, message) => {
    tauri.invoke.mockResolvedValue([]);
    render(<Page />);
    if (Page === ToolsPage) expect(await screen.findAllByText("Runtime has not reported this tool.")).toHaveLength(6);
    else expect(await screen.findByText("No retained activity in this snapshot")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText("ready")).not.toBeInTheDocument();
  });

  for (const Page of [RuntimeOverview, ActivityPage, ToolsPage]) {
    it.each(["RUNTIME_UNAVAILABLE", "RUNTIME_NOT_READY", "INVALID_REQUEST", "INTERNAL_ERROR", "SECRET_ERROR_SENTINEL"])(Page.name + " safely renders %s", async (code) => {
      tauri.invoke.mockRejectedValue({ code, message: JSON.stringify(sensitive) });
      const { container } = render(<Page />);
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent(code === "RUNTIME_UNAVAILABLE" ? "Runtime unavailable" : code === "RUNTIME_NOT_READY" ? "Runtime not ready" : code === "INVALID_REQUEST" ? "Invalid control read request" : "Control read failed");
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      expect(screen.queryByText("No tools reported")).not.toBeInTheDocument();
      expect(screen.queryByText("No retained activity")).not.toBeInTheDocument();
      noSensitiveDOM(container);
    });
  }

  it("keeps configured rows, clears old running facts on refresh and recovers from an unavailable Runtime", async () => {
    tauri.invoke.mockResolvedValueOnce([tool]).mockRejectedValueOnce({ code: "RUNTIME_UNAVAILABLE" }).mockResolvedValueOnce([]);
    render(<ToolsPage />);
    await screen.findByText("Unavailable by Runtime policy");
    fireEvent.click(screen.getByRole("button", { name: "Refresh tools" }));
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.queryByText("Unavailable by Runtime policy")).not.toBeInTheDocument();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "Refresh tools" }));
    expect(await screen.findAllByText("Runtime has not reported this tool.")).toHaveLength(6);
  });

  it("ignores late responses from the previous Runtime identity", async () => {
    let resolveOld!: (value: unknown) => void;
    tauri.invoke.mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; })).mockRejectedValueOnce({ code: "RUNTIME_NOT_READY" });
    const { rerender } = render(<RuntimeOverview />);
    await waitFor(() => expect(tauri.invoke).toHaveBeenCalledTimes(1));
    context = { ...context, runtime: { ...context.runtime, data: { ...context.runtime.data, runtimeId: "runtime-2" } } };
    rerender(<RuntimeOverview />);
    await screen.findByRole("alert");
    await act(async () => resolveOld(diagnostics()));
    expect(screen.getByRole("alert")).toHaveTextContent("Runtime not ready");
    expect(screen.queryByText("4 directly available / 9 declared")).not.toBeInTheDocument();
  });

  it("invalidates a successful snapshot on lifecycle change without deriving readiness", async () => {
    tauri.invoke.mockResolvedValueOnce(diagnostics()).mockRejectedValueOnce({ code: "RUNTIME_UNAVAILABLE" });
    const { rerender } = render(<RuntimeOverview />);
    await screen.findByText("4 directly available / 9 declared");
    context = { ...context, components: { ...context.components, runtime: "stopped" } };
    rerender(<RuntimeOverview />);
    expect(screen.queryByText("4 directly available / 9 declared")).not.toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent("Runtime unavailable");
  });

  it.each([undefined, null, {}, { ...diagnostics(), schemaVersion: 2 }, { ...diagnostics(), capturedAt: "SECRET_PATH" }])("rejects invalid diagnostics rather than displaying optimistic state", async (response) => {
    tauri.invoke.mockResolvedValue(response);
    const { container } = render(<RuntimeOverview />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Control read failed");
    noSensitiveDOM(container);
  });
});

it("M7 expands authoritative safe Activity detail and preserves raw identifiers", async () => {
  await i18n.changeLanguage("en");
  tauri.invoke.mockResolvedValue([{ ...entry("write"), eventType: "execution_completed", decisionSummary: "ask", outcomeSummary: "success", requestId: "request-Raw123", toolEnabled: true, capability: "workspace_write", matchedRuleIds: ["rule-Raw123"], policyReason: "matched_user_rules", approvalId: "approval-Raw123", approvalDecision: "allow_once", durationMs: 47, sanitizedArguments: { tool: "write", logicalPath: "safe.txt", content: "WRITE_CONTENT_SENTINEL" }, resultEvidence: { tool: "write", bytesWritten: 7, disposition: "created", content: "RESULT_SENTINEL" } }]);
  const { container } = render(<ActivityPage />);
  fireEvent.click(within(await screen.findByTestId("activity-row-event:event-safe")).getByRole("button"));
  const detail = await screen.findByRole("dialog", { name: "Activity details" });
  expect(within(detail).getByText("request-Raw123")).toBeInTheDocument();
  expect(within(detail).getByText("47")).toBeInTheDocument();
  expect(within(detail).getAllByText(/safe.txt/).length).toBeGreaterThan(0);
  expect(within(detail).getAllByText(/bytesWritten/).length).toBeGreaterThan(0);
  fireEvent.click(within(detail).getByText("Advanced evidence"));
  for (const id of ["rule-Raw123", "approval-Raw123"]) expect(within(detail).getByText(new RegExp(id))).toBeInTheDocument();
  expect(container.innerHTML).not.toContain("WRITE_CONTENT_SENTINEL"); expect(container.innerHTML).not.toContain("RESULT_SENTINEL");
});
it("M7 missing evidence is visibly unrecorded without invented arguments or results", async () => {
  await i18n.changeLanguage("en"); tauri.invoke.mockResolvedValue([entry("read_file")]);
  render(<ActivityPage />); fireEvent.click(within(await screen.findByTestId("activity-row-event:event-safe")).getByRole("button"));
  const detail = await screen.findByRole("dialog", { name: "Activity details" });
  expect(within(detail).getAllByText("Not recorded")).toHaveLength(2);
  expect(within(detail).queryByText(/bytesRead/)).not.toBeInTheDocument();
});
it("M7 Overview shows real counters and safe error code", async () => {
  await i18n.changeLanguage("en");
  tauri.invoke.mockResolvedValue({ ...diagnostics(), requests: { observation: "instrumented", activeRequests: 3, pendingApprovals: 2, runningExecutions: 1 }, lastError: { code: "BASH_TIMEOUT", message: "SECRET_ERROR_SENTINEL" } });
  const { container } = render(<RuntimeOverview />);
  await screen.findByText("Active requests"); expect(screen.getByText("3")).toBeInTheDocument(); expect(screen.getByText("2")).toBeInTheDocument(); expect(screen.getByText("1")).toBeInTheDocument(); expect(screen.getByText("BASH_TIMEOUT")).toBeInTheDocument(); noSensitiveDOM(container);
});
