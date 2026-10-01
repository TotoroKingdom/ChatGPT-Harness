import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, RuntimeState, ToolDescriptor } from "@protocol/v1";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { ToolsPage } from "./ToolsPage";

const state = vi.hoisted(() => ({
  context: undefined as any,
  effective: vi.fn(),
  save: vi.fn(),
}));

vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => state.context,
}));

const descriptors: ToolDescriptor[] = [
  { name: "read_file", description: "Read a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
  { name: "glob", description: "Find workspace paths.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "low" },
  { name: "write", description: "Write a workspace file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
  { name: "edit", description: "Edit a workspace text file.", implemented: true, enabled: true, exposed: true, effectivePolicy: "allow", scope: "workspace", readiness: "ready", riskLevel: "medium" },
];

const effectiveCatalog = (): ToolDiagnostic[] => descriptors.map((tool) => ({
  name: tool.name,
  availability: "available",
  decision: "allow",
}));

function config(): AppConfig {
  return {
    schemaVersion: 1,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
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
  };
}

function runtime(runtimeId = "runtime-1"): RuntimeState {
  return {
    runtimeId,
    startedAt: "2026-09-30T00:00:00Z",
    lifecycle: "ready",
    readiness: "ready",
    configRevision: "7",
    activeWorkspace: "D:\\workspace",
    securityMode: "normal",
    exposedToolCatalog: descriptors,
    activeRequests: 0,
    pendingApprovals: 0,
    runningExecutions: 0,
  };
}

function makeContext(overrides: Record<string, unknown> = {}) {
  return {
    client: { runtimeGetEffectiveTools: state.effective },
    config: config(),
    tools: { status: "success", data: { items: descriptors } },
    components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
    runtime: { status: "success", data: runtime() },
    restartRequired: false,
    saveConfig: state.save,
    ...overrides,
  };
}

beforeEach(() => {
  state.effective.mockReset();
  state.save.mockReset();
  state.effective.mockResolvedValue(effectiveCatalog());
  state.save.mockResolvedValue({ ok: true });
  state.context = makeContext();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ToolsPage M2 desired/effective controls", () => {
  it("allows a stopped Harness to save desired state without an active effective state", async () => {
    state.context = makeContext({
      components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
      runtime: { status: "idle" },
      tools: { status: "disconnected" },
    });
    render(<ToolsPage />);

    const toggle = screen.getByRole("checkbox", { name: "Desired state for write" });
    fireEvent.click(toggle);

    await waitFor(() => expect(state.save).toHaveBeenCalledOnce());
    expect(state.save.mock.calls[0][0].tools.write.enabled).toBe(false);
    expect(screen.getAllByText("not_running")).toHaveLength(4);
    expect(screen.getAllByText("No active Runtime generation.")).toHaveLength(4);
    expect(state.effective).not.toHaveBeenCalled();
  });

  it("keeps current effective state unchanged while a running save creates a restart mismatch", async () => {
    state.save.mockImplementation(async (next: AppConfig) => {
      state.context = makeContext({ config: next, restartRequired: true });
      return { ok: true };
    });
    render(<ToolsPage />);
    await screen.findAllByText("available");

    fireEvent.click(screen.getByRole("checkbox", { name: "Desired state for write" }));

    await waitFor(() => expect(screen.getByText("Restart Required")).toBeInTheDocument());
    expect(screen.getByRole("checkbox", { name: "Desired state for write" })).not.toBeChecked();
    expect(screen.getAllByText("available").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Available in the current Runtime generation.").length).toBeGreaterThan(0);
  });

  it("converges effective state after a new Runtime generation", async () => {
    const disabled = effectiveCatalog().map((tool) => tool.name === "write"
      ? { ...tool, availability: "disabled" as const, decision: "deny" as const }
      : tool);
    state.effective.mockResolvedValueOnce(effectiveCatalog()).mockResolvedValueOnce(disabled);
    state.context = makeContext({ restartRequired: true });
    const { rerender } = render(<ToolsPage />);
    await screen.findAllByText("available");

    state.context = makeContext({
      config: { ...config(), tools: { ...config().tools, write: { enabled: false } } },
      runtime: { status: "success", data: runtime("runtime-2") },
      restartRequired: false,
    });
    rerender(<ToolsPage />);

    await waitFor(() => expect(screen.getByText("disabled")).toBeInTheDocument());
    expect(screen.queryByText("Restart Required")).not.toBeInTheDocument();
  });

  it("shows save errors without leaving a false desired state", async () => {
    state.context = makeContext({
      components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
      runtime: { status: "idle" },
      tools: { status: "disconnected" },
    });
    state.save.mockResolvedValue({ ok: false, error: { message: "Configuration write failed" } });
    render(<ToolsPage />);

    const toggle = screen.getByRole("checkbox", { name: "Desired state for read_file" });
    fireEvent.click(toggle);

    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration write failed");
    expect(toggle).toBeChecked();
  });

  it("renders disabled separately from policy denial and refresh does not overwrite desired state", async () => {
    const observed: ToolDiagnostic[] = [
      { name: "read_file", availability: "disabled", decision: "deny" },
      { name: "glob", availability: "unavailable_by_policy", decision: "deny" },
      { name: "write", availability: "available", decision: "allow" },
      { name: "edit", availability: "available", decision: "allow" },
    ];
    state.context = makeContext({
      config: { ...config(), tools: { ...config().tools, readFile: { enabled: false } } },
      restartRequired: false,
    });
    state.effective.mockResolvedValue(observed);
    render(<ToolsPage />);

    await screen.findByText("Disabled by saved Runtime configuration.");
    expect(screen.getByText("NOT EVALUATED")).toBeInTheDocument();
    expect(screen.getByText("DENY")).toBeInTheDocument();
    expect(screen.getByText("Unavailable by Runtime policy.")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "Refresh tools" })[0]);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Desired state for read_file" })).not.toBeChecked());
  });
});
