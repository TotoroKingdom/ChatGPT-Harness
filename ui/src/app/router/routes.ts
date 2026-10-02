export type PageKey = "overview" | "tools" | "policies" | "activity" | "approvals" | "settings";
export type PolicyDraftHandoff = { tool: string; scope: string; name: string; reason: string };

export const pages: Array<{ key: PageKey; label: string; eyebrow: string }> = [
  { key: "overview", label: "Overview", eyebrow: "Control plane" },
  { key: "tools", label: "Tools", eyebrow: "Capability catalog" },
  { key: "policies", label: "Policies", eyebrow: "Security posture" },
  { key: "activity", label: "Activity", eyebrow: "Audit trail" },
  { key: "approvals", label: "Approvals", eyebrow: "Human approval" },
  { key: "settings", label: "Settings", eyebrow: "Configuration" },
];
