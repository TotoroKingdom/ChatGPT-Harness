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
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("prevents a full Settings save from overwriting an in-flight language preference", async () => {
  let finish!: () => void;
  const gate = new Promise<void>((resolve) => { finish = resolve; });
  const persist = state.client.uiLanguageSet.bind(state.client);
  vi.spyOn(state.client, "uiLanguageSet").mockImplementation(async (language) => { await gate; return persist(language); });
  const fullSave = vi.spyOn(state.client, "configSave");
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
  await screen.findByDisplayValue("mock-tunnel");
  fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "reviewed-draft" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
  const save = await screen.findByRole("button", { name: "保存更改" });
  try {
    expect(save).toBeDisabled();
    fireEvent.click(save);
    expect(fullSave).not.toHaveBeenCalled();
  } finally { finish(); }
  await screen.findByText("语言偏好已保存。");
  fireEvent.click(screen.getByRole("button", { name: "保存更改" }));
  await waitFor(() => expect(fullSave).toHaveBeenCalledOnce());
  await waitFor(async () => {
    const saved = await state.client.configGet();
    expect(saved?.ui.language).toBe("zh-CN");
    expect(saved?.tunnel.tunnelId).toBe("reviewed-draft");
  });
});

it("prevents language and reset mutations while a full Settings save is in flight", async () => {
  let finish!: () => void;
  const gate = new Promise<void>((resolve) => { finish = resolve; });
  const persist = state.client.configSave.bind(state.client);
  vi.spyOn(state.client, "configSave").mockImplementation(async (config, key) => { await gate; return persist(config, key); });
  const language = vi.spyOn(state.client, "uiLanguageSet");
  const reset = vi.spyOn(state.client, "configReset");
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
  await screen.findByDisplayValue("mock-tunnel");
  fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "reviewed-draft" } });
  fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));
  try {
    expect(screen.getByRole("combobox", { name: "Language" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset Local Config" })).toBeDisabled();
    expect(language).not.toHaveBeenCalled();
    expect(reset).not.toHaveBeenCalled();
  } finally { finish(); }
  await screen.findByText("Changes saved.");
  expect(screen.getByRole("combobox", { name: "Language" })).toBeEnabled();
});
