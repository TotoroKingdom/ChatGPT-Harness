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

const bashDescriptor: ToolDescriptor = {
  name: "bash",
  description: "Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.",
  implemented: true,
  enabled: false,
  exposed: true,
  effectivePolicy: "ask",
  scope: "active_workspace",
  readiness: "unavailable",
  riskLevel: "high",
};

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
    bash: { autoDetect: true },
    policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
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
    expect(screen.getAllByText("not_running")).toHaveLength(5);
    expect(screen.getAllByText("No active Runtime generation.")).toHaveLength(5);
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
    expect(screen.getAllByText("NOT EVALUATED")).toHaveLength(2);
    expect(screen.getByText("DENY")).toBeInTheDocument();
    expect(screen.getByText("Unavailable by Runtime policy.")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "Refresh tools" })[0]);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Desired state for read_file" })).not.toBeChecked());
  });

  it("renders Bash mode and readiness from the Runtime observation", async () => {
    const observed: ToolDiagnostic[] = [
      ...effectiveCatalog(),
      {
        name: "bash",
        declaredCapability: "developer_shell",
        availability: "unavailable_by_mode",
        decision: "deny",
        reason: "Developer/Test mode is required.",
        readiness: "unavailable",
        readinessReason: "Start a Developer/Test generation with validated Git Bash.",
      },
    ];
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable: vi.fn() },
      config: { ...config(), bash: { autoDetect: true, executablePath: "D:\\Git\\bin\\bash.exe" } },
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
    });
    state.effective.mockResolvedValue(observed);
    render(<ToolsPage />);

    expect(await screen.findByText("Unavailable by mode")).toBeInTheDocument();
    expect(screen.getByText("Readiness: unavailable")).toBeInTheDocument();
    expect(screen.getByText("Developer/Test mode is required.")).toBeInTheDocument();
    expect(screen.getByText(/not a filesystem sandbox/)).toBeInTheDocument();
    expect(screen.getByText(/files outside that directory and the network/)).toBeInTheDocument();
    expect(screen.getAllByText("NOT EVALUATED")).toHaveLength(1);
  });

  it("saves Bash picker and Auto Detect changes as desired config only", async () => {
    const chooseBashExecutable = vi.fn().mockResolvedValue("D:\\Git\\bin\\bash.exe");
    const observed: ToolDiagnostic[] = [...effectiveCatalog(), {
      name: "bash", availability: "unavailable_by_runtime", decision: "deny", readiness: "unavailable",
      reason: "Git Bash readiness is unavailable.", readinessReason: "Restart Runtime to validate the executable.",
    }];
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable },
      config: config(),
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
      restartRequired: true,
    });
    state.effective.mockResolvedValue(observed);
    render(<ToolsPage />);
    await screen.findByText("Unavailable by Runtime");

    fireEvent.click(screen.getByRole("button", { name: "Choose bash.exe" }));
    await waitFor(() => expect(state.save).toHaveBeenCalledOnce());
    expect(state.save.mock.calls[0][0].bash).toEqual({ autoDetect: true, executablePath: "D:\\Git\\bin\\bash.exe" });
    expect(state.save.mock.calls[0][0].tools.bash.enabled).toBe(false);

    fireEvent.click(screen.getByRole("checkbox", { name: "Auto detect Git Bash" }));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(2));
    expect(state.save.mock.calls[1][0].bash).toEqual({ autoDetect: false, executablePath: "D:\\Git\\bin\\bash.exe" });
  });

  it("clears the manual override and rolls the desired draft back on save failure", async () => {
    const chooseBashExecutable = vi.fn();
    const current = { ...config(), bash: { autoDetect: false, executablePath: "D:\\Git\\bin\\bash.exe" } };
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable },
      config: current,
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
    });
    state.effective.mockResolvedValue([...effectiveCatalog(), { name: "bash", availability: "disabled", decision: "deny", readiness: "unavailable" }]);
    state.save.mockResolvedValue({ ok: false, error: { message: "Configuration write failed" } });
    render(<ToolsPage />);
    await screen.findByText("Disabled");
    fireEvent.click(screen.getByRole("button", { name: "Clear override" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration write failed");
    expect(screen.getByRole("textbox", { name: "Manual Git Bash executable" })).toHaveValue("D:\\Git\\bin\\bash.exe");
  });

  it("preserves an explicit Auto Detect preference while choosing and clearing a manual path", async () => {
    const chooseBashExecutable = vi.fn().mockResolvedValue("D:\\Git\\bin\\new-bash.exe");
    const current = { ...config(), bash: { autoDetect: false, executablePath: "D:\\Git\\bin\\old-bash.exe" } };
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable },
      config: current,
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
    });
    state.effective.mockResolvedValue([...effectiveCatalog(), { name: "bash", availability: "unavailable_by_runtime", decision: "deny", readiness: "unavailable" }]);
    state.save.mockResolvedValue({ ok: true });
    render(<ToolsPage />);
    await screen.findByText("Unavailable by Runtime");

    fireEvent.click(screen.getByRole("button", { name: "Choose bash.exe" }));
    await waitFor(() => expect(state.save).toHaveBeenCalledOnce());
    expect(state.save.mock.calls[0][0].bash).toEqual({ autoDetect: false, executablePath: "D:\\Git\\bin\\new-bash.exe" });

    fireEvent.click(screen.getByRole("button", { name: "Clear override" }));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(2));
    expect(state.save.mock.calls[1][0].bash).toEqual({ autoDetect: false, executablePath: undefined });
  });
});
