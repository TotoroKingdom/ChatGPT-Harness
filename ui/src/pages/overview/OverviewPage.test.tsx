import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import { HarnessDataProvider } from "@/app/providers/HarnessDataProvider";
import { OverviewPage } from "./OverviewPage";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({ createHarnessControlClient: () => state.client }));
beforeEach(() => { state.client = new MockHarnessControlClient("ready"); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("keeps Stop secondary when ready and surfaces Restart as primary after a saved change", async () => {
  const config = (await state.client.configGet())!;
  await state.client.configSave({ ...config, workspace: { rootPath: "D:\\changed" } });
  render(<HarnessDataProvider><OverviewPage /></HarnessDataProvider>);
  const restart = await screen.findByRole("button", { name: "Restart Harness" });
  expect(restart).toHaveClass("button-primary");
  expect(screen.getByRole("button", { name: "Stop Harness" })).toHaveClass("button-secondary");
});
it("retains Stop and a settings remediation for a degraded managed session", async () => {
  vi.spyOn(state.client, "harnessStatus").mockResolvedValue({ ...state.client.getHarnessStatus(), lifecycle: "degraded", tunnel: "error" });
  const navigate = vi.fn();
  render(<HarnessDataProvider><OverviewPage onNavigate={navigate} /></HarnessDataProvider>);
  fireEvent.click(await screen.findByRole("button", { name: "Open Settings" }));
  expect(navigate).toHaveBeenCalledWith("settings", { settingsSection: "connection" });
  expect(screen.getByRole("button", { name: "Stop Harness" })).toBeEnabled();
});
it("never presents unreadable Harness status as ready or actionable start", async () => {
  vi.spyOn(state.client, "harnessStatus").mockRejectedValue(Error("offline"));
  render(<HarnessDataProvider><OverviewPage /></HarnessDataProvider>);
  expect(await screen.findByText("Harness is Unknown")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Start Harness" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Try again" })).toBeEnabled();
});
it("deduplicates lifecycle commands and shows observation rather than command acknowledgement as readiness", async () => {
  let finish!: () => void;
  const stop = vi.spyOn(state.client, "harnessStop").mockImplementation(async () => { await new Promise<void>((resolve) => { finish = resolve; }); return { ...state.client.getHarnessStatus(), lifecycle: "stopping", runtime: "stopping", mcp: "stopping", tunnel: "stopping" }; });
  render(<HarnessDataProvider><OverviewPage /></HarnessDataProvider>);
  const button = await screen.findByRole("button", { name: "Stop Harness" });
  fireEvent.click(button); fireEvent.click(button);
  expect(stop).toHaveBeenCalledOnce(); expect(button).toBeDisabled();
  finish();
  await waitFor(() => expect(screen.getByRole("heading", { name: "Harness is Stopping" })).toBeInTheDocument());
  expect(screen.queryByRole("heading", { name: "Harness is Ready" })).not.toBeInTheDocument();
});
it("surfaces the Runtime's fail-closed policy flag above a reported ready lifecycle", async () => {
  const snapshot = await state.client.policiesList();
  vi.spyOn(state.client, "policiesList").mockResolvedValue({ ...snapshot, effective: { ...snapshot.effective!, valid: false, status: "invalid", statusReason: "invalid_policy_snapshot" } });
  const navigate = vi.fn();
  render(<HarnessDataProvider><OverviewPage onNavigate={navigate} /></HarnessDataProvider>);
  expect(await screen.findByRole("heading", { name: "Harness needs attention" })).toBeInTheDocument();
  expect(screen.getAllByRole("alert").some((notice) => notice.textContent?.includes("Execution remains denied until valid"))).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Review Policies" }));
  expect(navigate).toHaveBeenCalledWith("policies");
  expect(screen.getByRole("button", { name: "Stop Harness" })).toBeEnabled();
});
