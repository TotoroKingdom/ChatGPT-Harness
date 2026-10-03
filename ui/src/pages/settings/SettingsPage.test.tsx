import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { FirstRunWizard, SettingsPage } from "./SettingsPage";

const state = vi.hoisted(() => ({ context: undefined as any }));

vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => state.context,
}));

function config(overrides: Partial<AppConfig> = {}): AppConfig {
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
    ...overrides,
  };
}

function makeContext(overrides: Record<string, unknown> = {}) {
  return {
    config: config(),
    configMetadata: { configDirectory: "D:\\config", configFile: "D:\\config\\config.json" },
    configError: null,
    eligibility: { status: "ready_to_start", canStart: true, message: "Local Runtime is ready to start." },
    apiKeyConfigured: false,
    restartRequired: false,
    chooseWorkspace: vi.fn().mockResolvedValue("D:\\workspace"),
    chooseTunnelClient: vi.fn().mockResolvedValue("D:\\tunnel-client"),
    detectTunnelClient: vi.fn().mockResolvedValue(null),
    deleteApiKey: vi.fn().mockResolvedValue(undefined),
    refreshConfig: vi.fn().mockResolvedValue(undefined),
    saveConfig: vi.fn().mockResolvedValue({ ok: true }),
    setUiLanguage: vi.fn().mockResolvedValue({ ok: true }),
    resetConfig: vi.fn().mockResolvedValue({ ok: true }),
    restartHarness: vi.fn().mockResolvedValue({ ok: true }),
    startHarness: vi.fn().mockResolvedValue({ ok: true }),
    ...overrides,
  };
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

describe("SettingsPage bilingual presentation", () => {
  it("renders the Settings surface in English and Chinese", async () => {
    render(<SettingsPage />);

    expect(screen.getByText("Configuration stays local and explicit")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save Changes" })).toBeInTheDocument();

    await i18n.changeLanguage("zh-CN");

    expect(screen.getByText("配置保持本地且明确")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存更改" })).toBeInTheDocument();
    expect(screen.getByText("Tunnel ID（必填）")).toBeInTheDocument();
  });

  it("switches immediately, persists only the UI preference, updates html lang, and does not call Runtime lifecycle", async () => {
    let resolvePersistence: ((value: { ok: boolean }) => void) | undefined;
    const pendingPersistence = new Promise<{ ok: boolean }>((resolve) => { resolvePersistence = resolve; });
    state.context = makeContext({ setUiLanguage: vi.fn(() => pendingPersistence) });
    render(<SettingsPage />);

    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });

    await waitFor(() => expect(state.context.setUiLanguage).toHaveBeenCalledWith("zh-CN"));
    expect(i18n.language).toBe("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("button", { name: "保存更改" })).toBeInTheDocument();
    expect(screen.queryByText("语言偏好已保存。")).not.toBeInTheDocument();
    resolvePersistence?.({ ok: true });
    expect(await screen.findByText("语言偏好已保存。")).toBeInTheDocument();
    expect(state.context.startHarness).not.toHaveBeenCalled();
    expect(state.context.restartHarness).not.toHaveBeenCalled();
  });

  it("rolls the locale back and presents a localized error when isolated persistence fails", async () => {
    state.context = makeContext({
      setUiLanguage: vi.fn().mockResolvedValue({ ok: false, error: { code: "CONFIG_WRITE_FAILED", message: "write failed" } }),
    });
    render(<SettingsPage />);

    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });

    await waitFor(() => expect(i18n.language).toBe("en"));
    expect(document.documentElement.lang).toBe("en");
    expect(screen.getByRole("combobox", { name: "Language" })).toHaveValue("");
    expect(await screen.findByText("Configuration could not be saved.")).toBeInTheDocument();
    expect(screen.queryByText("语言偏好已保存。")).not.toBeInTheDocument();
  });

  it("keeps a dirty draft and merges the latest persisted language into a later full save", async () => {
    const persisted = config({ ui: { language: "zh-CN" } });
    const saveConfig = vi.fn().mockResolvedValue({ ok: true });
    const context = makeContext({ saveConfig });
    state.context = context;
    const view = render(<SettingsPage />);

    fireEvent.change(screen.getByRole("textbox", { name: "Tunnel ID (required)" }), { target: { value: "draft-tunnel" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await waitFor(() => expect(context.setUiLanguage).toHaveBeenCalledWith("zh-CN"));

    context.config = persisted;
    view.rerender(<SettingsPage />);
    expect(screen.getByRole("textbox", { name: "Tunnel ID（必填）" })).toHaveValue("draft-tunnel");
    fireEvent.click(screen.getByRole("button", { name: "保存更改" }));

    await waitFor(() => expect(saveConfig).toHaveBeenCalledOnce());
    expect(saveConfig.mock.calls[0][0].tunnel.tunnelId).toBe("draft-tunnel");
    expect(saveConfig.mock.calls[0][0].ui).toEqual({ language: "zh-CN" });
    expect(context.restartHarness).not.toHaveBeenCalled();
  });

  it("does not restore a cleared Follow system preference from a dirty draft on later Save", async () => {
    await i18n.changeLanguage("zh-CN");
    const context = makeContext({ config: config({ ui: { language: "zh-CN" } }) });
    state.context = context;
    const view = render(<SettingsPage />);

    fireEvent.change(screen.getByRole("textbox", { name: "Tunnel ID（必填）" }), { target: { value: "draft-tunnel" } });
    fireEvent.change(screen.getByRole("combobox", { name: "语言" }), { target: { value: "" } });
    await waitFor(() => expect(context.setUiLanguage).toHaveBeenCalledWith(null));

    context.config = config({ ui: {} });
    view.rerender(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));

    await waitFor(() => expect(context.saveConfig).toHaveBeenCalledOnce());
    expect(context.saveConfig.mock.calls[0][0].ui).toEqual({});
    expect(context.saveConfig.mock.calls[0][0].tunnel.tunnelId).toBe("draft-tunnel");
  });

  it("keeps an existing Restart Required notice visible after a language switch", async () => {
    state.context = makeContext({ restartRequired: true });
    render(<SettingsPage />);

    expect(screen.getByText("Config Mismatch — Restart Required")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });

    expect(await screen.findByText("配置不匹配 — 需要重启")).toBeInTheDocument();
    expect(state.context.restartHarness).not.toHaveBeenCalled();
  });

  it("translates validation state on language changes without storing translated prose", async () => {
    render(<SettingsPage />);

    fireEvent.change(screen.getByRole("textbox", { name: "Control-plane HTTP proxy (optional)" }), { target: { value: "ftp://proxy" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));
    expect(await screen.findByText("Proxy URL must use http:// or https://.")).toBeInTheDocument();

    await i18n.changeLanguage("zh-CN");
    expect(screen.getByText("代理 URL 必须使用 http:// 或 https://。")).toBeInTheDocument();
  });
});

describe("FirstRunWizard bilingual presentation", () => {
  it("renders the first-run flow in English and Chinese", async () => {
    render(<FirstRunWizard onDone={vi.fn()} />);

    expect(screen.getByText("Local Runtime Setup")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save Configuration" })).toBeInTheDocument();

    await i18n.changeLanguage("zh-CN");

    expect(screen.getByText("本地 Runtime 设置")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存配置" })).toBeInTheDocument();
    expect(screen.getByText("Tunnel ID（必填）")).toBeInTheDocument();
  });

  it("uses typed eligibility presentation and keeps start calls out of language switching", async () => {
    const startHarness = vi.fn().mockResolvedValue({ ok: true });
    state.context = makeContext({
      eligibility: { status: "tunnel_id_required", canStart: false, message: "Configure a Tunnel ID before starting Harness." },
      startHarness,
    });
    render(<FirstRunWizard onDone={vi.fn()} />);

    expect(screen.getByText("Tunnel ID required")).toBeInTheDocument();
    await i18n.changeLanguage("zh-CN");
    expect(screen.getByText("需要 Tunnel ID")).toBeInTheDocument();
    expect(startHarness).not.toHaveBeenCalled();
  });
});
