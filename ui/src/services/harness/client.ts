import type { AppConfig, DiagnosticRun, RuntimeState } from "@protocol/v1";

export type ComponentStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type OverallStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type ComponentStates = { runtime: ComponentStatus; tunnel: ComponentStatus; mcp: ComponentStatus };
export type ConfigMetadata = { configDirectory: string; configFile: string };
export type Unsubscribe = () => void;
export type ActivityEventListener = (event: import("@protocol/v1").ActivityEvent) => void;

export function overallStatus(components: ComponentStates): OverallStatus {
  const values = Object.values(components);
  if (values.includes("error")) return "error";
  if (values.includes("stopping")) return "stopping";
  if (values.some((v) => v === "starting")) return "starting";
  if (values.every((v) => v === "stopped")) return "stopped";
  if (values.every((v) => v === "ready")) return "ready";
  return "degraded";
}

export interface HarnessControlClient {
  configGet(): Promise<AppConfig | null>;
  configMetadata(): Promise<ConfigMetadata>;
  configSave(config: AppConfig, apiKey?: string): Promise<AppConfig>;
  configReset(): Promise<void>;
  apiKeyStatus(): Promise<boolean>;
  apiKeyDelete(): Promise<void>;
  chooseWorkspace(): Promise<string | null>;
  chooseTunnelClient(): Promise<string | null>;
  detectTunnelClient(): Promise<string | null>;
  validatePath(path: string, file: boolean): Promise<boolean>;
  localDiagnosticsRun(): Promise<DiagnosticRun>;
  runtimeStart(): Promise<RuntimeState>;
  runtimeStatus(): Promise<RuntimeState>;
  runtimeStop(): Promise<{ accepted: boolean }>;
  getComponentStates(): ComponentStates;
  setComponentStates(states: ComponentStates): void;
  activityList(...args: any[]): Promise<any>;
  activitySubscribe(listener: ActivityEventListener): Promise<Unsubscribe>;
  toolsList(): Promise<any>;
  policiesList(): Promise<any>;
  approvalsList(...args: any[]): Promise<any>;
  approvalsResolve(...args: any[]): Promise<any>;
}