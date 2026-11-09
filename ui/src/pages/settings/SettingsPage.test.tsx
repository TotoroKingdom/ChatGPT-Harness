import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { SettingsPage } from "./SettingsPage";
import { applyTheme, resolveEffectiveTheme, type ThemePreference } from "@/theme/theme";

const state = vi.hoisted(() => ({ context: undefined as any }));

vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => state.context,
}));

function config(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    schemaVersion: 3,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: { readFile: { enabled: true }, glob: { enabled: true }, grep: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
    bash: { autoDetect: true },
    policies: { toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
    desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false },
    history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
    search: { globTimeoutSeconds: 30, grepTimeoutSeconds: 30 },
    ...overrides,
  };
}

function makeContext(overrides: Record<string, unknown> = {}) {
  const saved = config();
  const setThemePreference = vi.fn((preference: ThemePreference) => applyTheme(resolveEffectiveTheme(preference, false)));
  return {
    config: saved,
    configMetadata: { configDirectory: "D:\\config", configFile: "D:\\config\\config.json" },
    configError: null,
    eligibility: { status: "ready_to_start", canStart: true, message: "ready" },
    apiKeyConfigured: false,
    apiKeyObserved: true,
    restartRequired: false,
    themePreference: "dark" as ThemePreference,
    effectiveTheme: "dark" as const,
    setThemePreference,
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
    setDesktopPreferences: vi.fn().mockResolvedValue({ ok: true }),
    continueInBackground: vi.fn().mockResolvedValue({ ok: true }),
    quitDesktop: vi.fn().mockResolvedValue({ ok: true }),
    resetConfig: vi.fn().mockResolvedValue({ ok: true }),
    startHarness: vi.fn().mockResolvedValue({ ok: true }),
    stopHarness: vi.fn().mockResolvedValue({ ok: true }),
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
  it("saves the independent Auto Start preference for the next app launch", async () => {
    const setDesktopPreferences = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ setDesktopPreferences });
    state.context.config.desktop.autoStartHarnessOnAppLaunch = true;
    const view = render(<SettingsPage />);
    const toggle = screen.getByRole("checkbox", { name: "Auto Start Harness on app launch" });
    expect(toggle).toBeChecked();
    expect(screen.getByText(/does not open the app when you sign in to Windows/)).toBeInTheDocument();
    fireEvent.click(toggle);
    await waitFor(() => expect(setDesktopPreferences).toHaveBeenCalledWith({ autoStartHarnessOnAppLaunch: false }));
    expect(await screen.findByText("Auto Start preference saved for the next app launch.")).toBeInTheDocument();
    state.context.config.desktop.autoStartHarnessOnAppLaunch = false;
    view.rerender(<SettingsPage />);
    expect(toggle).not.toBeChecked();
    fireEvent.click(toggle);
    await waitFor(() => expect(setDesktopPreferences).toHaveBeenCalledWith({ autoStartHarnessOnAppLaunch: true }));
    expect(state.context.startHarness).not.toHaveBeenCalled();
    expect(state.context.stopHarness).not.toHaveBeenCalled();
    expect(state.context.restartHarness).not.toHaveBeenCalled();
    expect(screen.queryByText("Config Mismatch — Restart Required")).not.toBeInTheDocument();
  });

  it("disables only Auto Start while saving and refreshes persisted truth on failure", async () => {
    let finish!: (result: { ok: false; error: { code: string } }) => void;
    const setDesktopPreferences = vi.fn().mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    state.context = makeContext({ setDesktopPreferences, refreshConfig });
    render(<SettingsPage />);
    const toggle = screen.getByRole("checkbox", { name: "Auto Start Harness on app launch" });
    fireEvent.click(toggle);
    expect(toggle).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Connection notifications" })).toBeEnabled();
    finish({ ok: false, error: { code: "CONFIG_WRITE_FAILED" } });
    await waitFor(() => expect(refreshConfig).toHaveBeenCalledOnce());
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).not.toBeChecked();
    expect(screen.getByText("CONFIG_WRITE_FAILED")).toBeInTheDocument();
  });

  it("renders the six settings sections without a page-level Save action", () => {
    render(<SettingsPage />);
    ["general", "notifications", "workspace", "connection", "security", "history", "search", "advanced", "danger"].forEach((name) => {
      expect(document.getElementById(`settings-section-${name}`)).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "Save Changes" })).not.toBeInTheDocument();
  });

  it("saves generation-effective History settings and confirms every clear", async () => {
    const mutateConfig = vi.fn().mockImplementation(async (intent: (latest: AppConfig) => AppConfig) => {
      intent(state.context.config);
      return { ok: true };
    });
    const historyClear = vi.fn().mockResolvedValue({
      deletedRequestCount: 2,
      deletedEventCount: 5,
      deletedApprovalCount: 1,
      footprintBytes: 0,
      completedAtUtc: "2026-11-01T00:00:00Z",
    });
    const historyStatus = vi.fn().mockResolvedValue({
      state: "healthy",
      schemaVersion: 1,
      queueDepth: 0,
      queueCapacity: 1024,
      dropCount: 0,
      effectiveHistoryConfig: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
      footprintBytes: 4096,
    });
    state.context = makeContext({
      client: { historyClear, historyStatus },
      mutateConfig,
      restartRequired: true,
    });
    render(<SettingsPage />);
    expect(document.getElementById("settings-section-history")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("spinbutton", { name: "Retention days" }), { target: { value: "45" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Maximum storage (MiB)" }), { target: { value: "512" } });
    fireEvent.click(screen.getByRole("button", { name: "Save History settings" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const next = mutateConfig.mock.calls[0][0](state.context.config);
    expect(next.history).toEqual({ enabled: true, retentionDays: 45, maxStorageMiB: 512 });
    expect(screen.getByText("Saved History settings differ from the running Runtime generation.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Clear History" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Clear History" }));
    await waitFor(() => expect(historyClear).toHaveBeenCalledOnce());
    expect(screen.getByText("Cleared 2 requests, 5 events and 1 approvals.")).toBeInTheDocument();
  });

  it("validates and saves generation-effective search timeouts", async () => {
    const mutateConfig = vi.fn().mockImplementation(async (intent: (latest: AppConfig) => AppConfig) => {
      intent(state.context.config);
      return { ok: true };
    });
    state.context = makeContext({ mutateConfig });
    render(<SettingsPage />);
    expect(document.getElementById("settings-section-search")).toBeInTheDocument();
    const glob = screen.getByRole("spinbutton", { name: "Glob timeout (seconds)" });
    const grep = screen.getByRole("spinbutton", { name: "Grep timeout (seconds)" });
    fireEvent.change(glob, { target: { value: "4" } });
    expect(screen.getByText("Enter a whole number between 5 and 120.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save Search settings" })).toBeDisabled();
    fireEvent.change(glob, { target: { value: "45" } });
    fireEvent.change(grep, { target: { value: "60" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Search settings" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const next = mutateConfig.mock.calls[0][0](state.context.config);
    expect(next.search).toEqual({ globTimeoutSeconds: 45, grepTimeoutSeconds: 60 });
    expect(screen.getByText("Search settings saved. Restart Harness to apply them.")).toBeInTheDocument();
  });

  it("saves approval and connection notification toggles independently", async () => {
    const setDesktopPreferences = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({ setDesktopPreferences });
    render(<SettingsPage />);
    const approval = screen.getByRole("checkbox", { name: "Approval Tray reminder" });
    expect(screen.getByText("Flash the Tray icon while approvals are pending, including when the window is hidden. No approval pop-up notifications are sent.")).toBeInTheDocument();
    const connection = screen.getByRole("checkbox", { name: "Connection notifications" });
    fireEvent.click(approval);
    await waitFor(() => expect(setDesktopPreferences).toHaveBeenCalledWith({ approvalNotifications: false }));
    fireEvent.click(connection);
    await waitFor(() => expect(setDesktopPreferences).toHaveBeenCalledWith({ connectionNotifications: false }));
    expect(state.context.restartHarness).not.toHaveBeenCalled();
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

  it("defaults a missing theme to Dark and saves Light through scoped config mutation", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    const setThemePreference = vi.fn((preference: ThemePreference) => applyTheme(resolveEffectiveTheme(preference, false)));
    state.context = makeContext({ mutateConfig, setThemePreference });
    render(<SettingsPage />);

    expect(screen.getByRole("radio", { name: "Dark" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("radio", { name: "Light" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());

    const intent = mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig;
    const next = intent(state.context.config);
    expect(next.ui.theme).toBe("light");
    expect(next.workspace).toEqual(state.context.config.workspace);
    expect(next.tools).toEqual(state.context.config.tools);
    expect(state.context.restartHarness).not.toHaveBeenCalled();
  });

  it("selects System and resolves its effective theme from the OS", async () => {
    const originalMatchMedia = window.matchMedia;
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: vi.fn().mockReturnValue({ matches: true }),
    });
    const mutateConfig = vi.fn().mockResolvedValue({ ok: true });
    const setThemePreference = vi.fn((preference: ThemePreference) => applyTheme(resolveEffectiveTheme(preference, true)));
    state.context = makeContext({ mutateConfig, setThemePreference });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("radio", { name: "System" }));

    expect(document.documentElement.dataset.theme).toBe("dark");
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const intent = mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig;
    expect(intent(state.context.config).ui.theme).toBe("system");

    if (originalMatchMedia) Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: originalMatchMedia });
    else Reflect.deleteProperty(window, "matchMedia");
  });

  it("rolls the theme back when persistence fails", async () => {
    const mutateConfig = vi.fn().mockResolvedValue({ ok: false, error: { code: "CONFIG_WRITE_FAILED" } });
    const setThemePreference = vi.fn((preference: ThemePreference) => applyTheme(resolveEffectiveTheme(preference, false)));
    state.context = makeContext({ mutateConfig, setThemePreference });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("radio", { name: "Light" }));

    await waitFor(() => expect(screen.getByRole("radio", { name: "Dark" })).toHaveAttribute("aria-checked", "true"));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(setThemePreference).toHaveBeenNthCalledWith(1, "light");
    expect(setThemePreference).toHaveBeenNthCalledWith(2, "dark");
    expect(screen.getByText("CONFIG_WRITE_FAILED")).toBeInTheDocument();
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
