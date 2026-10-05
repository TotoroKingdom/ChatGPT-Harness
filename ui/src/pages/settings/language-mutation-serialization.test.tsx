import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "@/App";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({ createHarnessControlClient: () => state.client }));

beforeEach(() => {
  window.history.replaceState({}, "", "/");
  state.client = new MockHarnessControlClient("ready");
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "", "/");
});

async function openSettings() {
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
  await screen.findByText("mock-tunnel");
}
function openConnectionEditor() {
  fireEvent.click(document.querySelector("#settings-section-connection button")!);
}
function applyConnectionDraft() {
  fireEvent.click(screen.getByRole("dialog").querySelector("button.button-primary")!);
}

it("serializes Language with Connection Apply while preserving both scoped intents", async () => {
  let finish!: () => void;
  const gate = new Promise<void>((resolve) => { finish = resolve; });
  const persist = state.client.configSave.bind(state.client);
  const save = vi.spyOn(state.client, "configSave").mockImplementation(async (config, apiKey) => { await gate; return persist(config, apiKey); });
  const language = vi.spyOn(state.client, "uiLanguageSet");

  await openSettings();
  openConnectionEditor();
  fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "reviewed-draft" } });
  applyConnectionDraft();
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });

  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  expect(language).not.toHaveBeenCalled();
  finish();
  await screen.findByText("语言偏好已保存。");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  const saved = await state.client.configGet();
  expect(saved?.ui.language).toBe("zh-CN");
  expect(saved?.tunnel.tunnelId).toBe("reviewed-draft");
});

it("does not start Reset while a Connection Apply is saving", async () => {
  let finish!: () => void;
  const gate = new Promise<void>((resolve) => { finish = resolve; });
  const persist = state.client.configSave.bind(state.client);
  vi.spyOn(state.client, "configSave").mockImplementation(async (config, apiKey) => { await gate; return persist(config, apiKey); });
  const reset = vi.spyOn(state.client, "configReset");

  await openSettings();
  openConnectionEditor();
  fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "reviewed-draft" } });
  applyConnectionDraft();
  const resetButton = screen.getByRole("button", { name: "Reset Local Config" });
  expect(resetButton).toBeDisabled();
  expect(reset).not.toHaveBeenCalled();
  finish();
  await waitFor(() => expect(resetButton).toBeEnabled());
});
