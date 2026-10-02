import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ApprovalRequest } from "@protocol/v1";
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

describe("M4 Approval Center", () => {
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
    expect(await screen.findByText("This approval is already resolved or expired. Refresh the Approval Center.")).toBeInTheDocument();
    expect(refreshApprovals).toHaveBeenCalled();
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
});
