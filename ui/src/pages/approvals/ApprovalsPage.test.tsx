import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ApprovalRequest, BashApprovalDetails } from "@protocol/v1";
import { i18n } from "@/i18n";
import { ApprovalsPage } from "./ApprovalsPage";

const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => state.context }));
afterEach(() => cleanup());

function approval(status: ApprovalRequest["status"] = "pending"): ApprovalRequest {
  return {
    requestId: "request_01",
    runtimeId: "runtime_01",
    generationId: "generation_01",
    tool: "write",
    sanitizedArguments: { path: "notes.txt", content: "<redacted content>" },
    scopeSummary: "notes.txt",
    matchedRuleIds: ["review-writes"],
    policyReason: "matched_user_rules",
    riskLevel: "medium",
    createdAt: "2026-10-02T00:00:00Z",
    expiresAt: "2026-10-02T00:05:00Z",
    status,
    decision: status === "approved" ? "allow_once" : undefined,
  };
}

function bashApproval(classification: BashApprovalDetails["classification"]): ApprovalRequest {
  return {
    requestId: "bash_request_01",
    runtimeId: "runtime_01",
    generationId: "generation_01",
    tool: "bash",
    sanitizedArguments: { command: "git status --short" },
    scopeSummary: "git status --short",
    matchedRuleIds: ["bash-review"],
    policyReason: "matched_user_rules",
    riskLevel: "high",
    bashDetails: {
      commandPreview: classification === "classified" ? "git status --short" : "git $(dynamic) --short",
      classification,
      normalizedSegments: [{ executable: "git", argv: classification === "classified" ? ["status", "--short"] : [], argvRedacted: classification !== "classified" }],
      riskFlags: classification === "classified" ? [] : ["command_substitution"],
      cwd: "active_workspace",
      previewRedacted: false,
      previewTruncated: false,
    },
    allowForSessionEligible: classification === "classified",
    allowForSessionUnavailableReason: classification === "classified" ? undefined : "unclassified_high_risk",
    createdAt: "2026-10-02T00:00:00Z",
    expiresAt: "2026-10-02T00:05:00Z",
    status: "pending",
  };
}

function setup(request = approval()) {
  const approvalsResolve = vi.fn().mockResolvedValue({ approval: request });
  const refreshApprovals = vi.fn().mockResolvedValue(undefined);
  const navigate = vi.fn();
  state.context = {
    approvals: { status: "success", data: { items: [request] } },
    components: { runtime: "ready" },
    client: { approvalsResolve },
    refreshApprovals,
  };
  render(<ApprovalsPage onNavigate={navigate} />);
  return { approvalsResolve, refreshApprovals, navigate };
}

describe("M6 Approval Center", () => {
  it("shows pending evidence and all resolution actions", () => {
    setup();
    expect(screen.getByText("Approval Center")).toBeInTheDocument();
    expect(screen.getAllByText("notes.txt").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("<redacted content>")).toBeInTheDocument();
    expect(screen.getByText("review-writes")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Deny" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Create rule" })).toBeEnabled();
  });

  it("resolves through Runtime and refreshes truth instead of faking success", async () => {
    const { approvalsResolve, refreshApprovals } = setup();
    for (const [label, decision] of [
      ["Allow once", "allow_once"],
      ["Allow for session", "allow_for_session"],
      ["Deny", "deny"],
    ] as const) {
      fireEvent.click(screen.getByRole("button", { name: label }));
      await waitFor(() => expect(approvalsResolve).toHaveBeenLastCalledWith("request_01", decision));
    }
    expect(refreshApprovals).toHaveBeenCalled();
  });

  it("explains stale resolve failures and refreshes Runtime truth", async () => {
    const approvalsResolve = vi.fn().mockRejectedValue({ code: "APPROVAL_ALREADY_RESOLVED" });
    const refreshApprovals = vi.fn().mockResolvedValue(undefined);
    state.context = {
      approvals: { status: "success", data: { items: [approval()] } },
      components: { runtime: "ready" },
      client: { approvalsResolve },
      refreshApprovals,
    };
    render(<ApprovalsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    expect(await screen.findByText("This approval is already resolved or expired.")).toBeInTheDocument();
    expect(screen.getByText("Refresh the Approval Center and try again.")).toBeInTheDocument();
    expect(screen.getByText("APPROVAL_ALREADY_RESOLVED")).toBeInTheDocument();
    expect(refreshApprovals).toHaveBeenCalled();

    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("该审批已处理或已过期。")).toBeInTheDocument();
    expect(screen.getByText("请刷新审批中心后重试。")).toBeInTheDocument();
    expect(screen.getByText("APPROVAL_ALREADY_RESOLVED")).toBeInTheDocument();
  });

  it("Create rule navigates to a broader explicit policy draft without resolving", () => {
    const { approvalsResolve, navigate } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith("policies", {
      policyDraft: {
        tool: "write",
        scope: "notes.txt",
        name: "Review write approvals",
        reason: "Review requests for write before allowing execution.",
      },
    });
  });

  it("makes terminal approvals read-only", () => {
    setup(approval("expired"));
    expect(screen.getByText("EXPIRED")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create rule" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Allow once" })).not.toBeInTheDocument();
  });

  it("shows classified Bash evidence and creates an exact command matcher draft", () => {
    const request = bashApproval("classified");
    const { navigate, approvalsResolve } = setup(request);
    expect(screen.getAllByText("git status --short").length).toBeGreaterThan(0);
    expect(screen.getByText("active_workspace")).toBeInTheDocument();
    expect(screen.getByText("Allow for session")).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith("policies", {
      policyDraft: expect.objectContaining({
        tool: "bash",
        matcher: { type: "bash_command", executable: "git", argvPrefix: ["status", "--short"] },
      }),
    });
  });

  it("keeps Runtime-eligible session approval enabled when Bash display facts are redacted or truncated", () => {
    const base = bashApproval("classified");
    const request: ApprovalRequest = {
      ...base,
      bashDetails: {
        ...base.bashDetails!,
        previewRedacted: true,
        previewTruncated: true,
      },
      allowForSessionEligible: true,
    };
    const { navigate, approvalsResolve } = setup(request);
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeEnabled();
    expect(screen.getByText(/Display facts are redacted and truncated/)).toBeInTheDocument();
    expect(screen.queryByText(/reusable command scope is unavailable/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(approvalsResolve).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith("policies", {
      policyDraft: expect.objectContaining({
        tool: "bash",
        matcher: { type: "tool", tool: "bash" },
      }),
    });
  });

  it("keeps Allow for session disabled for unclassified Bash", () => {
    const request = bashApproval("unclassified_high_risk");
    const { navigate } = setup(request);
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeDisabled();
    expect(screen.getByText(/disabled for unclassified high-risk commands/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create rule" }));
    expect(navigate).toHaveBeenCalledWith("policies", {
      policyDraft: expect.objectContaining({
        tool: "bash",
        matcher: { type: "tool", tool: "bash" },
      }),
    });
  });

  it("switches presentation live in both locales while preserving approval identity and actions", async () => {
    const { approvalsResolve, refreshApprovals } = setup();
    await waitFor(() => expect(refreshApprovals).toHaveBeenCalledTimes(1));
    const refreshCallsBeforeSwitch = refreshApprovals.mock.calls.length;

    expect(screen.getByText("Approval Center")).toBeInTheDocument();
    expect(screen.getByText("PENDING")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeEnabled();
    expect(screen.getByText(/request_01/)).toBeInTheDocument();
    expect(screen.getByText("write", { exact: true })).toBeInTheDocument();

    await act(async () => { await i18n.changeLanguage("zh-CN"); });

    expect(screen.getByText("审批中心")).toBeInTheDocument();
    expect(screen.getByText("待处理")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "仅允许本次" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "本次会话允许" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "拒绝" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "创建规则" })).toBeEnabled();
    expect(screen.getByText(/request_01/)).toBeInTheDocument();
    expect(screen.getByText("write", { exact: true })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(refreshApprovals).toHaveBeenCalledTimes(refreshCallsBeforeSwitch);
    expect(approvalsResolve).not.toHaveBeenCalled();

    await act(async () => { await i18n.changeLanguage("en"); });
    expect(screen.getByText("Approval Center")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });
});
