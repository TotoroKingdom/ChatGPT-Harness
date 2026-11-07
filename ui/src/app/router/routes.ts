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
  approvalRequestId?: string;
};

export type NavigationOptions = {
  policyDraft?: PolicyDraftHandoff;
  objectId?: string;
  settingsSection?: "general" | "appearance" | "notifications" | "history" | "workspace" | "connection" | "security" | "advanced" | "danger";
};
export type Navigate = (page: PageKey, options?: NavigationOptions) => void;

export const pages: Array<{ key: PageKey; label: string; eyebrow: string; description: string }> = [
  { key: "overview", label: "nav.overview", eyebrow: "nav.eyebrows.overview", description: "nav.descriptions.overview" },
  { key: "tools", label: "nav.tools", eyebrow: "nav.eyebrows.tools", description: "nav.descriptions.tools" },
  { key: "policies", label: "nav.policies", eyebrow: "nav.eyebrows.policies", description: "nav.descriptions.policies" },
  { key: "approvals", label: "nav.approvals", eyebrow: "nav.eyebrows.approvals", description: "nav.descriptions.approvals" },
  { key: "activity", label: "nav.activity", eyebrow: "nav.eyebrows.activity", description: "nav.descriptions.activity" },
  { key: "settings", label: "nav.settings", eyebrow: "nav.eyebrows.settings", description: "nav.descriptions.settings" },
];
