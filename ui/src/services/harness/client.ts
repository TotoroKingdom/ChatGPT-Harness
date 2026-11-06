import type { ControlReadClient } from "./control-read";
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
  EvidenceStoreStatus,
  HistoryClearResult,
  HistoryGetParams,
  HistoryGetResult,
  HistoryQueryParams,
  HistoryQueryResult,
  MetricsQueryParams,
  MetricsQueryResult,
  TraceGetParams,
  TraceGetResult,
  PoliciesListResult,
  RuntimeState,
  RuntimeStopResult,
  StartupAttempt,
  StartEligibility,
  ToolsListResult,
  UiLanguage,
} from "@protocol/v1";

export type ComponentStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type OverallStatus = "stopped" | "starting" | "ready" | "degraded" | "error" | "stopping";
export type ComponentStates = { runtime: ComponentStatus; tunnel: ComponentStatus; mcp: ComponentStatus };
export type TunnelHealthState = "unknown" | "starting" | "stopping" | "ready" | "degraded" | "failed" | "stopped";
export type TunnelHealth = {
  process: TunnelHealthState;
  localMcp: TunnelHealthState;
  controlPlane: TunnelHealthState;
};
export type TunnelRecoveryStatus = {
  phase: "disarmed" | "idle" | "backoff" | "cooldown" | "half_open" | "recovering" | "blocked";
  reason: { code: string; message: string } | null;
  nextAttemptAt: string | null;
  cooldownUntil: string | null;
};
export type HarnessStatus = {
  lifecycle: OverallStatus;
  runtime: ComponentStatus;
  mcp: ComponentStatus;
  tunnel: ComponentStatus;
  activeSessionId?: string | null;
  configRevision?: string | null;
  lastError?: { code: string; message: string } | null;
  tunnelHealth?: TunnelHealth | null;
  tunnelRecovery?: TunnelRecoveryStatus | null;
  tunnelReadiness?: {
    controlPlaneConnected: boolean;
    mcpTargetReachable: boolean;
    healthCheckPassed: boolean;
    controlPlaneRoute?: {
      routeMode: string;
      proxySource: string;
      proxyUrl?: string | null;
    } | null;
    controlPlaneError?: string | null;
    lastSuccessfulPollTimestamp?: number | null;
    lastSuccessfulPollAgeSeconds?: number | null;
  } | null;
};
export type ConfigMetadata = { configDirectory: string; configFile: string };
export type Unsubscribe = () => void;
export type ActivityEventListener = (event: ActivityEvent) => void;
export type ComponentStatesListener = (
  states: ComponentStates,
  runtime?: RuntimeState,
  process?: RuntimeProcessEvent,
) => void;
export type RuntimeProcessPhase = "stopped" | "starting" | "running" | "stopping" | "crashed";
export type RuntimeProcessEvent = {
  phase: RuntimeProcessPhase;
  error?: { code: string; message: string };
};
export type HarnessAdapterKind = "real" | "mock";
export type RuntimeProcessStatus = {
  phase: RuntimeProcessPhase;
  last_error?: { code: string; message: string } | null;
};
export type DesktopNavigationIntent =
  | { page: "overview" }
  | { page: "approvals"; objectId: string }
  | { page: "approvalsUnavailable"; objectId: string };
export type DesktopPreferencesPatch = {
  approvalNotifications?: boolean;
  connectionNotifications?: boolean;
  backgroundNoticeAcknowledged?: boolean;
  autoStartHarnessOnAppLaunch?: boolean;
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

export interface HarnessControlClient extends ControlReadClient {
  readonly adapterKind: HarnessAdapterKind;
  configGet(): Promise<AppConfig | null>;
  configMetadata(): Promise<ConfigMetadata>;
  configSave(config: AppConfig, apiKey?: string): Promise<AppConfig>;
  uiLanguageSet(language: UiLanguage | null): Promise<AppConfig>;
  configReset(): Promise<void>;
  apiKeyStatus(): Promise<boolean>;
  apiKeyDelete(): Promise<void>;
  chooseWorkspace(): Promise<string | null>;
  chooseTunnelClient(): Promise<string | null>;
  /** Pick a desired Git Bash executable; Runtime validates it on next start. */
  chooseBashExecutable(): Promise<string | null>;
  detectTunnelClient(): Promise<string | null>;
  validatePath(path: string, file: boolean): Promise<boolean>;
  localDiagnosticsRun(): Promise<DiagnosticRun>;
  connectionDiagnosticsRun(): Promise<DiagnosticRun>;
  startupLatest(): Promise<StartupAttempt | null>;
  runtimeStartEligibility(): Promise<StartEligibility>;
  runtimeStart(): Promise<RuntimeState>;
  runtimeStatus(): Promise<RuntimeState>;
  runtimeProcessStatus(): Promise<RuntimeProcessStatus>;
  runtimeStop(): Promise<RuntimeStopResult>;
  harnessStartEligibility(): Promise<StartEligibility>;
  harnessStart(): Promise<HarnessStatus>;
  harnessStatus(): Promise<HarnessStatus>;
  harnessStop(): Promise<HarnessStatus>;
  harnessRestart(): Promise<HarnessStatus>;
  getHarnessStatus(): HarnessStatus;
  getComponentStates(): ComponentStates;
  setComponentStates(states: ComponentStates): void;
  componentStatesSubscribe(listener: ComponentStatesListener): Promise<Unsubscribe>;
  activityList(params?: ActivityListParams): Promise<ActivityListResult>;
  historyQuery(params?: HistoryQueryParams): Promise<HistoryQueryResult>;
  historyGet(params: HistoryGetParams): Promise<HistoryGetResult>;
  historyStatus(): Promise<EvidenceStoreStatus>;
  historyClear(): Promise<HistoryClearResult>;
  /** Read-only bounded projections of retained Runtime evidence. */
  metricsQuery(params: MetricsQueryParams): Promise<MetricsQueryResult>;
  traceGet(params: TraceGetParams): Promise<TraceGetResult>;
  activitySubscribe(listener: ActivityEventListener, afterSequence?: string): Promise<Unsubscribe>;
  toolsList(): Promise<ToolsListResult>;
  policiesList(): Promise<PoliciesListResult>;
  approvalsList(params?: ApprovalsListParams): Promise<ApprovalsListResult>;
  approvalsResolve(requestId: string, decision: ApprovalDecision): Promise<ApprovalsResolveResult>;
  desktopNavigationSubscribe(listener: (intent: DesktopNavigationIntent) => void): Promise<Unsubscribe>;
  desktopBackgroundNoticeSubscribe(listener: () => void): Promise<Unsubscribe>;
  desktopPreferencesSet(patch: DesktopPreferencesPatch): Promise<AppConfig>;
  desktopContinueInBackground(): Promise<void>;
  desktopQuit(): Promise<void>;
}
