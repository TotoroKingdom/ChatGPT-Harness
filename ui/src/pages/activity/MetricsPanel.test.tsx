import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MetricsQueryResult } from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { MetricsPanel } from "./MetricsPanel";

const harness = vi.hoisted(() => ({ current: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => harness.current }));

const result: MetricsQueryResult = {
  counts: {
    receivedCalls: 3, executionAttempts: 2, successfulCalls: 1, failedCalls: 1,
    deniedCalls: 1, cancelledCalls: 0, timedOutCalls: 0, preExecutionFailedCalls: 0,
    approvalExpiredCalls: 0, approvalCancelledCalls: 0, incompleteCalls: 0,
    unknownCalls: 0, legacyUntracedCalls: 0, completedExecutionDenominator: 2,
    executionSuccessNumerator: 1, executionSuccessRate: 0.5, executionSuccessSampleStatus: "INSUFFICIENT",
  },
  execution: { sampleCount: 2, averageMs: 12.5, sampleStatus: "INSUFFICIENT" },
  approvalWait: { sampleCount: 0, sampleStatus: "NO_DATA" },
  requestWall: { sampleCount: 3, averageMs: 25, p95Ms: undefined, sampleStatus: "INSUFFICIENT" },
  coverage: {
    completeRequestCount: 3, incompleteRequestCount: 0, unknownRequestCount: 0, legacyUntracedRequestCount: 0,
    executionDurationEligibleCount: 2, approvalWaitDurationEligibleCount: 0, requestWallDurationEligibleCount: 3,
    storeStatus: { state: "healthy", schemaVersion: 2, queueDepth: 0, queueCapacity: 1024, dropCount: 0, effectiveHistoryConfig: { enabled: true, retentionDays: 30, maxStorageMiB: 256 }, footprintBytes: 1 },
    dropCount: 0,
  },
};

beforeEach(async () => { await initializeLanguage("en"); });
afterEach(() => { cleanup(); void i18n.changeLanguage("en"); });

describe("M08 Metrics panel", () => {
  it("queries bounded range and renders denominator plus duration sample state", async () => {
    const metricsQuery = vi.fn().mockResolvedValue(result);
    harness.current = { client: { metricsQuery } };
    render(<MetricsPanel />);
    await waitFor(() => expect(screen.getByText("50.0%")).toBeInTheDocument());
    expect(metricsQuery).toHaveBeenCalledWith(expect.objectContaining({ fromUtc: expect.any(String), toUtc: expect.any(String) }));
    expect(screen.getByText("1 / 2 completed executions")).toBeInTheDocument();
    expect(screen.getByText("Insufficient sample")).toBeInTheDocument();
    expect(screen.getAllByText("Timing unavailable").length).toBeGreaterThan(0);
    expect(screen.queryByText(/Agent accuracy|Tool selection accuracy/i)).not.toBeInTheDocument();
  });

  it("applies exact Tool filter", async () => {
    const metricsQuery = vi.fn().mockResolvedValue(result);
    harness.current = { client: { metricsQuery } };
    render(<MetricsPanel />);
    await waitFor(() => expect(metricsQuery).toHaveBeenCalled());
    fireEvent.change(screen.getByRole("combobox", { name: "Tool" }), { target: { value: "grep" } });
    await waitFor(() => expect(metricsQuery).toHaveBeenLastCalledWith(expect.objectContaining({ tool: "grep" })));
  });
});
