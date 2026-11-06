import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  EvidenceStoreStatus,
  HistoryGetResult,
  HistoryQueryResult,
  HistoryRequestSummary,
  TraceGetResult,
} from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { HistoryPanel } from "./HistoryPanel";

const harness = vi.hoisted(() => ({ current: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => harness.current,
}));

const item = (overrides: Partial<HistoryRequestSummary> = {}): HistoryRequestSummary => ({
  runtimeId: "runtime-a",
  requestId: "request-a",
  toolName: "read_file",
  firstSeenAtUtc: "2026-11-01T00:00:00Z",
  lastSeenAtUtc: "2026-11-01T00:00:01Z",
  terminalState: "succeeded",
  outcome: "success",
  policyDecision: "allow",
  approvalState: "allow_once",
  durationMs: 12,
  evidenceCoverage: "COMPLETE",
  ...overrides,
});

const healthy = (overrides: Partial<EvidenceStoreStatus> = {}): EvidenceStoreStatus => ({
  state: "healthy",
  schemaVersion: 1,
  queueDepth: 0,
  queueCapacity: 1024,
  dropCount: 0,
  effectiveHistoryConfig: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
  footprintBytes: 4096,
  ...overrides,
});

function detail(): HistoryGetResult {
  return {
    summary: item(),
    events: [{
      schemaVersion: 1,
      eventId: "event-safe",
      runtimeId: "runtime-a",
      requestId: "request-a",
      toolName: "read_file",
      occurredAtUtc: "2026-11-01T00:00:00Z",
      recordedAtUtc: "2026-11-01T00:00:00Z",
      stage: "execution_completed",
      lifecycleState: "completed",
      workspaceValidation: "passed",
      policyDecision: "allow",
      matchedRuleIds: [],
      outcome: "success",
      durationMs: 12,
      sanitizedArguments: { tool: "read_file", logicalPath: "notes.txt" },
      coverage: "COMPLETE",
    }],
    terminalApproval: {
      runtimeId: "runtime-a",
      approvalId: "approval-a",
      requestId: "request-a",
      toolName: "read_file",
      terminalAtUtc: "2026-11-01T00:00:00Z",
      terminalState: "allow_once",
      decision: "allow_once",
    },
    storeStatus: healthy(),
  };
}

function trace(): TraceGetResult {
  return {
    runtimeId: "runtime-a",
    requestId: "request-a",
    traceId: "trace-a",
    toolName: "read_file",
    outcome: "success",
    policyDecision: "allow",
    approvalState: "none",
    executorStarted: true,
    coverage: "complete",
    availability: "available",
    spans: [{
      spanId: "span-a", traceId: "trace-a", runtimeId: "runtime-a", requestId: "request-a", spanSequence: 0,
      spanName: "request_wall", startedAtUtc: "2026-11-01T00:00:00Z", startedOffsetMs: 0, durationMs: 12,
      status: "success", coverage: "COMPLETE",
    }],
    storeStatus: healthy({ schemaVersion: 2 }),
  };
}

function setup(result?: Partial<HistoryQueryResult>, status: EvidenceStoreStatus = healthy()) {
  const historyQuery = vi.fn().mockResolvedValue({
    items: [item()],
    storeStatus: status,
    ...result,
  });
  const historyGet = vi.fn().mockResolvedValue(detail());
  const traceGet = vi.fn().mockResolvedValue(trace());
  harness.current = { client: { historyQuery, historyGet, traceGet } };
  render(<HistoryPanel />);
  return { historyQuery, historyGet, traceGet };
}

beforeEach(async () => {
  await initializeLanguage("en");
});

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

describe("M03 History panel", () => {
  it("loads the last 24 hours by default and renders safe coverage", async () => {
    const view = setup();
    await waitFor(() => expect(screen.getByTestId("history-row-runtime-a-request-a")).toBeInTheDocument());
    const params = view.historyQuery.mock.calls[0][0];
    expect(params.fromUtc).toBeTruthy();
    expect(params.limit).toBe(50);
    expect(screen.getByText("COMPLETE")).toBeInTheDocument();
    expect(screen.getByText("History storage is healthy.")).toBeInTheDocument();
  });

  it("resets keyset state when a filter changes and supports load more", async () => {
    const view = setup({ nextCursor: "mock:1" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Load more" })).toBeInTheDocument());
    fireEvent.change(screen.getByRole("combobox", { name: "Tool" }), { target: { value: "write" } });
    await waitFor(() => expect(view.historyQuery).toHaveBeenCalledWith(expect.objectContaining({ tool: "write" })));
    await waitFor(() => expect(screen.getByRole("button", { name: "Load more" })).toBeInTheDocument());
    view.historyQuery.mockResolvedValueOnce({
      items: [item({ requestId: "request-b", toolName: "write" })],
      storeStatus: healthy(),
    });
    fireEvent.click(screen.getByRole("button", { name: "Load more" }));
    await waitFor(() => expect(view.historyQuery).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: "mock:1", tool: "write" })));
  });

  it("opens an exact request trace from the selected History detail", async () => {
    const view = setup();
    await waitFor(() => expect(screen.getByTestId("history-row-runtime-a-request-a")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("history-row-runtime-a-request-a").querySelector("button")!);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open request trace" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Open request trace" }));
    await waitFor(() => expect(view.traceGet).toHaveBeenCalledWith({ runtimeId: "runtime-a", requestId: "request-a" }));
    expect(screen.getByText("Runtime-observed timeline")).toBeInTheDocument();
    expect(screen.getByText("trace-a")).toBeInTheDocument();
    expect(screen.getByText("Request wall time")).toBeInTheDocument();
  });

  it("renders historical detail without approval or retry authority", async () => {
    const view = setup();
    await waitFor(() => expect(screen.getByTestId("history-row-runtime-a-request-a")).toBeInTheDocument());
    fireEvent.click(within(screen.getByTestId("history-row-runtime-a-request-a")).getByRole("button"));
    await waitFor(() => expect(view.historyGet).toHaveBeenCalledWith({ runtimeId: "runtime-a", requestId: "request-a" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Safe event timeline")).toBeInTheDocument();
    expect(screen.getByText("Terminal Approval evidence")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /approve|deny|retry/i })).not.toBeInTheDocument();
  });

  it("surfaces degraded and incomplete evidence instead of calling it empty", async () => {
    setup({
      items: [item({ evidenceCoverage: "INCOMPLETE", terminalState: "incomplete", outcome: undefined })],
    }, healthy({ state: "degraded", dropCount: 2 }));
    await waitFor(() => expect(screen.getByText("History storage is degraded. Existing evidence is preserved.")).toBeInTheDocument());
    expect(screen.getByText("INCOMPLETE")).toBeInTheDocument();
    expect(screen.queryByText("No retained History matches these filters")).not.toBeInTheDocument();
  });
});
