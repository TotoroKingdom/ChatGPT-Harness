import { describe, expect, it } from "vitest";
import type { StartupAttempt } from "@protocol/v1";
import { compareStartupBenchmark, computeStartupCohortStats } from "./startup-benchmark";

function attempt(cohort: "cold" | "warm", total: number, phaseDuration = total): StartupAttempt {
  return {
    attemptId: `${cohort}-${total}`,
    harnessSessionId: `${cohort}-session`,
    cohort,
    startedAt: "2026-10-25T00:00:00Z",
    outcome: "ready",
    totalDurationMs: total,
    phases: [{ phase: "runtime_initialize", startedOffsetMs: 0, durationMs: phaseDuration, status: "completed" }],
    appVersion: "0.1.3",
  };
}

describe("startup benchmark formulas", () => {
  it("uses nearest-rank percentiles and keeps failures in the denominator", () => {
    const attempts = [attempt("cold", 10), attempt("cold", 20), attempt("cold", 30), {
      ...attempt("cold", 40),
      outcome: "failed" as const,
      totalDurationMs: undefined,
    }];
    const result = computeStartupCohortStats(attempts, "cold");
    expect(result.acceptedStarts).toBe(4);
    expect(result.readySamples).toBe(3);
    expect(result.failures).toBe(1);
    expect(result.failureRate).toBe(0.25);
    expect(result.total.p50).toBe(20);
    expect(result.total.p95).toBe(30);
  });

  it("requires both cohort medians, tail rules and a material bottleneck", () => {
    const baseline = {
      cold: computeStartupCohortStats([attempt("cold", 100, 40)], "cold"),
      warm: computeStartupCohortStats([attempt("warm", 100, 40)], "warm"),
    };
    const candidate = {
      cold: computeStartupCohortStats([attempt("cold", 70, 20)], "cold"),
      warm: computeStartupCohortStats([attempt("warm", 80, 20)], "warm"),
    };
    // The small fixture intentionally fails the >=29 Ready sample gate even
    // though its formula improvements are material.
    const result = compareStartupBenchmark(baseline, candidate);
    expect(result.coldMedian).toBe(true);
    expect(result.warmMedian).toBe(true);
    expect(result.materialBottleneck).toBe(true);
    expect(result.readySampleCount).toBe(false);
    expect(result.pass).toBe(false);
  });
});

