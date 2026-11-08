import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import { HarnessDataProvider } from "@/app/providers/HarnessDataProvider";
import type { ActivityEvent } from "@protocol/v1";
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

it("keeps a direct Start retry alongside settings remediation for an eligible error state", async () => {
  const eligibility = await state.client.harnessStartEligibility();
  vi.spyOn(state.client, "harnessStartEligibility").mockResolvedValue({ ...eligibility, canStart: true });
  vi.spyOn(state.client, "harnessStatus").mockResolvedValue({
    ...state.client.getHarnessStatus(),
    lifecycle: "error",
    runtime: "error",
  });
  const start = vi.spyOn(state.client, "harnessStart").mockResolvedValue({
    ...state.client.getHarnessStatus(),
    lifecycle: "ready",
    runtime: "ready",
    tunnel: "ready",
    mcp: "ready",
  });
  const navigate = vi.fn();
  render(<HarnessDataProvider><OverviewPage onNavigate={navigate} /></HarnessDataProvider>);
  const retry = await screen.findByRole("button", { name: "Start Harness" });
  const settings = screen.getByRole("button", { name: "Open Settings" });
  expect(retry).toHaveClass("button-primary");
  expect(settings).toHaveClass("button-secondary");
  fireEvent.click(settings);
  expect(navigate).toHaveBeenCalledWith("settings", { settingsSection: "connection" });
  fireEvent.click(retry);
  await waitFor(() => expect(start).toHaveBeenCalledOnce());
});

it("maps Overview activity markers from event semantics instead of unconditional success", async () => {
  const event = (overrides: Partial<ActivityEvent> & Pick<ActivityEvent, "eventId" | "sequence" | "eventType">): ActivityEvent => ({
    runtimeId: "runtime_mock_01",
    timestamp: "2026-11-01T00:00:00Z",
    summary: "fixture",
    metadata: {},
    ...overrides,
  });
  vi.spyOn(state.client, "activityList").mockResolvedValue({
    items: [
      event({ eventId: "done", sequence: "1", eventType: "execution_completed" }),
      event({ eventId: "failed", sequence: "2", eventType: "execution_failed" }),
      event({ eventId: "started", sequence: "3", eventType: "runtime_started" }),
      event({ eventId: "unknown", sequence: "4", eventType: "policy_evaluated" }),
    ],
    latestSequence: "4",
  });
  render(<HarnessDataProvider><OverviewPage /></HarnessDataProvider>);
  await screen.findByText("Execution completed");
  const marker = (label: string) => screen.getByText(label).closest(".activity-mini-row")?.querySelector(".event-marker") ?? null;
  expect(marker("Execution completed")).toHaveAttribute("data-state", "success");
  expect(marker("Execution failed")).toHaveAttribute("data-state", "danger");
  expect(marker("Runtime started")).toHaveAttribute("data-state", "neutral");
  expect(marker("Policy evaluated")).toHaveAttribute("data-state", "neutral");
});
