import type {
  ActivityEvent,
  ActivityListParams,
  ActivityListResult,
  ApprovalDecision,
  ApprovalsListParams,
  ApprovalsListResult,
  ApprovalsResolveResult,
  AppConfig,
  DiagnosticRun,
  PoliciesListResult,
  RuntimeState,
  RuntimeStopResult,
  ToolsListResult,
} from "@protocol/v1";

export type ComponentStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type OverallStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type ComponentStates = { runtime: ComponentStatus; tunnel: ComponentStatus; mcp: ComponentStatus };
export type ConfigMetadata = { configDirectory: string; configFile: string };
export type Unsubscribe = () => void;
export type ActivityEventListener = (event: ActivityEvent) => void;
export type ComponentStatesListener = (states: ComponentStates, runtime?: RuntimeState) => void;
export type RuntimeProcessPhase = "stopped" | "starting" | "running" | "stopping" | "crashed";
export type RuntimeProcessEvent = {
  phase: RuntimeProcessPhase;
  error?: { code: string; message: string };
};

export function overallStatus(components: ComponentStates): OverallStatus {
  const values = Object.values(components);
  if (values.includes("error")) return "error";
  if (values.includes("stopping")) return "stopping";
  if (values.some((value) => value === "starting")) return "starting";
  if (values.every((value) => value === "stopped")) return "stopped";
  if (values.every((value) => value === "ready")) return "ready";
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
  runtimeStop(): Promise<RuntimeStopResult>;
  getComponentStates(): ComponentStates;
  setComponentStates(states: ComponentStates): void;
  componentStatesSubscribe(listener: ComponentStatesListener): Promise<Unsubscribe>;
  activityList(params?: ActivityListParams): Promise<ActivityListResult>;
  activitySubscribe(listener: ActivityEventListener, afterSequence?: string): Promise<Unsubscribe>;
  toolsList(): Promise<ToolsListResult>;
  policiesList(): Promise<PoliciesListResult>;
  approvalsList(params?: ApprovalsListParams): Promise<ApprovalsListResult>;
  approvalsResolve(requestId: string, decision: ApprovalDecision): Promise<ApprovalsResolveResult>;
}
