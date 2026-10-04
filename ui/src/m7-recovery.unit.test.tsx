import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "@/App";
import { initializeLanguage } from "@/i18n";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({ createHarnessControlClient: () => state.client }));
beforeEach(async () => { await initializeLanguage("en"); state.client = new MockHarnessControlClient("ready"); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); Reflect.deleteProperty(navigator, "clipboard"); });

it.each(["CONFIG_MIGRATION_FAILED", "CONFIG_SCHEMA_UNSUPPORTED"])("preserves metadata and offers safe recovery for %s", async (code) => {
  vi.spyOn(state.client, "configGet").mockRejectedValue({ code, message: "RAW_SECRET_SENTINEL" });
  const reset = vi.spyOn(state.client, "configReset");
  const retry = vi.spyOn(state.client, "configMetadata");
  const copy = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: copy } });
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  const { container } = render(<App />);
  expect(await screen.findByText(code)).toBeInTheDocument();
  expect(screen.getByText("The original configuration has not been overwritten.")).toBeInTheDocument();
  const path = screen.getByRole("textbox", { name: "Config file path" }) as HTMLInputElement;
  expect(path.value).toContain("config.json");
  expect(container.innerHTML).not.toContain("RAW_SECRET_SENTINEL");
  fireEvent.click(screen.getByRole("button", { name: "Copy config path" }));
  expect(copy).toHaveBeenCalledWith(path.value);
  fireEvent.click(screen.getByRole("button", { name: "Retry reload / migration" }));
  await waitFor(() => expect(retry).toHaveBeenCalledTimes(2));
  fireEvent.click(screen.getByRole("button", { name: "Reset configuration" }));
  expect(confirm).toHaveBeenCalledOnce(); expect(reset).not.toHaveBeenCalled();
  confirm.mockReturnValue(true);
  fireEvent.click(screen.getByRole("button", { name: "Reset configuration" }));
  await waitFor(() => expect(reset).toHaveBeenCalledOnce());
});

it("retry can recover a repaired configuration without reset", async () => {
  const saved = await state.client.configGet();
  vi.spyOn(state.client, "configGet").mockRejectedValueOnce({ code: "CONFIG_MIGRATION_FAILED" }).mockResolvedValue(saved);
  const reset = vi.spyOn(state.client, "configReset");
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Retry reload / migration" }));
  expect(await screen.findByRole("heading", { name: "Workspace", level: 2 })).toBeInTheDocument();
  expect(reset).not.toHaveBeenCalled();
});

it("shows localized Chinese recovery while keeping the machine code unchanged", async () => {
  await initializeLanguage("zh-CN");
  vi.spyOn(state.client, "configGet").mockRejectedValue({ code: "CONFIG_SCHEMA_UNSUPPORTED" });
  render(<App />);
  expect(await screen.findByText("CONFIG_SCHEMA_UNSUPPORTED")).toBeInTheDocument();
  expect(screen.getByText("原配置未被覆盖。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重试加载／迁移" })).toBeInTheDocument();
});
