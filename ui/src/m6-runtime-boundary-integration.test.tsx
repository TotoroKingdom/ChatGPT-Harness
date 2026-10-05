import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import { initializeLanguage } from "@/i18n";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

/*
 * These two cases deliberately keep the real HarnessDataProvider and the
 * local typed client.  Only the factory is pointed at a deterministic client;
 * this is the UI/host boundary needed to prove recovery and lifecycle
 * non-interference, rather than a page-only mocked context.
 */
const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => state.client,
}));

beforeEach(async () => {
  await initializeLanguage("en");
  document.documentElement.lang = "en";
  state.client = new MockHarnessControlClient("ready");
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("M6 real Provider/client boundary Integration", () => {
  it("INT-19: configGet failure renders First Run recovery UI and keeps reset available", async () => {
    const client = state.client;
    const persisted = await client.configGet();
    const configGet = vi.spyOn(client, "configGet")
      .mockImplementationOnce(async () => { throw { code: "CONFIG_INVALID" }; })
      .mockResolvedValue(persisted);
    const reset = vi.spyOn(client, "configReset");

    render(<App />);
    expect(await screen.findByText("Set up your local Harness.")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Configuration needs attention");
    const resetButton = screen.getAllByRole("button", { name: "Reset configuration" })[0];
    expect(resetButton).toBeEnabled();
    fireEvent.click(resetButton);
    expect(screen.getByRole("dialog", { name: "Reset local configuration?" })).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: "Reset configuration" }).at(-1)!);
    await waitFor(() => expect(reset).toHaveBeenCalledOnce());
    expect(configGet).toHaveBeenCalled();
    expect(await screen.findByRole("heading", { name: "Workspace", level: 2 })).toBeInTheDocument();
  });

  it("INT-37: actual Provider language mutation persists UI preference without Runtime/Harness lifecycle calls", async () => {
    const client = state.client;
    const before = await client.configGet();
    const lifecycle = ["runtimeStart", "runtimeStop", "harnessStart", "harnessStop", "harnessRestart"]
      .map((method) => vi.spyOn(client, method as "runtimeStart"));
    const languageSet = vi.spyOn(client, "uiLanguageSet");

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /Settings$/ }));
    await screen.findByText("mock-tunnel");
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    expect(await screen.findByText("语言偏好已保存。" )).toBeInTheDocument();

    expect(languageSet).toHaveBeenCalledWith("zh-CN");
    expect((await client.configGet())?.configRevision).toBe(before?.configRevision);
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  });
});
