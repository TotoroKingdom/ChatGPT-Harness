import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { SettingsPage } from "./SettingsPage";

const state = vi.hoisted(() => ({ context: undefined as any }));

vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => state.context,
}));

function config(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    schemaVersion: 2,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
    bash: { autoDetect: true },
    policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
    ...overrides,
  };
}

function makeContext(overrides: Record<string, unknown> = {}) {
  const saved = config();
  return {
    config: saved,
    configMetadata: { configDirectory: "D:\\config", configFile: "D:\\config\\config.json" },
    configError: null,
    eligibility: { status: "ready_to_start", canStart: true, message: "ready" },
    apiKeyConfigured: false,
    apiKeyObserved: true,
    restartRequired: false,
    components: { runtime: "stopped", tunnel: "stopped", mcp: "stopped" },
    harness: { lifecycle: "stopped", runtime: "stopped", tunnel: "stopped", mcp: "stopped", tunnelReadiness: null },
    runtime: { status: "success", data: { securityMode: "normal" } },
    chooseWorkspace: vi.fn().mockResolvedValue("D:\\new-workspace"),
    chooseTunnelClient: vi.fn().mockResolvedValue("D:\\tunnel-client"),
    detectTunnelClient: vi.fn().mockResolvedValue(null),
    deleteApiKey: vi.fn().mockResolvedValue(undefined),
    refreshConfig: vi.fn().mockResolvedValue(undefined),
    mutateConfig: vi.fn().mockResolvedValue({ ok: true }),
    setUiLanguage: vi.fn().mockResolvedValue({ ok: true }),
    resetConfig: vi.fn().mockResolvedValue({ ok: true }),
    restartHarness: vi.fn().mockResolvedValue({ ok: true }),
    runDiagnostics: vi.fn().mockResolvedValue({ checks: [] }),
    ...overrides,
  };
}

function openConnectionEditor() {
  fireEvent.click(document.querySelector("#settings-section-connection button")!);
}

function applyConnectionDraft() {
  fireEvent.click(screen.getByRole("dialog").querySelector("button.button-primary")!);
}

beforeEach(async () => {
  await initializeLanguage("en");
  document.documentElement.lang = "en";
  state.context = makeContext();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("M8 Settings section/list workflow", () => {
  it("renders the six settings sections without a page-level Save action", () => {
    render(<SettingsPage />);
    ["general", "workspace", "connection", "security", "advanced", "danger"].forEach((name) => {
      expect(document.getElementById(`settings-section-${name}`)).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "Save Changes" })).not.toBeInTheDocument();
  });

  it("applies Language immediately and rolls back locale on persistence failure", async () => {
    state.context = makeContext({ setUiLanguage: vi.fn().mockResolvedValue({ ok: false, error: { code: "CONFIG_WRITE_FAILED" } }) });
    render(<SettingsPage />);
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await waitFor(() => expect(i18n.language).toBe("en"));
    expect(document.documentElement.lang).toBe("en");
    expect(screen.getByRole("combobox", { name: "Language" })).toHaveValue("");
    expect(state.context.restartHarness).not.toHaveBeenCalled();
  });

  it("saves a picked Workspace through a scoped mutation only", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ mutateConfig });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Choose Folder" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const intent = mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig;
    const next = intent(state.context.config);
    expect(next.workspace.rootPath).toBe("D:\\new-workspace");
    expect(next.tunnel).toEqual(state.context.config.tunnel);
  });

  it("keeps Connection edits in a draft until Apply and scopes the saved tunnel section", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ mutateConfig });
    render(<SettingsPage />);
    openConnectionEditor();
    const tunnelId = screen.getByRole("textbox", { name: "Tunnel ID (required)" });
    fireEvent.change(tunnelId, { target: { value: "draft-tunnel" } });
    expect(mutateConfig).not.toHaveBeenCalled();
    applyConnectionDraft();
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const intent = mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig;
    const next = intent(state.context.config);
    expect(next.tunnel.tunnelId).toBe("draft-tunnel");
    expect(next.workspace).toEqual(state.context.config.workspace);
  });

  it("retains the draft and reports a conflict when the observed tunnel changed", async () => {
    const mutateConfig = vi.fn().mockImplementation(async (intent: (latest: AppConfig) => AppConfig) => {
      try {
        intent({ ...state.context.config, tunnel: { ...state.context.config.tunnel, tunnelId: "external" } });
        return { ok: true };
      } catch (error) {
        return { ok: false, error };
      }
    });
    state.context = makeContext({ mutateConfig });
    render(<SettingsPage />);
    openConnectionEditor();
    const tunnelId = screen.getByRole("textbox", { name: "Tunnel ID (required)" });
    fireEvent.change(tunnelId, { target: { value: "draft-tunnel" } });
    applyConnectionDraft();
    await screen.findAllByText("CONFIG_CONFLICT");
    expect(screen.getByRole("textbox", { name: "Tunnel ID (required)" })).toHaveValue("draft-tunnel");
  });

  it("blocks Apply immediately when the latest observed tunnel changes and supports Reload", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ mutateConfig });
    const view = render(<SettingsPage />);
    openConnectionEditor();
    fireEvent.change(screen.getByRole("textbox", { name: "Tunnel ID (required)" }), { target: { value: "draft-tunnel" } });
    state.context.config = config({ tunnel: { tunnelId: "external", autoDetect: true } });
    view.rerender(<SettingsPage />);
    expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
    expect(screen.getByRole("dialog").querySelector("button.button-primary")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Reload" }));
    expect(screen.getByRole("textbox", { name: "Tunnel ID (required)" })).toHaveValue("external");
    expect(mutateConfig).not.toHaveBeenCalled();
  });

  it("requires a separate confirmation for API Key removal and never routes it through Apply", async () => {
    const deleteApiKey = vi.fn().mockResolvedValue(undefined);
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ apiKeyConfigured: true, deleteApiKey, mutateConfig });
    render(<SettingsPage />);
    openConnectionEditor();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(deleteApiKey).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(deleteApiKey).not.toHaveBeenCalled();
    openConnectionEditor();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" }).at(-1)!);
    await waitFor(() => expect(deleteApiKey).toHaveBeenCalledOnce());
    expect(mutateConfig).not.toHaveBeenCalled();
  });

  it("clears a failed replacement and requires a new key before retry", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: false, error: { code: "CONFIG_WRITE_FAILED" } });
    state.context = makeContext({ apiKeyConfigured: true, mutateConfig });
    render(<SettingsPage />);
    openConnectionEditor();
    fireEvent.click(screen.getByRole("button", { name: "Replace" }));
    fireEvent.change(screen.getByLabelText("API Key (required)"), { target: { value: "secret-once" } });
    applyConnectionDraft();
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("API Key (required)")).toHaveValue("");
    expect(screen.getByText("Enter an API Key for the connection.")).toBeInTheDocument();
    expect(screen.getByRole("dialog").querySelector("button.button-primary")).toBeDisabled();
  });

  it("confirms Developer/Test and keeps saved/effective mode separate", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ mutateConfig });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("radio", { name: "Developer/Test Mode" }));
    expect(mutateConfig).not.toHaveBeenCalled();
    fireEvent.click(screen.getAllByRole("button", { name: "Developer/Test Mode" }).at(-1)!);
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    expect(screen.getByText(/Saved configuration/)).toBeInTheDocument();
    expect(screen.getAllByText(/Current Runtime/).length).toBeGreaterThan(0);
  });

  it("keeps Reset behind an independent danger confirmation", async () => {
    const resetConfig = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ resetConfig });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Reset Local Config" }));
    expect(resetConfig).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(resetConfig).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Reset Local Config" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Reset Local Config" }).at(-1)!);
    await waitFor(() => expect(resetConfig).toHaveBeenCalledOnce());
  });
});
