import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ActivityEntry } from "@/services/harness/control-read";
import type { ApprovalRequest, PolicyRule } from "@protocol/v1";
import { i18n, initializeLanguage } from "@/i18n";
import { ActivityPage } from "./ActivityPage";

const harness = vi.hoisted(() => ({ current: undefined as any }));
const control = vi.hoisted(() => ({ state: { status: "loading" } as any, refresh: vi.fn() }));

vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => harness.current }));
vi.mock("@/app/providers/useControlRead", () => ({ useControlRead: () => control }));

function entry(overrides: Partial<ActivityEntry> = {}): ActivityEntry {
  return {
    eventId: "event_01",
    requestId: "request_01",
    timestamp: "2026-10-05T00:00:00.000Z",
    category: "execution",
    eventType: "execution_completed",
    lifecycleState: "completed",
    toolName: "read_file",
    decisionSummary: "allow",
    matchedRuleIds: [],
    policyReason: null,
    outcomeSummary: "success",
    ...overrides,
  };
}

const rule = (): PolicyRule => ({ id: "rule-1", name: "Read notes", decision: "allow", scope: "tool:read_file", reason: "review", source: "user", editable: true, enabled: true });
const approval = (): ApprovalRequest => ({
  requestId: "approval-1", runtimeId: "runtime-1", generationId: "generation-1", tool: "read_file",
  sanitizedArguments: { logicalPath: "notes.txt" }, scopeSummary: "notes.txt", matchedRuleIds: [], policyReason: "tool_default", riskLevel: "low",
  createdAt: "2026-10-05T00:00:00.000Z", expiresAt: "2026-10-05T01:00:00.000Z", status: "pending",
});

function setup(entries: ActivityEntry[], options: { rules?: PolicyRule[]; effectiveRules?: PolicyRule[]; approvals?: ApprovalRequest[] } = {}) {
  const navigate = vi.fn();
  const historyQuery = vi.fn().mockResolvedValue({
    items: [],
    storeStatus: {
      state: "healthy",
      schemaVersion: 1,
      queueDepth: 0,
      queueCapacity: 1024,
      dropCount: 0,
      effectiveHistoryConfig: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
    },
  });
  harness.current = {
    client: { runtimeGetActivitySnapshot: vi.fn(), historyQuery },
    config: { policies: { userRules: (options.rules ?? []).map((item) => ({ id: item.id, name: item.name, enabled: item.enabled, decision: item.decision, reason: item.reason, matcher: { type: "tool", tool: "read_file" } })) } },
    policies: { status: "success", data: { items: options.effectiveRules ?? options.rules ?? [] } },
    approvals: { status: "success", data: { items: options.approvals ?? [] } },
  };
  control.state = { status: "success", data: entries };
  control.refresh.mockReset();
  const rendered = render(<ActivityPage onNavigate={navigate} />);
  return { ...rendered, navigate, historyQuery };
}

beforeEach(async () => {
  await initializeLanguage("en");
});

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

describe("M8 T7 ActivityPage", () => {
  it("labels the default view as a snapshot and switches to the durable History tab", async () => {
    const view = setup([entry({ eventId: "live-default" })]);
    await waitFor(() => expect(screen.getByTestId("activity-row-event:live-default")).toBeInTheDocument());
    expect(screen.getByRole("radio", { name: "Snapshot" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("radio", { name: "History" }));
    await waitFor(() => expect(view.historyQuery).toHaveBeenCalledOnce());
    expect(screen.getByText("No retained History matches these filters")).toBeInTheDocument();
  });

  it("renders a compact feed with distinct reported outcomes, local filters, and non-tool events", async () => {
    setup([
      entry({ eventId: "success", category: "request", eventType: "request_received", lifecycleState: "received", toolName: "read_file" }),
      entry({ eventId: "policy", outcomeSummary: "denied", decisionSummary: "deny", policyReason: "matched_user_rules" }),
      entry({ eventId: "approval", outcomeSummary: "denied", decisionSummary: "ask", policyReason: null, failureCategory: "approval_denied", approvalDecision: "deny" }),
      entry({ eventId: "failure", outcomeSummary: "failed", decisionSummary: "allow", failureCategory: "execution_failed" }),
      entry({ eventId: "unavailable", outcomeSummary: "failed", failureCategory: "bash_unavailable", toolName: "bash" }),
      entry({ eventId: "pending", outcomeSummary: "approval_required", lifecycleState: "awaiting_approval" }),
      entry({ eventId: "workspace", category: "workspace", eventType: "workspace_validated", toolName: null }),
    ]);
    await waitFor(() => expect(screen.getByText("Policy denied")).toBeInTheDocument());
    expect(screen.getByText("User approval denied")).toBeInTheDocument();
    expect(screen.getByText("Allowed execution failed")).toBeInTheDocument();
    expect(screen.getByText("Tool unavailable")).toBeInTheDocument();
    expect(screen.getByText("Pending")).toBeInTheDocument();
    expect(screen.getAllByText("Non-tool event").length).toBeGreaterThan(0);
    expect(screen.getByText("Filters apply only to the currently loaded bounded snapshot (up to 100 records).")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("radio", { name: /^Needs attention$/ }));
    expect(screen.queryByTestId("activity-row-event:success")).not.toBeInTheDocument();
    expect(screen.getByText("Allowed execution failed")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Tool" }), { target: { value: "bash" } });
    expect(screen.getByText("Tool unavailable")).toBeInTheDocument();
    expect(screen.queryByText("Allowed execution failed")).not.toBeInTheDocument();
  });

  it("opens the Inspector with human facts first and keeps complete safe evidence under Advanced", async () => {
    setup([entry({ eventId: "write-event", toolName: "write", sanitizedArguments: { tool: "write", logicalPath: "notes.txt" }, resultEvidence: { tool: "write", bytesWritten: 2, disposition: "created" } })]);
    await waitFor(() => expect(screen.getByTestId("activity-row-event:write-event")).toBeInTheDocument());
    fireEvent.click(within(screen.getByTestId("activity-row-event:write-event")).getByRole("button"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("What happened")).toBeInTheDocument();
    expect(screen.getByText("Why")).toBeInTheDocument();
    expect(screen.getByText("Result")).toBeInTheDocument();
    expect(screen.getByText("Next step")).toBeInTheDocument();
    expect(screen.queryByText(/0 ms/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByText("Advanced evidence"));
    expect(screen.getAllByText(/write-event/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/notes\.txt/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/bytesWritten/).length).toBeGreaterThan(0);
  });

  it("retains the previous snapshot and marks it stale when refresh fails", async () => {
    const view = setup([entry({ eventId: "retained", toolName: "read_file" })]);
    await waitFor(() => expect(screen.getByTestId("activity-row-event:retained")).toBeInTheDocument());
    control.state = { status: "loading" };
    view.rerender(<ActivityPage />);
    expect(screen.getByText("Refreshing activity…")).toBeInTheDocument();
    expect(screen.getByTestId("activity-row-event:retained")).toBeInTheDocument();
    control.state = { status: "error", error: { code: "RUNTIME_NOT_READY" } };
    view.rerender(<ActivityPage />);
    await waitFor(() => expect(screen.getByText("Activity snapshot is stale.")).toBeInTheDocument());
    expect(screen.getByTestId("activity-row-event:retained")).toBeInTheDocument();
    expect(screen.queryByText("No retained activity in this snapshot")).not.toBeInTheDocument();
  });

  it("keeps a stable event selected across refresh and does not relink historical IDs without current records", async () => {
    const view = setup([entry({ eventId: "stable-event", matchedRuleIds: ["rule-1"], approvalId: "approval-1" })], { rules: [rule()], effectiveRules: [], approvals: [approval()] });
    await waitFor(() => expect(screen.getByTestId("activity-row-event:stable-event")).toBeInTheDocument());
    fireEvent.click(within(screen.getByTestId("activity-row-event:stable-event")).getByRole("button"));
    expect(screen.getByRole("button", { name: "Open current rule rule-1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open current approval approval-1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open current rule rule-1" }));
    expect(view.navigate).toHaveBeenCalledWith("policies", { objectId: "rule-1" });

    control.state = { status: "loading" };
    view.rerender(<ActivityPage onNavigate={view.navigate} />);
    control.state = { status: "success", data: [entry({ eventId: "stable-event", matchedRuleIds: ["rule-1"], approvalId: "approval-1" })] };
    view.rerender(<ActivityPage onNavigate={view.navigate} />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Open current rule rule-1" })).toBeInTheDocument());
    expect(screen.getByText(/historical evidence/)).toBeInTheDocument();
  });

  it("switches the activity copy immediately when the language changes", async () => {
    setup([entry({ eventId: "language-event" })]);
    await waitFor(() => expect(screen.getByRole("radio", { name: /^All$/ })).toBeInTheDocument());
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByRole("radio", { name: /^全部$/ })).toBeInTheDocument();
    expect(screen.getByText("需要关注")).toBeInTheDocument();
    expect(screen.getByText("活动记录列表")).toBeInTheDocument();
  });

  it("exposes list semantics and supports keyboard inspection and dismissal", async () => {
    setup([entry({ eventId: "keyboard-one" }), entry({ eventId: "keyboard-two", toolName: "glob" })]);
    const row = await screen.findByTestId("activity-row-event:keyboard-one");
    const list = screen.getByRole("list", { name: "Activity Feed" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);

    const button = within(row).getByRole("button");
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.keyDown(button, { key: "Enter" });
    const dialog = await screen.findByRole("dialog", { name: "Activity details" });
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(button).toHaveFocus();

    fireEvent.keyDown(button, { key: " " });
    expect(await screen.findByRole("dialog", { name: "Activity details" })).toBeInTheDocument();
  });
});
