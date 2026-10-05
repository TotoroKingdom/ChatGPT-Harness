import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import { bootstrapPresentation, i18n } from "@/i18n";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({ createHarnessControlClient: () => state.client }));
beforeEach(() => {
  window.history.replaceState({}, "", "/");
  state.client = new MockHarnessControlClient("ready");
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); window.history.replaceState({}, "", "/"); });

async function openSettings() {
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
  await screen.findByText("mock-tunnel");
}
function openConnectionEditor() {
  fireEvent.click(document.querySelector("#settings-section-connection button")!);
}
function lifecycleSpies() {
  return ["runtimeStart", "runtimeStop", "harnessStart", "harnessStop", "harnessRestart"].map((method) =>
    vi.spyOn(state.client, method as "runtimeStart"));
}

describe("language preference with the actual Provider and local mock adapter", () => {
  it("persists immediately without committing a dirty draft, changing revision or Runtime identity", async () => {
    const before = await state.client.configGet();
    const runtime = await state.client.runtimeStatus();
    const lifecycle = lifecycleSpies();
    const fullSave = vi.spyOn(state.client, "configSave");
    let finish!: () => void;
    const gate = new Promise<void>((resolve) => { finish = resolve; });
    const persist = state.client.uiLanguageSet.bind(state.client);
    vi.spyOn(state.client, "uiLanguageSet").mockImplementation(async (language) => { await gate; return persist(language); });
    await openSettings();
    openConnectionEditor();
    fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "unsaved-draft" } });
    const input = screen.getByDisplayValue("unsaved-draft");
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await screen.findByRole("combobox", { name: "语言" });
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByDisplayValue("unsaved-draft")).toBe(input);
    expect((await state.client.configGet())?.ui.language).toBeUndefined();
    finish();
    await screen.findByText("语言偏好已保存。");
    const saved = await state.client.configGet();
    expect(saved?.ui.language).toBe("zh-CN");
    expect(saved?.configRevision).toBe(before?.configRevision);
    expect(saved?.tunnel).toEqual(before?.tunnel);
    expect(screen.getByDisplayValue("unsaved-draft")).toBe(input);
    expect(fullSave).not.toHaveBeenCalled();
    expect(await state.client.runtimeStatus()).toEqual(runtime);
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
    await bootstrapPresentation(() => state.client.configGet(), () => {
      expect(i18n.language).toBe("zh-CN");
    }, ["en-US"]);
  });

  it("preserves existing restart debt and active session across a UI-only change", async () => {
    const config = (await state.client.configGet())!;
    await state.client.configSave({ ...config, tunnel: { ...config.tunnel, tunnelId: "changed" } });
    const before = await state.client.configGet();
    const runtime = await state.client.runtimeStatus();
    const session = state.client.getHarnessStatus().activeSessionId;
    const lifecycle = lifecycleSpies();
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
    await screen.findByText("Config Mismatch — Restart Required");
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await screen.findByText("语言偏好已保存。");
    expect(screen.getByText("配置不匹配 — 需要重启")).toBeInTheDocument();
    expect((await state.client.configGet())?.configRevision).toBe(before?.configRevision);
    expect(await state.client.runtimeStatus()).toEqual(runtime);
    expect(state.client.getHarnessStatus().activeSessionId).toBe(session);
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  });

  it("rolls back a failed language save and retains persisted config", async () => {
    const before = await state.client.configGet();
    const lifecycle = lifecycleSpies();
    vi.spyOn(state.client, "uiLanguageSet").mockRejectedValue({ code: "CONFIG_WRITE_FAILED", message: "diagnostic" });
    await openSettings();
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Language" })).toHaveValue(""));
    expect(document.documentElement.lang).toBe("en");
    expect(screen.queryByText("语言偏好已保存。")).not.toBeInTheDocument();
    expect(await state.client.configGet()).toEqual(before);
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  });
});
