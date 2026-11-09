import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApprovalRequest, ApprovalStatus, BashApprovalDetails, RuntimeState } from "@protocol/v1";
import { i18n } from "@/i18n";
import { ApprovalsPage } from "./ApprovalsPage";

const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => state.context }));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  void i18n.changeLanguage("en");
});
beforeEach(() => { void i18n.changeLanguage("en"); });

function iso(offsetMs: number): string {
  return new Date(Date.now() + offsetMs).toISOString();
}

function runtime(runtimeId = "runtime_01"): RuntimeState {
  return { runtimeId } as RuntimeState;
}

function approval(status: ApprovalStatus = "pending", overrides: Partial<ApprovalRequest> = {}): ApprovalRequest {
  return {
    requestId: "request_01",
    runtimeId: "runtime_01",
    generationId: "generation_01",
    tool: "write",
    sanitizedArguments: { logicalPath: "notes.txt", content: "<redacted content>" },
    scopeSummary: "notes.txt",
    matchedRuleIds: ["review-writes"],
    policyReason: "matched_user_rules",
    riskLevel: "medium",
    createdAt: iso(-5_000),
    expiresAt: iso(300_000),
    status,
    ...(status === "approved" ? { decision: "allow_once" as const, resolvedAt: iso(0) } : {}),
    ...overrides,
  };
}

function bashApproval(overrides: Partial<ApprovalRequest> = {}): ApprovalRequest {
  const bashDetails: BashApprovalDetails = {
    commandPreview: "git status --short",
    classification: "classified",
    normalizedSegments: [{ executable: "git", argv: ["status", "--short"], argvRedacted: false }],
    riskFlags: [],
    cwd: "active_workspace",
    previewRedacted: false,
    previewTruncated: false,
    destructive: false,
  };
  return approval("pending", {
    requestId: "bash_request_01",
    tool: "bash",
    riskLevel: "high",
    scopeSummary: "git status --short",
    bashDetails,
    allowForSessionEligible: true,
    ...overrides,
  });
}

function setup(requests: ApprovalRequest[] = [approval()], overrides: Record<string, unknown> = {}) {
  const navigate = vi.fn();
  const approvalsResolve = vi.fn().mockImplementation(async (requestId: string, decision: ApprovalRequest["decision"]) => ({
    approval: { ...requests.find((item) => item.requestId === requestId)!, status: decision === "deny" ? "denied" : "approved", decision, resolvedAt: iso(0) },
  }));
  const approvalsList = vi.fn().mockResolvedValue({ items: requests });
  const runtimeStatus = vi.fn().mockResolvedValue(runtime(requests[0]?.runtimeId ?? "runtime_01"));
  const getHarnessStatus = vi.fn().mockReturnValue({ activeSessionId: "session_01" });
  const getComponentStates = vi.fn().mockReturnValue({ runtime: "ready", mcp: "ready", tunnel: "ready" });
  const refreshApprovals = vi.fn().mockResolvedValue(undefined);
  state.context = {
    approvals: { status: "success", data: { items: requests } },
    runtime: { status: "success", data: runtime(requests[0]?.runtimeId ?? "runtime_01") },
    harness: { activeSessionId: "session_01" },
    components: { runtime: "ready" },
    client: { approvalsResolve, approvalsList, runtimeStatus, getHarnessStatus, getComponentStates },
    refreshApprovals,
    ...overrides,
  };
  const rendered = render(<ApprovalsPage onNavigate={navigate} />);
  return { ...rendered, navigate, approvalsResolve, approvalsList, runtimeStatus, getHarnessStatus, getComponentStates, refreshApprovals };
}

describe("M8 T6 Approval queue", () => {
  it("keeps Pending decision-first and moves terminal requests into secondary History", () => {
    const terminal = approval("approved", { requestId: "request_done" });
    setup([approval(), terminal]);

    expect(screen.getByRole("heading", { name: "Pending" })).toBeInTheDocument();
    expect(screen.getByText(/1 pending request/)).toBeInTheDocument();
    expect(screen.getAllByText("notes.txt").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Risk/).length).toBeGreaterThan(0);
    expect(screen.getByText(/did not report a complete preview/)).toBeInTheDocument();
    expect(screen.getByText(/History · 1 historical request/)).toBeInTheDocument();
    expect(screen.queryByText("<redacted content>")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /write PENDING/ }));
    expect(screen.getByRole("heading", { name: /Request request_01/ })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Allow once" }).length).toBeGreaterThanOrEqual(2);
    fireEvent.click(screen.getByText("Advanced evidence"));
    expect(screen.getByText(/redacted content/)).toBeInTheDocument();
    expect(screen.getByText("generation_01")).toBeInTheDocument();
  });

  it("shows front-loaded risk, incomplete preview, active_workspace semantics and session scope", () => {
    const request = bashApproval({
      allowForSessionEligible: false,
      allowForSessionUnavailableReason: "unclassified_high_risk",
      bashDetails: {
        ...bashApproval().bashDetails!,
        classification: "unclassified_high_risk",
        previewRedacted: true,
        previewTruncated: true,
        normalizedSegments: [{ executable: "git", argv: [], argvRedacted: true }],
        riskFlags: ["command_substitution"],
      },
    });
    setup([request]);

    expect(screen.getAllByText(/Runtime could not classify this command/).length).toBeGreaterThan(0);
    expect(screen.getByText(/redacted and truncated/)).toBeInTheDocument();
    expect(screen.getByText("active_workspace")).toBeInTheDocument();
    expect(screen.getByText(/disabled for unclassified high-risk commands/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeDisabled();
  });

  it("shows destructive reasons and keeps exact session grants available for classified destructive commands", () => {
    const request = bashApproval({
      allowForSessionEligible: true,
      bashDetails: {
        ...bashApproval().bashDetails!,
        destructive: true,
        destructiveReasons: ["delete", "repository"],
      },
    });
    setup([request]);

    expect(screen.getAllByText(/Destructive operation/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/File or directory deletion/).length).toBeGreaterThan(0);
    expect(screen.getByText(/cannot be auto-allowed by a persistent Allow rule/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow for session" })).not.toBeDisabled();
  });

  it("puts the file-tool session scope in the decision surface without claiming content matching", () => {
    setup([approval()]);
    expect(screen.getByText(/Allow for session reuses the write tool within the Runtime-approved logical scope notes\.txt/)).toBeInTheDocument();
    expect(screen.getByText(/Different file contents may still be included/)).toBeInTheDocument();
  });

  it("does not invent a Runtime identity when the current observation is unknown", () => {
    setup([approval()], { runtime: { status: "idle" } });
    expect(screen.getByText("Runtime identity could not be confirmed, so this request cannot be decided.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeDisabled();
  });

  it("does not treat an invalid expiry value as an active approval window", () => {
    setup([approval("pending", { expiresAt: "not-a-timestamp" })]);
    expect(screen.getByText("Runtime did not report a valid expiry time, so this request cannot be decided.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeDisabled();
  });

  it("preflights the latest request and Runtime identity before resolving, then retains the authoritative DTO", async () => {
    const request = approval();
    const { approvalsResolve, approvalsList, runtimeStatus, refreshApprovals } = setup([request]);
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(approvalsResolve).toHaveBeenCalledWith("request_01", "allow_once"));
    expect(runtimeStatus).toHaveBeenCalled();
    expect(approvalsList).toHaveBeenCalled();
    expect(refreshApprovals).toHaveBeenCalled();
    expect(await screen.findByText(/Decision submitted to Runtime/)).toBeInTheDocument();
    expect(screen.getByText("APPROVED")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow once" })).not.toBeInTheDocument();
  });

  it("refreshes and pauses when the latest request metadata changes before resolve", async () => {
    const request = approval();
    const { approvalsResolve, approvalsList, refreshApprovals } = setup([request]);
    approvalsList.mockResolvedValue({ items: [{ ...request, scopeSummary: "other.txt" }] });
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(approvalsList).toHaveBeenCalled());
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(await screen.findByText(/belongs to a different Runtime identity/)).toBeInTheDocument();
    expect(refreshApprovals).toHaveBeenCalled();
  });

  it("refreshes and pauses when the latest deadline is expired before resolve", async () => {
    const request = approval();
    const { approvalsResolve, approvalsList, refreshApprovals } = setup([request]);
    approvalsList.mockResolvedValue({ items: [{ ...request, expiresAt: iso(-1_000) }] });
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(approvalsList).toHaveBeenCalled());
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect((await screen.findAllByText(/may have expired/)).length).toBeGreaterThan(0);
    expect(refreshApprovals).toHaveBeenCalled();
  });

  it("does not recompute session eligibility when the latest Runtime field changes", async () => {
    const request = bashApproval({ allowForSessionEligible: true });
    const { approvalsResolve, approvalsList } = setup([request]);
    approvalsList.mockResolvedValue({ items: [{ ...request, allowForSessionEligible: false, allowForSessionUnavailableReason: "unclassified_high_risk" }] });
    fireEvent.click(screen.getByRole("button", { name: "Allow for session" }));
    await waitFor(() => expect(approvalsList).toHaveBeenCalled());
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(await screen.findByText(/belongs to a different Runtime identity/)).toBeInTheDocument();
  });

  it("ignores a Runtime identity or Harness session change that arrives during preflight", async () => {
    const request = approval();
    const context = setup([request]);
    context.runtimeStatus.mockResolvedValueOnce(runtime("runtime_01")).mockResolvedValueOnce(runtime("runtime_02"));
    context.getHarnessStatus.mockReturnValueOnce({ activeSessionId: "session_01" }).mockReturnValueOnce({ activeSessionId: "session_02" });
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(context.approvalsList).toHaveBeenCalledTimes(2));
    expect(context.approvalsResolve).not.toHaveBeenCalled();
    expect(await screen.findByText(/belongs to a different Runtime identity/)).toBeInTheDocument();
  });

  it("locks only the request being resolved and does not issue a second resolve on double click", async () => {
    const request = approval();
    let release!: (value: { approval: ApprovalRequest }) => void;
    const pending = new Promise<{ approval: ApprovalRequest }>((resolve) => { release = resolve; });
    const context = setup([request]);
    context.approvalsResolve.mockReturnValueOnce(pending);
    const allow = screen.getByRole("button", { name: "Allow once" });
    fireEvent.click(allow);
    fireEvent.click(allow);
    expect(context.approvalsResolve).toHaveBeenCalledTimes(0);
    await waitFor(() => expect(context.runtimeStatus).toHaveBeenCalled());
    await waitFor(() => expect(context.approvalsResolve).toHaveBeenCalledTimes(1));
    release({ approval: { ...request, status: "approved", decision: "allow_once", resolvedAt: iso(0) } });
    await waitFor(() => expect(screen.getByText(/Decision submitted to Runtime/)).toBeInTheDocument());
  });

  it("keeps a confirmed decision in History when the follow-up refresh fails", async () => {
    const refreshApprovals = vi.fn().mockRejectedValue(new Error("refresh unavailable"));
    const context = setup([approval()], { refreshApprovals });
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(context.approvalsResolve).toHaveBeenCalledWith("request_01", "allow_once"));
    expect(await screen.findByText("APPROVED")).toBeInTheDocument();
    expect(screen.getByText(/Decision submitted to Runtime/)).toBeInTheDocument();
    expect(screen.queryByText(/^Execution succeeded/i)).not.toBeInTheDocument();
  });

  it("pauses a request when Runtime identity changed and never guesses from generation or Harness session IDs", () => {
    const request = approval();
    setup([request], { runtime: { status: "success", data: runtime("runtime_new") } });
    expect(screen.getByText(/belongs to a different Runtime identity/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeDisabled();
  });

  it("retains evidence and disables decisions while approvals are disconnected", () => {
    const mounted = setup([approval()]);
    state.context = {
      ...state.context,
      approvals: { status: "disconnected", error: { code: "RUNTIME_DISCONNECTED" } },
    };
    mounted.rerender(<ApprovalsPage />);
    expect(screen.getByText(/Approval data is stale/)).toBeInTheDocument();
    expect(screen.getAllByText("notes.txt").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Allow once" })).toBeDisabled();
    expect(screen.queryByText("No pending approvals were reported")).not.toBeInTheDocument();
  });

  it("checks an expired pending request once without writing a local terminal status", async () => {
    vi.useFakeTimers();
    const request = approval("pending", { expiresAt: iso(1_000) });
    const { refreshApprovals } = setup([request]);
    const initialCalls = refreshApprovals.mock.calls.length;
    await act(async () => { vi.advanceTimersByTime(1_100); await Promise.resolve(); });
    expect(refreshApprovals.mock.calls.length).toBe(initialCalls + 1);
    expect(screen.getByText(/may have expired/)).toBeInTheDocument();
    expect(screen.getByText("PENDING")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeDisabled();
    await act(async () => { vi.advanceTimersByTime(5_000); await Promise.resolve(); });
    expect(refreshApprovals.mock.calls.length).toBe(initialCalls + 1);
  });

  it("uses safe matcher handoff and keeps unsafe Bash evidence explicit", () => {
    const safe = bashApproval();
    const { navigate, approvalsResolve } = setup([safe]);
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith("policies", expect.objectContaining({
      policyDraft: expect.objectContaining({ approvalRequestId: "bash_request_01", matcher: { type: "bash_command", executable: "git", argvPrefix: ["status", "--short"] } }),
    }));

    const unsafe = bashApproval({ requestId: "bash_unsafe", bashDetails: { ...safe.bashDetails!, normalizedSegments: [safe.bashDetails!.normalizedSegments[0], { executable: "echo", argv: ["x"], argvRedacted: false }] } });
    cleanup();
    const unsafeSetup = setup([unsafe]);
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(unsafeSetup.navigate).toHaveBeenCalledWith("policies", expect.objectContaining({ policyDraft: expect.not.objectContaining({ matcher: { type: "tool", tool: "bash" } }) }));
    expect(screen.getByText(/no Bash command matcher was prefilled/)).toBeInTheDocument();
  });

  it("uses a broader explicit whole-tool draft for file approvals", () => {
    const { navigate } = setup([approval()]);
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    const draft = navigate.mock.calls[0][1].policyDraft;
    expect(draft.approvalRequestId).toBe("request_01");
    expect(draft.matcher).toEqual({ type: "tool", tool: "write" });
    expect(draft.reason).toMatch(/broader than this path/);
  });

  it("reports resolution failures locally and refreshes instead of claiming approval", async () => {
    const request = approval();
    const refreshApprovals = vi.fn().mockResolvedValue(undefined);
    const context = setup([request], { refreshApprovals });
    context.approvalsResolve.mockRejectedValueOnce({ code: "APPROVAL_ALREADY_RESOLVED" });
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    expect(await screen.findByText("This approval is already resolved or expired.")).toBeInTheDocument();
    expect(screen.getByText("Refresh the Approval Center and try again.")).toBeInTheDocument();
    expect(screen.queryByText("Decision submitted to Runtime")).not.toBeInTheDocument();
    expect(refreshApprovals).toHaveBeenCalled();
  });

  it("switches language immediately while preserving request identity and action semantics", async () => {
    setup([approval()]);
    expect(screen.getByText("Pending")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /write PENDING/ }));
    expect(screen.getByText("request_01")).toBeInTheDocument();
    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getAllByText("待处理").length).toBeGreaterThan(0);
    expect(screen.getByText("request_01")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "仅允许本次" }).some((button) => !(button as HTMLButtonElement).disabled)).toBe(true);
    expect(screen.getAllByRole("button", { name: "本次会话允许" }).some((button) => !(button as HTMLButtonElement).disabled)).toBe(true);
    expect(screen.getAllByRole("button", { name: "拒绝" }).some((button) => !(button as HTMLButtonElement).disabled)).toBe(true);
  });

  it("opens the request supplied by Approval return context without resolving it", () => {
    const { approvalsResolve } = setup([approval()], {});
    cleanup();
    render(<ApprovalsPage initialRequestId="request_01" />);
    expect(screen.getByRole("heading", { name: /Request request_01/ })).toBeInTheDocument();
    expect(approvalsResolve).not.toHaveBeenCalled();
  });
});
