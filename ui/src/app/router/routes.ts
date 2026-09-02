export type PageKey = "overview" | "tools" | "policies" | "activity" | "settings";

export const pages: Array<{ key: PageKey; label: string; eyebrow: string }> = [
  { key: "overview", label: "Overview", eyebrow: "Control plane" },
  { key: "tools", label: "Tools", eyebrow: "Capability catalog" },
  { key: "policies", label: "Policies", eyebrow: "Security posture" },
  { key: "activity", label: "Activity", eyebrow: "Audit trail" },
  { key: "settings", label: "Settings", eyebrow: "Configuration" },
];
