import type {
  ActivityEvent,
  ActivityListParams,
  ActivityListResult,
  ActivitySubscribeParams,
  ActivitySubscribeResult,
  ActivityEventType,
  ApprovalDecision,
  ApprovalRequest,
  ApprovalsListParams,
  ApprovalsListResult,
  ApprovalsResolveResult,
  DiagnosticRun,
  PoliciesListResult,
  PolicyDecision,
  PolicyRule,
  RuntimeState,
  RuntimeStopResult,
  SecurityMode,
  ToolDescriptor,
  ToolsListResult,
} from "@protocol/v1";
import { HarnessClientError } from "./errors";
import type { ActivityEventListener, HarnessControlClient, Unsubscribe } from "./client";

type MockScenario = "ready" | "disconnected" | "empty" | "error";

const delay = (milliseconds = 120) => new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

function event(
  sequence: bigint,
  eventType: ActivityEventType,
  summary: string,
  metadata: Record<string, string>,
  requestId?: string,
): ActivityEvent {
  return {
    eventId: `event_mock_${sequence.toString()}`,
    sequence,
    eventType,
    requestId,
    timestamp: `2026-01-01T00:0${sequence.toString()}:00Z`,
    summary,
    metadata,
  };
}

const tools: ToolDescriptor[] = [
  {
    name: "read_file",
    description: "Read a workspace file.",
    implemented: true,
    enabled: true,
    exposed: true,
    effectivePolicy: "allow",
    scope: "active_workspace",
    readiness: "ready",
    riskLevel: "low",
  },
  {
    name: "glob",
    description: "Find workspace paths.",
    implemented: true,
    enabled: true,
    exposed: true,
    effectivePolicy: "allow",
    scope: "active_workspace",
    readiness: "ready",
    riskLevel: "low",
  },
  {
    name: "bash",
    description: "Developer test command execution.",
    implemented: true,
    enabled: false,
    exposed: false,
    effectivePolicy: "ask",
    scope: "active_workspace",
    readiness: "unavailable",
    riskLevel: "high",
  },
];

const policies: PolicyRule[] = [
  {
    id: "system-read-workspace",
    name: "Allow workspace reads",
    decision: "allow",
    scope: "active_workspace",
    reason: "Read-only default capability.",
    source: "system",
    editable: false,
    enabled: true,
  },
  {
    id: "user-review-sensitive",
    name: "Review sensitive requests",
    decision: "ask",
    scope: "developer_test_capabilities",
    reason: "Requires explicit approval before a high-risk request.",
    source: "user",
    editable: false,
    enabled: true,
  },
  {
    id: "system-deny-outside",
    name: "Deny outside workspace",
    decision: "deny",
    scope: "outside_active_workspace",
    reason: "Workspace boundary.",
    source: "system",
    editable: false,
    enabled: true,
  },
];

const activities: ActivityEvent[] = [
  event(1n, "runtime_started", "Runtime started in mock mode.", { source: "mock-runtime" }),
  event(2n, "policy_evaluated", "Workspace read policy evaluated.", { decision: "allow" }, "request_mock_01"),
  event(3n, "tool_requested", "ChatGPT requested read_file.", { tool: "read_file" }, "request_mock_01"),
];

const approvals: ApprovalRequest[] = [
  {
    requestId: "approval_mock_01",
    tool: "bash",
    sanitizedArguments: { command: "<redacted command>" },
    createdAt: "2026-01-01T00:03:00Z",
    expiresAt: "2026-01-01T00:08:00Z",
    status: "pending",
  },
];

const runtime: RuntimeState = {
  runtimeId: "runtime_mock_01",
  startedAt: "2026-01-01T00:00:00Z",
  lifecycle: "ready",
  readiness: "ready",
  configRevision: 7n,
  activeWorkspace: "D:\\example-project",
  securityMode: "normal",
  exposedToolCatalog: tools.filter((tool) => tool.exposed),
  activeRequests: 0,
  pendingApprovals: 1,
  runningExecutions: 0,
};

export class MockHarnessControlClient implements HarnessControlClient {
  private readonly scenario: MockScenario;
  private nextSequence = 4n;

  constructor(scenario: MockScenario = "ready") {
    this.scenario = scenario;
  }

  private async respond<T>(value: T): Promise<T> {
    await delay();
    if (this.scenario === "disconnected") {
      throw new HarnessClientError("disconnected", "The local Runtime is not connected.");
    }
    if (this.scenario === "error") {
      throw new HarnessClientError("error", "The mock Runtime returned an example error.");
    }
    return value;
  }

  async runtimeStart(): Promise<RuntimeState> {
    return this.respond(runtime);
  }

  async runtimeStatus(): Promise<RuntimeState> {
    return this.respond(runtime);
  }

  async runtimeStop(): Promise<RuntimeStopResult> {
    return this.respond({ accepted: false });
  }

  async toolsList(): Promise<ToolsListResult> {
    return this.respond({ items: this.scenario === "empty" ? [] : tools });
  }

  async policiesList(): Promise<PoliciesListResult> {
    return this.respond({ items: this.scenario === "empty" ? [] : policies });
  }

  async activityList(_params?: ActivityListParams): Promise<ActivityListResult> {
    return this.respond({
      items: this.scenario === "empty" ? [] : activities,
      latestSequence: this.scenario === "empty" ? 0n : 3n,
    });
  }

  async activitySubscribe(
    listener: ActivityEventListener,
    _params?: ActivitySubscribeParams,
  ): Promise<Unsubscribe> {
    await this.respond<ActivitySubscribeResult>({ latestSequence: this.scenario === "empty" ? 0n : 3n });
    let active = true;
    const timer = window.setTimeout(() => {
      if (!active || this.scenario !== "ready") return;
      const next = event(
        this.nextSequence,
        "execution_completed",
        "Mock activity stream received a completion event.",
        { source: "mock-subscription" },
        "request_mock_02",
      );
      this.nextSequence += 1n;
      listener(next);
    }, 220);

    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }

  async approvalsList(_params?: ApprovalsListParams): Promise<ApprovalsListResult> {
    return this.respond({ items: this.scenario === "empty" ? [] : approvals });
  }

  async approvalsResolve(
    _requestId: string,
    _decision: ApprovalDecision,
  ): Promise<ApprovalsResolveResult> {
    throw new HarnessClientError("error", "Approval actions are not enabled until M8.");
  }

  async diagnosticsRun(): Promise<DiagnosticRun> {
    throw new HarnessClientError("error", "Diagnostics are not enabled until M2.");
  }
}

export type { MockScenario };
