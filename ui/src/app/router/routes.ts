import type { PolicyMatcherConfig } from "@protocol/v1";

export type PageKey = "overview" | "tools" | "policies" | "activity" | "approvals" | "settings";

/**
 * A review handoff from Approval Center to Policies.  The matcher is only a
 * prefilled draft: Policies still validates it and the user must explicitly
 * save the resulting desired configuration.
 */
export type PolicyDraftHandoff = {
  tool: string;
  scope: string;
  name: string;
  reason: string;
  matcher?: PolicyMatcherConfig;
};

export const pages: Array<{ key: PageKey; label: string; eyebrow: string }> = [
  { key: "overview", label: "Overview", eyebrow: "Control plane" },
  { key: "tools", label: "Tools", eyebrow: "Capability catalog" },
  { key: "policies", label: "Policies", eyebrow: "Security posture" },
  { key: "activity", label: "Activity", eyebrow: "Audit trail" },
  { key: "approvals", label: "Approvals", eyebrow: "Human approval" },
  { key: "settings", label: "Settings", eyebrow: "Configuration" },
];
