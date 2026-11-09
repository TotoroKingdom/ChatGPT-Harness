import type { AppConfig } from "@protocol/v1";

/** First configuration seed only. Runtime effective values are never derived here. */
export const initialConfig: AppConfig = {
  schemaVersion: 3, configRevision: "0", workspace: { rootPath: "" },
  tunnel: { tunnelId: "", autoDetect: true }, security: { mode: "normal", developerTestMode: false }, ui: {},
  tools: { readFile: { enabled: true }, glob: { enabled: true }, grep: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
  bash: { autoDetect: true }, policies: { toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
  desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false },
  history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
  search: { globTimeoutSeconds: 30, grepTimeoutSeconds: 30 },
};
export type ProxyValidationKey = "validation.settings.proxyUrl" | "validation.settings.proxyProtocol" | "validation.settings.proxyHost" | "validation.settings.proxyCredentials" | "validation.settings.proxyPath";
export function validateControlPlaneProxy(value: string): ProxyValidationKey | "" {
  const trimmed = value.trim();
  if (!trimmed) return "";
  let parsed: URL;
  try { parsed = new URL(trimmed); } catch { return "validation.settings.proxyUrl"; }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return "validation.settings.proxyProtocol";
  if (!parsed.hostname) return "validation.settings.proxyHost";
  if (parsed.username || parsed.password) return "validation.settings.proxyCredentials";
  if (parsed.pathname !== "/" || parsed.search || parsed.hash) return "validation.settings.proxyPath";
  return "";
}
