import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, RuntimeState, ToolDescriptor } from "@protocol/v1";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { i18n } from "@/i18n";
import { ToolsPage } from "./ToolsPage";

const state = vi.hoisted(() => ({
  context: undefined as any,
  effective: vi.fn(),
  mutate: vi.fn(),
  save: vi.fn(),
  chooseBash: vi.fn(),
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
  description: "Run a controlled Git Bash command from the active Workspace.",
  implemented: true,
  enabled: false,
  exposed: true,
  effectivePolicy: "ask",
  scope: "active_workspace",
  readiness: "unavailable",
  riskLevel: "high",
};

function config(): AppConfig {
  return {
    schemaVersion: 2,
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

const availableTools = (): ToolDiagnostic[] => descriptors.map((tool) => ({ name: tool.name, availability: "available", decision: "allow" }));

function makeContext(overrides: Record<string, unknown> = {}) {
  return {
    client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable: state.chooseBash },
    config: config(),
    tools: { status: "success", data: { items: descriptors } },
    components: { runtime: "ready", tunnel: "stopped", mcp: "ready" },
    runtime: { status: "success", data: runtime() },
    harness: { lifecycle: "ready", runtime: "ready", tunnel: "stopped", mcp: "ready" },
    restartRequired: false,
    mutateConfig: state.mutate,
    saveConfig: state.save,
    ...overrides,
  };
}

beforeEach(async () => {
  await i18n.changeLanguage("en");
  state.effective.mockReset();
  state.mutate.mockReset();
  state.save.mockReset();
  state.chooseBash.mockReset();
  state.effective.mockResolvedValue(availableTools());
  state.mutate.mockResolvedValue({ ok: true });
  state.save.mockResolvedValue({ ok: true });
  state.chooseBash.mockResolvedValue(null);
  state.context = makeContext();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ToolsPage M8 compact rows and Inspector", () => {
  it("keeps saved rows editable while Runtime is stopped without reading effective tools", async () => {
    state.context = makeContext({
      components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
      harness: { lifecycle: "stopped", runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
      runtime: { status: "idle" },
      tools: { status: "disconnected" },
    });
    render(<ToolsPage />);

    expect(screen.getAllByText("not_running").length).toBeGreaterThan(0);
    const toggle = screen.getByRole("switch", { name: "Saved Enabled state for write" });
    fireEvent.click(toggle);
    await waitFor(() => expect(state.mutate).toHaveBeenCalledOnce());
    const intent = state.mutate.mock.calls[0][0] as (value: AppConfig) => AppConfig;
    expect(intent(config()).tools.write.enabled).toBe(false);
    expect(state.effective).not.toHaveBeenCalled();
  });

  it("uses local mutation intent for Enabled and default policy auto-save", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ mutateConfig });
    render(<ToolsPage />);

    fireEvent.click(screen.getByRole("switch", { name: "Saved Enabled state for write" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const enabledIntent = mutateConfig.mock.calls[0][0] as (value: AppConfig) => AppConfig;
    expect(enabledIntent(config()).tools.write.enabled).toBe(false);

    const writeDefault = screen.getAllByRole("radiogroup", { name: "Saved default policy for write" })[0];
    fireEvent.click(within(writeDefault).getByRole("radio", { name: "DENY" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledTimes(2));
    const policyIntent = mutateConfig.mock.calls[1][0] as (value: AppConfig) => AppConfig;
    expect(policyIntent(config()).policies.toolDefaults.write).toBe("deny");
  });

  it("does not open Inspector when a row control is used, and retains a details surface for identity", async () => {
    render(<ToolsPage />);
    fireEvent.click(screen.getByRole("switch", { name: "Saved Enabled state for write" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /write Write a workspace file/ }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(within(screen.getByRole("dialog")).getByText("Saved configuration")).toBeInTheDocument();
    expect(within(screen.getByRole("dialog")).getByText("Advanced")).toBeInTheDocument();
  });

  it("keeps rows editable when the Runtime observation is checking or unreadable", async () => {
    state.context = makeContext({
      components: { runtime: "error", tunnel: "stopped", mcp: "error" },
      harness: { lifecycle: "error", runtime: "error", tunnel: "stopped", mcp: "error" },
      runtime: { status: "error", error: { code: "RUNTIME_UNAVAILABLE" } },
      tools: { status: "error", error: { code: "RUNTIME_UNAVAILABLE" } },
    });
    render(<ToolsPage />);

    expect(screen.getByText("Current Runtime observation could not be read. Saved configuration remains editable.")).toBeInTheDocument();
    expect(screen.getAllByText("Unknown").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("switch", { name: "Saved Enabled state for write" }));
    await waitFor(() => expect(state.mutate).toHaveBeenCalledOnce());
  });

  it("shows exact availability and policy differences without turning mode unavailability into Denied", async () => {
    state.effective.mockResolvedValue([
      { name: "read_file", availability: "available_with_approval", decision: "ask" },
      { name: "glob", availability: "unavailable_by_mode", decision: "deny" },
      { name: "write", availability: "unavailable_by_policy", decision: "deny" },
      { name: "edit", availability: "disabled", decision: "deny" },
    ]);
    render(<ToolsPage />);

    expect(await screen.findByText("Available with approval")).toBeInTheDocument();
    expect(screen.getByText("Unavailable by mode")).toBeInTheDocument();
    expect(screen.getByText("Unavailable by Runtime policy")).toBeInTheDocument();
    expect(screen.getByText("Disabled")).toBeInTheDocument();
  });

  it("keeps a successful save visible when follow-up refresh is unconfirmed", async () => {
    state.mutate.mockResolvedValue({ ok: true, refreshUnconfirmed: true });
    state.context = makeContext({ components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" }, runtime: { status: "idle" }, tools: { status: "disconnected" } });
    render(<ToolsPage />);
    fireEvent.click(screen.getByRole("switch", { name: "Saved Enabled state for write" }));
    expect(await screen.findAllByText("Saved; status refresh is unconfirmed.")).not.toHaveLength(0);
    expect(screen.getAllByRole("switch", { name: "Saved Enabled state for write" })[0]).toHaveAttribute("aria-checked", "true");
  });

  it("keeps Bash shell controls in Inspector, does not write on picker cancel, and never claims validation", async () => {
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable: state.chooseBash },
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
      config: { ...config(), bash: { autoDetect: false, executablePath: "D:\\Git\\bin\\bash.exe" } },
    });
    state.effective.mockResolvedValue([...availableTools(), { name: "bash", availability: "unavailable_by_runtime", decision: "deny", readiness: "unavailable" }]);
    render(<ToolsPage />);

    fireEvent.click(screen.getByRole("button", { name: /bash Run a controlled/ }));
    expect(await screen.findByText("Bash configuration")).toBeInTheDocument();
    expect(screen.getByText(/Saved here does not mean Runtime validated it/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Choose bash.exe" }));
    await waitFor(() => expect(state.chooseBash).toHaveBeenCalledOnce());
    expect(state.mutate).not.toHaveBeenCalled();

    state.chooseBash.mockResolvedValue("D:\\Git\\bin\\new-bash.exe");
    fireEvent.click(screen.getByRole("button", { name: "Choose bash.exe" }));
    await waitFor(() => expect(state.mutate).toHaveBeenCalledOnce());
    const bashIntent = state.mutate.mock.calls[0][0] as (value: AppConfig) => AppConfig;
    expect(bashIntent((state.context as { config: AppConfig }).config).bash).toEqual({ autoDetect: false, executablePath: "D:\\Git\\bin\\new-bash.exe" });
  });

  it("restores the Bash override on save failure and preserves the non-sensitive draft", async () => {
    const current = { ...config(), bash: { autoDetect: false, executablePath: "D:\\Git\\bin\\bash.exe" } };
    state.context = makeContext({
      client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable: state.chooseBash },
      config: current,
      tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
    });
    state.effective.mockResolvedValue([...availableTools(), { name: "bash", availability: "disabled", decision: "deny", readiness: "unavailable" }]);
    state.mutate.mockResolvedValue({ ok: false, error: { code: "CONFIG_WRITE_FAILED", message: "Configuration write failed" } });
    state.chooseBash.mockResolvedValue("D:\\Git\\bin\\new-bash.exe");
    render(<ToolsPage />);

    fireEvent.click(screen.getByRole("button", { name: /bash Run a controlled/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Choose bash.exe" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration could not be saved.");
    expect(screen.getByRole("textbox", { name: "Manual Git Bash executable" })).toHaveValue("D:\\Git\\bin\\bash.exe");
  });
});

it("switches T4 presentation labels live without changing tool identity", async () => {
  state.context = makeContext({
    client: { runtimeGetEffectiveTools: state.effective, chooseBashExecutable: state.chooseBash },
    tools: { status: "success", data: { items: [...descriptors, bashDescriptor] } },
  });
  state.effective.mockResolvedValue([...availableTools(), { name: "bash", availability: "unavailable_by_mode", decision: "deny", readiness: "unavailable" }]);
  render(<ToolsPage />);
  expect(screen.getAllByRole("heading", { name: "Tools" }).length).toBeGreaterThan(0);
  expect(screen.getByText("read_file")).toBeInTheDocument();
  await screen.findByText("Unavailable by mode");
  await i18n.changeLanguage("zh-CN");
  expect(screen.getByRole("heading", { name: "工具" })).toBeInTheDocument();
  expect(screen.getByText("read_file")).toBeInTheDocument();
  await screen.findByText("当前模式不可用");
});
