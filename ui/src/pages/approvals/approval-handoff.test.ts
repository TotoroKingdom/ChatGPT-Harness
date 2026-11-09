import { expect, it } from "vitest";
import type { ApprovalRequest, BashApprovalDetails } from "@protocol/v1";
import { safeApprovalMatcher } from "./approval-handoff";

function request(patch: Partial<BashApprovalDetails> = {}): ApprovalRequest {
  return { requestId: "a1", runtimeId: "runtime1", generationId: "generation1", tool: "bash", sanitizedArguments: {}, scopeSummary: "git status", matchedRuleIds: [], policyReason: "tool_default", riskLevel: "low", createdAt: "2026-10-05T00:00:00Z", expiresAt: "2026-10-05T01:00:00Z", status: "pending", allowForSessionEligible: true, bashDetails: { commandPreview: "git status", classification: "classified", normalizedSegments: [{ executable: "git", argv: ["status"], argvRedacted: false }], riskFlags: [], cwd: "active_workspace", previewRedacted: false, previewTruncated: false, destructive: false, ...patch } };
}
it("prefills only an unambiguous single segment and preserves literal tokens", () => {
  const source = request({ normalizedSegments: [{ executable: "printf", argv: ["two words", ""], argvRedacted: false }] });
  const before = JSON.stringify(source);
  expect(safeApprovalMatcher(source)).toEqual({ type: "bash_command", executable: "printf", argvPrefix: ["two words", ""] });
  expect(JSON.stringify(source)).toBe(before);
});
it.each([
  { previewRedacted: true }, { previewTruncated: true }, { classification: "unclassified_high_risk" as const },
  { destructive: true },
  { previewRedacted: undefined }, { previewTruncated: undefined },
  { normalizedSegments: [] },
  { normalizedSegments: [{ executable: "git", argv: ["status"], argvRedacted: false }, { executable: "git", argv: ["push"], argvRedacted: false }] },
  { normalizedSegments: [{ executable: "git", argv: ["<redacted>"], argvRedacted: true }] },
  { normalizedSegments: [{ executable: "git", argv: ["\0"], argvRedacted: false }] },
  { normalizedSegments: [{ executable: "git", argv: Array(65).fill("arg"), argvRedacted: false }] },
  { normalizedSegments: [{ executable: "git", argv: ["é".repeat(513)], argvRedacted: false }] },
  { normalizedSegments: [{ executable: "git && pwd", argv: [], argvRedacted: false }] },
])("does not broaden unsupported evidence to an entire Bash tool: %j", (patch) => {
  expect(safeApprovalMatcher(request(patch))).toBeUndefined();
});
it("does not use preview redaction to recompute Runtime session eligibility", () => {
  const source = request({ previewRedacted: true });
  expect(safeApprovalMatcher(source)).toBeUndefined();
  expect(source.allowForSessionEligible).toBe(true);
});
it("makes the existing file-tool whole-tool scope explicit and refuses unknown tool scopes", () => {
  expect(safeApprovalMatcher({ ...request(), tool: "write" })).toEqual({ type: "tool", tool: "write" });
  expect(safeApprovalMatcher({ ...request(), tool: "unknown" })).toBeUndefined();
});
