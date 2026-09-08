import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TauriHarnessControlClient } from "@/services/harness/local-client";
import { RuntimeOverview } from "./overview/RuntimeOverview";
import { OverviewPage } from "./overview/OverviewPage";
import { ActivityPage } from "./activity/ActivityPage";
import { ToolsPage } from "./tools/ToolsPage";

const tauri = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => context }));
let context = makeContext();
function makeContext() {
  return { client: new TauriHarnessControlClient(), components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
    runtime: { status: "success", data: { runtimeId: "runtime-1", activeWorkspace: "D:\private\SECRET_PATH" } },
    overall: "degraded", restartRequired: false };
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
  it("renders diagnostics authority, not config or calculated tool counts", async () => {
    tauri.invoke.mockResolvedValue(diagnostics());
    const { container } = render(<OverviewPage />);
    expect(await screen.findByText("4 directly available / 9 declared")).toBeInTheDocument();
    expect(screen.getByText("Validated at initialization")).toBeInTheDocument();
    expect(screen.getByText("Engine not loaded")).toBeInTheDocument();
    expect(screen.getByText("degraded")).toBeInTheDocument();
    expect(screen.getByText(/M7-C: BLOCKED/)).toBeInTheDocument();
    expect(tauri.invoke.mock.calls).toEqual([["runtime_get_diagnostics", undefined]]);
    noSensitiveDOM(container);
  });

  it("renders every availability verbatim, keeps ASK, and never invents reason", async () => {
    const tools = ["available", "unavailable_by_policy", "unavailable_by_runtime", "disabled"].map((availability, index) => ({ ...tool, name: "registry_tool_" + index, availability }));
    tauri.invoke.mockResolvedValue(tools);
    const { container } = render(<ToolsPage />);
    expect(await screen.findByText("registry_tool_0")).toBeInTheDocument();
    tools.forEach((item) => expect(screen.getByText(item.availability)).toBeInTheDocument());
    expect(screen.getAllByText("ASK")).toHaveLength(4);
    expect(screen.getAllByText("Not provided by Control Plane")).toHaveLength(4);
    expect(tauri.invoke.mock.calls).toEqual([["runtime_get_effective_tools", undefined]]);
    noSensitiveDOM(container);
  });

  it("reverses reception order without timestamp sorting, mutation or implicit ALLOW", async () => {
    const entries = [entry("first", "2026-09-08T23:00:00.000Z"), entry("second", "2026-09-08T01:00:00.000Z"), entry("third", "2026-09-08T01:00:00.000Z"), entry(null, null)];
    const original = JSON.stringify(entries);
    tauri.invoke.mockResolvedValue(entries);
    const { container } = render(<ActivityPage />);
    await screen.findByRole("table");
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]).getAllByText("Not recorded")).toHaveLength(2);
    expect(within(rows[1]).getByText("third")).toBeInTheDocument();
    expect(within(rows[2]).getByText("second")).toBeInTheDocument();
    expect(within(rows[3]).getByText("first")).toBeInTheDocument();
    expect(screen.getAllByText("Not evaluated")).toHaveLength(4);
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
    expect(await screen.findByText("ALLOW")).toBeInTheDocument();
    expect(screen.getByText("DENY")).toBeInTheDocument();
    expect(screen.getByText("ASK")).toBeInTheDocument();
    expect(screen.getByText("success")).toBeInTheDocument();
    expect(screen.getByText("denied")).toBeInTheDocument();
    expect(screen.getAllByText("approval_required")).toHaveLength(2);
  });

  it.each([[ActivityPage, "No retained activity"], [ToolsPage, "No tools reported"]] as const)("distinguishes empty arrays from unavailable", async (Page, message) => {
    tauri.invoke.mockResolvedValue([]);
    render(<Page />);
    expect(await screen.findByText(message)).toBeInTheDocument();
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

  it("clears old data on refresh and recovers from an unavailable Runtime", async () => {
    tauri.invoke.mockResolvedValueOnce([tool]).mockRejectedValueOnce({ code: "RUNTIME_UNAVAILABLE" }).mockResolvedValueOnce([]);
    render(<ToolsPage />);
    await screen.findByText("read_file");
    fireEvent.click(screen.getByRole("button", { name: "Refresh tools" }));
    expect(screen.queryByText("read_file")).not.toBeInTheDocument();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "Refresh tools" }));
    expect(await screen.findByText("No tools reported")).toBeInTheDocument();
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
