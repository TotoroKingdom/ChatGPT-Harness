import type { StartupAttempt, StartupPhase } from "@protocol/v1";

export type PhaseStats = {
  p50: number | null;
  p95: number | null;
};

export type StartupCohortStats = {
  cohort: "cold" | "warm";
  acceptedStarts: number;
  readySamples: number;
  failures: number;
  failureRate: number;
  total: PhaseStats;
  phases: Partial<Record<StartupPhase, PhaseStats>>;
  slowestPhase: StartupPhase | null;
};

export type StartupBenchmark = {
  cold: StartupCohortStats;
  warm: StartupCohortStats;
};

export type StartupThresholdResult = {
  coldMedian: boolean;
  warmMedian: boolean;
  coldTailGuard: boolean;
  warmTailGuard: boolean;
  tailImprovement: boolean;
  failureRateGuard: boolean;
  readySampleCount: boolean;
  materialBottleneck: boolean;
  pass: boolean;
};

const hostControllablePhases: readonly StartupPhase[] = [
  "eligibility_prepare",
  "runtime_spawn",
  "runtime_initialize",
  "mcp_endpoint_ready",
  "tunnel_resolve",
  "tunnel_launch",
  "harness_ready",
];

function nearestRank(values: number[], percentile: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const rank = Math.max(1, Math.ceil(percentile * sorted.length));
  return sorted[rank - 1];
}

function stats(values: number[]): PhaseStats {
  return { p50: nearestRank(values, 0.5), p95: nearestRank(values, 0.95) };
}

function readyAttempts(attempts: StartupAttempt[]): StartupAttempt[] {
  return attempts.filter((attempt) => attempt.outcome === "ready" && attempt.totalDurationMs !== undefined);
}

export function computeStartupCohortStats(
  attempts: readonly StartupAttempt[],
  cohort: "cold" | "warm",
): StartupCohortStats {
  const counted = attempts.filter((attempt) => attempt.cohort === cohort);
  const ready = readyAttempts(counted);
  const phases = {} as Partial<Record<StartupPhase, PhaseStats>>;
  for (const phase of new Set(ready.flatMap((attempt) => attempt.phases.map((timing) => timing.phase)))) {
    phases[phase] = stats(
      ready.flatMap((attempt) => attempt.phases
        .filter((timing) => timing.phase === phase && timing.status === "completed")
        .map((timing) => timing.durationMs)),
    );
  }
  const slowestPhase = hostControllablePhases
    .filter((phase) => phases[phase]?.p50 !== null && phases[phase]?.p50 !== undefined)
    .sort((left, right) => (phases[right]?.p50 ?? 0) - (phases[left]?.p50 ?? 0))[0] ?? null;
  const failures = counted.length - ready.length;
  return {
    cohort,
    acceptedStarts: counted.length,
    readySamples: ready.length,
    failures,
    failureRate: counted.length === 0 ? 0 : failures / counted.length,
    total: stats(ready.map((attempt) => attempt.totalDurationMs!)),
    phases,
    slowestPhase,
  };
}

export function computeStartupBenchmark(attempts: readonly StartupAttempt[]): StartupBenchmark {
  return {
    cold: computeStartupCohortStats(attempts, "cold"),
    warm: computeStartupCohortStats(attempts, "warm"),
  };
}

function atMost(candidate: number | null, baseline: number | null, multiplier: number): boolean {
  return candidate !== null && baseline !== null && candidate <= baseline * multiplier;
}

function materialBottleneck(baseline: StartupCohortStats, candidate: StartupCohortStats): boolean {
  const phase = baseline.slowestPhase;
  const baselineTotal = baseline.total.p50;
  const baselinePhase = phase ? baseline.phases[phase]?.p50 ?? null : null;
  const candidatePhase = phase ? candidate.phases[phase]?.p50 ?? null : null;
  const criterionA = baselineTotal !== null
    && baselinePhase !== null
    && candidatePhase !== null
    && baselinePhase / baselineTotal >= 0.1
    && candidatePhase <= baselinePhase * 0.75;
  const criterionB = atMost(candidate.total.p95, baseline.total.p95, 0.9);
  return criterionA || criterionB;
}

export function compareStartupBenchmark(
  baseline: StartupBenchmark,
  candidate: StartupBenchmark,
): StartupThresholdResult {
  const coldMedian = atMost(candidate.cold.total.p50, baseline.cold.total.p50, 0.9);
  const warmMedian = atMost(candidate.warm.total.p50, baseline.warm.total.p50, 0.85);
  const coldTailGuard = atMost(candidate.cold.total.p95, baseline.cold.total.p95, 1.05);
  const warmTailGuard = atMost(candidate.warm.total.p95, baseline.warm.total.p95, 1.05);
  const tailImprovement = atMost(candidate.cold.total.p95, baseline.cold.total.p95, 0.9)
    || atMost(candidate.warm.total.p95, baseline.warm.total.p95, 0.9);
  const failureRateGuard = candidate.cold.failureRate <= baseline.cold.failureRate
    && candidate.warm.failureRate <= baseline.warm.failureRate
    && candidate.cold.failureRate <= 1 / 30
    && candidate.warm.failureRate <= 1 / 30;
  const readySampleCount = candidate.cold.readySamples >= 29 && candidate.warm.readySamples >= 29;
  const material = materialBottleneck(baseline.cold, candidate.cold)
    || materialBottleneck(baseline.warm, candidate.warm);
  return {
    coldMedian,
    warmMedian,
    coldTailGuard,
    warmTailGuard,
    tailImprovement,
    failureRateGuard,
    readySampleCount,
    materialBottleneck: material,
    pass: coldMedian && warmMedian && coldTailGuard && warmTailGuard && tailImprovement
      && failureRateGuard && readySampleCount && material,
  };
}

