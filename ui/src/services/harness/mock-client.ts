import type { ActivityEntry, RuntimeDiagnostics, ToolDiagnostic } from "./control-read";
import type { AppConfig, ActivityEvent, ActivityListResult, ActivitySubscribeResult, ActivityEventType, ApprovalDecision, ApprovalRequest, ApprovalsListResult, DiagnosticRun, EvidenceStoreStatus, HistoryClearResult, HistoryGetParams, HistoryGetResult, HistoryQueryParams, HistoryQueryResult, HistoryRequestSummary, MetricsQueryParams, MetricsQueryResult, PoliciesListResult, StartEligibility, PolicyDecision, PolicyEffectiveSnapshot, PolicyRule, RuntimeState, SecurityMode, StartupAttempt, ToolsListResult, ToolDescriptor, TraceGetParams, TraceGetResult, UiLanguage } from "@protocol/v1";
import { HarnessClientError } from "./errors";
import type { ActivityEventListener, ComponentStates, ComponentStatesListener, DesktopNavigationIntent, DesktopPreferencesPatch, HarnessControlClient, HarnessStatus, RuntimeProcessStatus, Unsubscribe } from "./client";
export type MockScenario = "ready" | "disconnected" | "empty" | "error";
const delay = (ms=120) => new Promise<void>((resolve) => window.setTimeout(resolve, ms));
const MAX_DECIMAL_U64 = "18446744073709551615";
function incrementDecimal(value: string): string {
  if (value === MAX_DECIMAL_U64) throw new HarnessClientError("error", "configRevision overflow", undefined, "CONFIG_REVISION_OVERFLOW");
  const digits = value.split("");
  let carry = 1;
  for (let index = digits.length - 1; index >= 0 && carry; index -= 1) {
    const next = digits[index].charCodeAt(0) - 48 + carry;
    digits[index] = String(next % 10);
    carry = next > 9 ? 1 : 0;
  }
  return carry ? `1${digits.join("")}` : digits.join("");
}
function decrementDecimal(value: string): string {
  const digits = value.split("");
  for (let index = digits.length - 1; index >= 0; index -= 1) {
    if (digits[index] !== "0") { digits[index] = String(digits[index].charCodeAt(0) - 49); return digits.join("").replace(/^0+(?=\\d)/, ""); }
    digits[index] = "9";
  }
  return "0";
}
const completeConfig = (): AppConfig => ({ schemaVersion: 3, configRevision: "7", workspace: { rootPath: "D:\\example-project" }, tunnel: { tunnelId: "mock-tunnel", tunnelClientPath: "D:\\example-project\\tunnel-client-v0.0.13-windows-amd64", autoDetect: true, apiKeyRef: { provider: "memory-development", key: "tunnel-api-key" } }, security: { mode: "normal", developerTestMode: false }, ui: {}, tools: { readFile: { enabled: true }, glob: { enabled: true }, grep: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } }, bash: { autoDetect: true }, policies: { toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] }, desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false }, history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 }, search: { globTimeoutSeconds: 30, grepTimeoutSeconds: 30 } });
const safeConfig = (): AppConfig => {
 const config = completeConfig();
 return {
  ...config,
  configRevision: "0",
  workspace: { rootPath: "" },
  tunnel: { ...config.tunnel, tunnelId: "", tunnelClientPath: undefined, apiKeyRef: undefined },
 };
};
function event(sequence: string, eventType: ActivityEventType, summary: string, metadata: Record<string,string>, requestId?: string): ActivityEvent { return { eventId: `event_mock_${sequence}`, runtimeId: "runtime_mock_01", sequence, eventType, requestId, timestamp: new Date(Date.now() - (4 - Number(sequence)) * 60_000).toISOString(), summary, metadata }; }
const tools: ToolDescriptor[] = [{name:"read_file",description:"Read a workspace file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"low"},{name:"glob",description:"Find workspace paths.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"low"},{name:"grep",description:"Search text content inside the active Workspace.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"low"},{name:"write",description:"Write a workspace file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"medium"},{name:"edit",description:"Edit a workspace text file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"medium"},{name:"bash",description:"Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.",implemented:true,enabled:false,exposed:false,effectivePolicy:"ask",scope:"active_workspace",readiness:"unavailable",riskLevel:"high"}];
const policies: PolicyRule[] = [{id:"system-read-workspace",name:"Allow workspace reads",decision:"allow",scope:"active_workspace",reason:"Read-only default capability.",source:"system",editable:false,enabled:true},{id:"user-review-sensitive",name:"Review sensitive requests",decision:"ask",scope:"developer_test_capabilities",reason:"Requires explicit approval before a high-risk request.",source:"user",editable:false,enabled:true},{id:"system-deny-outside",name:"Deny outside workspace",decision:"deny",scope:"outside_active_workspace",reason:"Workspace boundary.",source:"system",editable:false,enabled:true}];
function restartSemantics(config: AppConfig | null) {
 if (!config) return null;
 const { configRevision: _configRevision, ui: _ui, ...runtimeConfig } = config;
 return runtimeConfig;
}
const policySeverity = (decision: PolicyDecision) => decision === "deny" ? 2 : decision === "ask" ? 1 : 0;
function effectivePolicy(config: AppConfig["policies"]): PolicyEffectiveSnapshot {
  const systemRules = policies.filter((rule) => rule.source === "system").map((rule) => ({ id: rule.id, name: rule.name, decision: rule.decision, reason: rule.reason, source: rule.source, editable: false, enabled: rule.enabled }));
  const userRules = config.userRules.map((rule) => ({ id: rule.id, name: rule.name, decision: rule.decision, matcher: rule.matcher, reason: rule.reason, source: "user" as const, editable: true, enabled: rule.enabled }));
  const perTool = tools.map((tool) => {
    const matches = config.userRules.filter((rule) => rule.enabled && rule.matcher.type === "tool" && rule.matcher.tool === tool.name).sort((left, right) => right.id.localeCompare(left.id));
    const decisive = matches.slice().sort((left, right) => policySeverity(right.decision) - policySeverity(left.decision))[0];
    const decision = decisive?.decision ?? config.toolDefaults[tool.name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as keyof typeof config.toolDefaults];
    return { tool: tool.name, decision, matchedRuleIds: matches.map((rule) => rule.id).sort(), reason: decisive ? "matched_user_rules" as const : "tool_default" as const, availability: decision === "allow" ? "available" as const : "unavailable_by_policy" as const };
  });
  return { valid: true, status: "valid", systemRules, userRules, toolDefaults: config.toolDefaults, perTool };
}
const approvalCreatedAt = new Date(Date.now() - 60_000).toISOString();
const approvalExpiresAt = new Date(Date.now() + 5 * 60_000).toISOString();
const approvals: ApprovalRequest[] = [{requestId:"approval_mock_01",runtimeId:"runtime_mock_01",generationId:"generation_mock_01",tool:"write",sanitizedArguments:{path:"notes.txt",content:"<redacted content>"},scopeSummary:"notes.txt",matchedRuleIds:["review-writes"],policyReason:"matched_user_rules",riskLevel:"medium",createdAt:approvalCreatedAt,expiresAt:approvalExpiresAt,status:"pending"}];
const hoursAgo = (hours: number) => new Date(Date.now() - hours * 60 * 60 * 1000).toISOString();
const historyItems: HistoryRequestSummary[] = [
  { runtimeId: "runtime_mock_01", requestId: "history-read-1", toolName: "read_file", firstSeenAtUtc: hoursAgo(1), lastSeenAtUtc: hoursAgo(1), terminalState: "succeeded", outcome: "success", policyDecision: "allow", approvalState: "none", durationMs: 12, evidenceCoverage: "COMPLETE" },
  { runtimeId: "runtime_mock_01", requestId: "history-write-1", toolName: "write", firstSeenAtUtc: hoursAgo(2), lastSeenAtUtc: hoursAgo(2), terminalState: "denied", outcome: "denied", policyDecision: "deny", approvalState: "none", failureCategory: "execution_failed", durationMs: 2, evidenceCoverage: "COMPLETE" },
  { runtimeId: "runtime_mock_00", requestId: "history-incomplete-1", toolName: "bash", firstSeenAtUtc: hoursAgo(25), lastSeenAtUtc: hoursAgo(25), terminalState: "incomplete", approvalState: "required_observed", evidenceCoverage: "INCOMPLETE" },
];
export class MockHarnessControlClient implements HarnessControlClient {
 readonly adapterKind = "mock" as const;
 private scenario: MockScenario; private config: AppConfig | null = completeConfig(); private activeTools: AppConfig["tools"] | null = completeConfig().tools; private activePolicies: AppConfig["policies"] = completeConfig().policies; private configured = true; private components: ComponentStates = {runtime:"ready",tunnel:"ready",mcp:"ready"}; private activeRevision = "7"; private nextSequence="4"; private activities: ActivityEvent[]; private storedHistory: HistoryRequestSummary[] = historyItems.map((item) => ({ ...item }));
 constructor(scenario: MockScenario="ready") { this.scenario=scenario; this.activities=scenario==="empty"?[]:[event("1","runtime_started","Runtime started in mock mode.",{source:"mock-runtime"}),event("2","policy_evaluated","Workspace read policy evaluated.",{decision:"allow"},"request_mock_01"),event("3","tool_requested","ChatGPT requested read_file.",{tool:"read_file"},"request_mock_01")]; if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("setup")==="first-run") { this.config=null; this.configured=false; this.components={runtime:"stopped",tunnel:"stopped",mcp:"stopped"}; this.activeRevision="0"; this.activities=[]; } }
 syncConfig(config: AppConfig | null, apiKeyConfigured: boolean) {
  this.config = config;
  this.configured = apiKeyConfigured;
  if (!config) { this.components = { runtime: "stopped", tunnel: "stopped", mcp: "stopped" }; this.activeRevision="0"; }
 }
 private harnessState(): HarnessStatus { const lifecycle = this.components.runtime === "error" || this.components.tunnel === "error" || this.components.mcp === "error" ? "error" : this.components.runtime === "stopping" || this.components.tunnel === "stopping" || this.components.mcp === "stopping" ? "stopping" : this.components.runtime === "starting" || this.components.tunnel === "starting" || this.components.mcp === "starting" ? "starting" : this.components.runtime === "ready" && this.components.tunnel === "ready" && this.components.mcp === "ready" ? "ready" : this.components.runtime === "stopped" && this.components.tunnel === "stopped" && this.components.mcp === "stopped" ? "stopped" : "degraded"; return { lifecycle, ...this.components, activeSessionId: lifecycle === "stopped" ? null : "mock-session", configRevision: lifecycle === "stopped" ? null : this.activeRevision, lastError: null }; }
 private recordActivity(eventType: ActivityEventType, summary: string) { const created=event(this.nextSequence,eventType,summary,{source:"mock-lifecycle"}); this.nextSequence=incrementDecimal(this.nextSequence); this.activities=[...this.activities,created]; }
 private async respond<T>(value:T):Promise<T>{await delay(); if(this.scenario==="disconnected") throw new HarnessClientError("disconnected","The local Runtime is not connected."); if(this.scenario==="error") throw new HarnessClientError("error","The mock Runtime returned an example error."); return value;}
 // Legacy mock lifecycle data is not evidence for M8-C observations.
 runtimeGetDiagnostics(): Promise<RuntimeDiagnostics> { return Promise.reject({code:"RUNTIME_UNAVAILABLE"}); }
  runtimeGetActivitySnapshot(limit: number): Promise<ActivityEntry[]> {
    if (this.scenario === "empty") return Promise.resolve([]);
    const items: ActivityEntry[] = [
      { eventId: "activity_mock_read", requestId: "request_mock_01", category: "execution", lifecycleState: "completed", timestamp: new Date(Date.now() - 3 * 60_000).toISOString(), eventType: "execution_completed", toolName: "read_file", decisionSummary: "allow", matchedRuleIds: [], policyReason: "tool_default", outcomeSummary: "success", durationMs: 12, sanitizedArguments: { tool: "read_file", logicalPath: "src/App.tsx" }, resultEvidence: { tool: "read_file", bytesRead: 1840 } },
      { eventId: "activity_mock_approval", requestId: "request_mock_02", category: "approval", lifecycleState: "awaiting_approval", timestamp: new Date(Date.now() - 2 * 60_000).toISOString(), eventType: "approval_required", toolName: "write", decisionSummary: "ask", matchedRuleIds: ["review-writes"], policyReason: "matched_user_rules", outcomeSummary: "approval_required", approvalId: "approval_mock_01", sanitizedArguments: { tool: "write", logicalPath: "notes.txt" } },
      { eventId: "activity_mock_denied", requestId: "request_mock_03", category: "policy", lifecycleState: "denied", timestamp: new Date(Date.now() - 60_000).toISOString(), eventType: "request_denied", toolName: "bash", decisionSummary: "deny", matchedRuleIds: ["system-deny-outside"], policyReason: "system_guardrail", outcomeSummary: "denied" },
    ];
    return this.respond(items.slice(-Math.min(Math.max(limit, 1), 100)));
  }
 private toolCatalog(): ToolDescriptor[] { return tools.map((tool) => { const key = tool.name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as keyof AppConfig["tools"]; const configured = (this.activeTools ?? this.config?.tools)?.[key]?.enabled ?? tool.enabled; return { ...tool, enabled: configured, exposed: tool.name === "bash" ? configured && this.config?.security.mode === "developer_test" : configured }; }); }
 private effectiveTools(): ToolDiagnostic[] {
  const config = this.config ?? completeConfig();
  return this.toolCatalog().map((tool) => {
   if (tool.name === "bash") {
    if (config.security.mode !== "developer_test") return { name: tool.name, availability: "unavailable_by_mode" as const, decision: "deny" as const, declaredCapability: "developer_shell" as const, readiness: "unavailable" as const, readinessReason: "Developer/Test mode is required." };
    if (!tool.enabled) return { name: tool.name, availability: "disabled" as const, decision: "deny" as const, declaredCapability: "developer_shell" as const, readiness: "unavailable" as const, readinessReason: "Bash is disabled in the desired Runtime configuration." };
    return { name: tool.name, availability: "unavailable_by_runtime" as const, decision: "deny" as const, declaredCapability: "developer_shell" as const, readiness: "unavailable" as const, reason: "Git Bash readiness is unavailable in the mock Runtime.", readinessReason: "Start a Runtime generation with a validated Git Bash executable." };
   }
   return { name: tool.name, availability: tool.enabled ? "available" as const : "disabled" as const, decision: tool.enabled ? "allow" as const : "deny" as const };
  });
 }
 runtimeGetEffectiveTools(): Promise<ToolDiagnostic[]> { return this.scenario === "empty" ? Promise.resolve([]) : this.respond(this.effectiveTools()); }
 configGet(){return this.respond(this.config);}
 configMetadata(){return this.respond({configDirectory:"%APPDATA%\\ChatGPT-Harness",configFile:"%APPDATA%\\ChatGPT-Harness\\config.json"});}
 async configSave(config:AppConfig,apiKey?:string){
  const previousRevision = this.config?.configRevision ?? "0";
  const apiKeySaved = Boolean(apiKey?.trim());
  const candidate = apiKeySaved ? {...config,tunnel:{...config.tunnel,apiKeyRef:{provider:"memory-development",key:"tunnel-api-key"}}} : config;
  const semanticsChanged = !this.config || JSON.stringify(restartSemantics(this.config)) !== JSON.stringify(restartSemantics(candidate));
  const next = {...candidate,configRevision:semanticsChanged ? incrementDecimal(previousRevision) : previousRevision};
  const persisted = await this.respond(next);
  this.config=persisted;
  if(apiKeySaved)this.configured=true;
  return persisted;
 }
 async uiLanguageSet(language: UiLanguage | null): Promise<AppConfig>{
  const hadPersistedConfig = this.config !== null;
  const base = this.config ?? safeConfig();
  const ui: AppConfig["ui"] = {...base.ui};
  if (language === null) delete ui.language;
  else ui.language = language;
  const next = {...base,ui,configRevision: hadPersistedConfig ? base.configRevision : "1"};
  const persisted = await this.respond(next);
  this.config=persisted;
  return persisted;
 }
 configReset(){this.config=null;this.configured=false;return this.respond(undefined);}
 apiKeyStatus(){return this.respond(this.configured);}
 apiKeyDelete(){this.configured=false;if(this.config)this.config={...this.config,tunnel:{...this.config.tunnel,apiKeyRef:undefined}};return this.respond(undefined);}
 chooseWorkspace(){return this.respond("D:\\example-project");}
 chooseTunnelClient(){return this.respond("D:\\example-project\\tunnel-client-v0.0.13-windows-amd64");}
 chooseBashExecutable(){return this.respond(null);}
 detectTunnelClient(){return this.respond("D:\\example-project\\tunnel-client-v0.0.13-windows-amd64");}
 validatePath(){return this.respond(true);}
 runtimeStartEligibility(): Promise<StartEligibility> {
  const running = this.components.runtime !== "stopped" && this.components.runtime !== "error";
 if (running) return this.respond({ status: "runtime_already_running", canStart: false, message: "Local Runtime is already running or changing state." });
  if (!this.config?.workspace.rootPath.trim()) return this.respond({ status: "setup_required", canStart: false, message: "Choose a Workspace to finish local setup." });
  if (!this.config.tunnel.tunnelId.trim()) return this.respond({ status: "tunnel_id_required", canStart: false, message: "Configure a Tunnel ID before starting Harness." });
  if (!this.configured) return this.respond({ status: "api_key_required", canStart: false, message: "Save a Tunnel API Key before starting Harness." });
  return this.respond({ status: "ready_to_start", canStart: true, message: "Local Runtime is ready to start." });
 }
 harnessStartEligibility(): Promise<StartEligibility> { return this.runtimeStartEligibility(); }
 private mockDiagnostics(): DiagnosticRun {
  const checks = [
   { id: "workspace", status: "pass" as const, summary: "Workspace folder is accessible.", scope: "workspace" as const, evidence: [{ key: "exists" as const, value: "true" }] },
   { id: "runtime_process", status: this.components.runtime === "ready" ? "pass" as const : "warning" as const, summary: this.components.runtime === "ready" ? "Runtime process is running." : "Runtime process is not ready.", scope: "runtime" as const, evidence: [{ key: "process_phase" as const, value: this.components.runtime }] },
   { id: "credential_presence", status: this.configured ? "pass" as const : "fail" as const, summary: this.configured ? "The referenced credential is present." : "The referenced credential is missing.", scope: "credential" as const, evidence: [{ key: "exists" as const, value: String(this.configured) }] },
  ];
  return { runId: "diagnostic_mock", startedAt: "2026-09-02T00:00:00Z", finishedAt: "2026-09-02T00:00:01Z", overallStatus: checks.some((check) => check.status === "fail") ? "fail" : checks.some((check) => check.status === "warning") ? "warning" : "pass", checks };
 }
 localDiagnosticsRun(): Promise<DiagnosticRun> { return this.connectionDiagnosticsRun(); }
 connectionDiagnosticsRun(): Promise<DiagnosticRun> { return this.respond(this.mockDiagnostics()); }
 startupLatest(): Promise<StartupAttempt | null> {
  if (this.getHarnessStatus().lifecycle === "stopped") return this.respond(null);
  const phases = ["eligibility_prepare", "runtime_spawn", "runtime_initialize", "mcp_endpoint_ready", "tunnel_resolve", "tunnel_launch", "control_plane_connected", "mcp_target_ready", "harness_ready"] as const;
  return this.respond({ attemptId: "startup_mock", harnessSessionId: "mock-session", cohort: "warm", startedAt: "2026-09-02T00:00:00Z", finishedAt: "2026-09-02T00:00:01Z", outcome: "ready", totalDurationMs: 1000, phases: phases.map((phase, index) => ({ phase, startedOffsetMs: index * 100, durationMs: 100, status: "completed" as const })), appVersion: "0.1.3" });
 }
 private runtime():RuntimeState{return {runtimeId:"runtime_mock_01",startedAt:"2026-09-02T00:00:00Z",lifecycle:this.components.runtime==="ready"?"ready":this.components.runtime==="stopping"?"stopping":"initializing",readiness:this.components.runtime==="ready"?"ready":this.components.runtime==="degraded"?"degraded":"not_ready",configRevision:this.activeRevision,activeWorkspace:this.config?.workspace.rootPath ?? "",securityMode:this.config?.security.mode ?? "normal",exposedToolCatalog:this.toolCatalog().filter(t=>t.exposed),activeRequests:0,pendingApprovals:approvals.filter((item) => item.status === "pending").length,runningExecutions:0};}
 async runtimeStart(){this.components={runtime:"starting",tunnel:"stopped",mcp:"stopped"}; await delay(90); this.components={runtime:"ready",tunnel:"starting",mcp:"stopped"}; await delay(90); this.components={runtime:"ready",tunnel:"ready",mcp:"starting"}; await delay(90); this.components={runtime:"ready",tunnel:"ready",mcp:"ready"}; this.activeTools=this.config?.tools ?? completeConfig().tools; this.activePolicies=this.config?.policies ?? completeConfig().policies; this.activeRevision=this.config?.configRevision??"1"; this.recordActivity("runtime_started","Runtime started in mock mode."); return this.respond(this.runtime());}
 runtimeStatus(){return this.respond(this.runtime());}
 runtimeProcessStatus():Promise<RuntimeProcessStatus>{const phase=this.components.runtime==="starting"?"starting":this.components.runtime==="stopping"?"stopping":this.components.runtime==="error"?"crashed":this.components.runtime==="stopped"?"stopped":"running";return this.respond({phase,last_error:null});}
 async runtimeStop(){if(this.components.runtime==="stopped")return this.respond({accepted:false});this.components={runtime:"ready",tunnel:"ready",mcp:"stopping"};await delay(90);this.components={runtime:"ready",tunnel:"stopping",mcp:"stopped"};await delay(90);this.components={runtime:"stopping",tunnel:"stopped",mcp:"stopped"};await delay(90);this.components={runtime:"stopped",tunnel:"stopped",mcp:"stopped"};this.activeTools=null;this.activeRevision="0";this.recordActivity("runtime_stopped","Runtime stopped in mock mode.");return this.respond({accepted:true});}
 async harnessStart(){await this.runtimeStart();return this.respond(this.harnessState());}
 async harnessStatus(){return this.respond(this.harnessState());}
 async harnessStop(){await this.runtimeStop();return this.respond(this.harnessState());}
 async harnessRestart(){await this.runtimeStop();await this.runtimeStart();return this.respond(this.harnessState());}
 getHarnessStatus(){return this.harnessState();}
 getComponentStates(){return this.components;} setComponentStates(s:ComponentStates){this.components=s;if(s.runtime==="stopped"&&s.tunnel==="stopped"&&s.mcp==="stopped")this.activeRevision="0";} async componentStatesSubscribe(_listener:ComponentStatesListener):Promise<Unsubscribe>{return ()=>{};}
 toolsList(){return this.respond({items:this.scenario==="empty"?[]:this.toolCatalog()});} policiesList(){return this.respond(this.scenario==="empty"?{items:[]}:{items:policies,effective:effectivePolicy(this.activePolicies)});}
 activityList():Promise<ActivityListResult>{return this.respond({items:this.activities,latestSequence:decrementDecimal(this.nextSequence)});}
  async historyQuery(params: HistoryQueryParams = {}): Promise<HistoryQueryResult> {
    const filtered = this.storedHistory.filter((item) =>
      (!params.fromUtc || item.firstSeenAtUtc >= params.fromUtc)
      && (!params.toUtc || item.firstSeenAtUtc < params.toUtc)
      && (!params.tool || item.toolName === params.tool)
      && (!params.outcome || item.outcome === params.outcome)
      && (!params.policyDecision || item.policyDecision === params.policyDecision)
      && (!params.approvalState || item.approvalState === params.approvalState));
    const offset = params.cursor?.startsWith("mock:") ? Number(params.cursor.slice(5)) : 0;
    const limit = Math.min(Math.max(params.limit ?? 50, 1), 200);
    const items = filtered.slice(offset, offset + limit);
    const next = offset + items.length < filtered.length ? `mock:${offset + items.length}` : undefined;
    return this.respond({ items, ...(next ? { nextCursor: next } : {}), storeStatus: this.historyStatusValue() });
  }
  async historyGet(params: HistoryGetParams): Promise<HistoryGetResult> {
    const summary = this.storedHistory.find((item) => item.runtimeId === params.runtimeId && item.requestId === params.requestId);
    if (!summary) throw new HarnessClientError("error", "History request was not found.", undefined, "EVIDENCE_NOT_FOUND");
    const terminal = summary.terminalState === "succeeded";
    return this.respond({
      summary,
      events: [
        { schemaVersion: 1, eventId: "history-event-1", runtimeId: summary.runtimeId, requestId: summary.requestId, toolName: summary.toolName, occurredAtUtc: summary.firstSeenAtUtc, recordedAtUtc: summary.firstSeenAtUtc, stage: "request_received", lifecycleState: "received", workspaceValidation: "not_attempted", matchedRuleIds: [], outcome: "in_progress", coverage: summary.evidenceCoverage },
        ...(terminal ? [{ schemaVersion: 1 as const, eventId: "history-event-2", runtimeId: summary.runtimeId, requestId: summary.requestId, toolName: summary.toolName, occurredAtUtc: summary.lastSeenAtUtc, recordedAtUtc: summary.lastSeenAtUtc, stage: "execution_completed" as const, lifecycleState: "completed" as const, workspaceValidation: "passed" as const, policyDecision: "allow" as const, matchedRuleIds: [], outcome: "success" as const, durationMs: summary.durationMs, coverage: "COMPLETE" as const }] : []),
      ],
      ...(terminal ? { terminalApproval: { runtimeId: summary.runtimeId, approvalId: "mock-approval", requestId: summary.requestId, toolName: summary.toolName, terminalAtUtc: summary.lastSeenAtUtc, terminalState: "allow_once" as const, decision: "allow_once" as const } } : {}),
      ...(summary.evidenceCoverage === "COMPLETE" ? {} : { notice: { code: "EVIDENCE_QUEUE_OVERFLOW" as const, coverage: summary.evidenceCoverage } }),
      storeStatus: this.historyStatusValue(),
    });
  }
  historyStatus(): Promise<EvidenceStoreStatus> { return this.respond(this.historyStatusValue()); }
  metricsQuery(_params: MetricsQueryParams): Promise<MetricsQueryResult> {
    const status = this.historyStatusValue();
    const noData = { sampleCount: 0, averageMs: undefined, p95Ms: undefined, sampleStatus: "NO_DATA" as const };
    return this.respond({
      counts: {
        receivedCalls: 0, executionAttempts: 0, successfulCalls: 0, failedCalls: 0,
        deniedCalls: 0, cancelledCalls: 0, timedOutCalls: 0, preExecutionFailedCalls: 0,
        approvalExpiredCalls: 0, approvalCancelledCalls: 0, incompleteCalls: 0,
        unknownCalls: 0, legacyUntracedCalls: this.storedHistory.length,
        completedExecutionDenominator: 0, executionSuccessNumerator: 0,
        executionSuccessRate: undefined, executionSuccessSampleStatus: "NO_DATA" as const,
      }, execution: noData, approvalWait: noData, requestWall: noData,
      coverage: {
        completeRequestCount: 0, incompleteRequestCount: 0, unknownRequestCount: 0,
        legacyUntracedRequestCount: this.storedHistory.length,
        executionDurationEligibleCount: 0, approvalWaitDurationEligibleCount: 0,
        requestWallDurationEligibleCount: 0, storeStatus: status,
        lastCommitAtUtc: status.lastCommitAtUtc, dropCount: status.dropCount,
        coverageUnknownSinceUtc: status.coverageUnknownSinceUtc,
      },
    });
  }
  traceGet(params: TraceGetParams): Promise<TraceGetResult> {
    const item = this.storedHistory.find((candidate) => candidate.runtimeId === params.runtimeId && candidate.requestId === params.requestId);
    if (!item) throw new HarnessClientError("error", "History request was not found.", undefined, "EVIDENCE_NOT_FOUND");
    return this.respond({
      runtimeId: item.runtimeId, requestId: item.requestId, traceId: undefined, toolName: item.toolName,
      outcome: item.outcome, policyDecision: item.policyDecision, approvalState: item.approvalState,
      executorStarted: item.outcome === "success" || item.outcome === "failed" || item.outcome === "cancelled" || item.outcome === "timed_out",
      coverage: "legacy_untraced" as const, availability: "legacy_untraced" as const, spans: [],
      failureStage: undefined, slowestMeasuredSpan: undefined, storeStatus: this.historyStatusValue(),
    });
  }
  async historyClear(): Promise<HistoryClearResult> {
    const deletedRequestCount = this.storedHistory.length;
    this.storedHistory.splice(0, this.storedHistory.length);
    return this.respond({ deletedRequestCount, deletedEventCount: deletedRequestCount * 2, deletedApprovalCount: 1, footprintBytes: 0, completedAtUtc: "2026-09-02T00:10:00Z" });
  }
  private historyStatusValue(): EvidenceStoreStatus {
    return {
      state: this.scenario === "error" ? "corrupt" : this.scenario === "empty" ? "disabled" : "healthy",
      schemaVersion: 1,
      queueDepth: 0,
      queueCapacity: 1024,
      dropCount: 0,
      effectiveHistoryConfig: this.config?.history ?? completeConfig().history,
      footprintBytes: 4096,
    };
  }
 async activitySubscribe(listener:ActivityEventListener,_afterSequence?:string):Promise<Unsubscribe>{await this.respond<ActivitySubscribeResult>({latestSequence:decrementDecimal(this.nextSequence)});let active=true;const timer=window.setTimeout(()=>{if(active&&this.scenario==="ready"){listener(event(this.nextSequence,"execution_completed","Mock activity stream received a completion event.",{source:"mock-subscription"},"request_mock_02"));this.nextSequence=incrementDecimal(this.nextSequence);}},220);return()=>{active=false;window.clearTimeout(timer);};}
 approvalsList(params: { status?: ApprovalRequest["status"] } = {}):Promise<ApprovalsListResult>{return this.respond({items:this.scenario==="empty"?[]:approvals.filter((item) => !params.status || item.status === params.status)});}
 async approvalsResolve(requestId:string, decision:ApprovalDecision){
  const item = approvals.find((approval) => approval.requestId === requestId);
  if (!item) throw new HarnessClientError("error", "Approval request was not found.", undefined, "APPROVAL_NOT_FOUND");
  if (item.status !== "pending") throw new HarnessClientError("error", "Approval request is already resolved.", undefined, "APPROVAL_ALREADY_RESOLVED");
  item.status = decision === "deny" ? "denied" : "approved";
  item.decision = decision;
  item.resolvedAt = "2026-09-02T00:04:00Z";
  return this.respond({ approval: { ...item } });
 }
 desktopNavigationSubscribe(_listener: (intent: DesktopNavigationIntent) => void) { return Promise.resolve(() => undefined); }
 desktopBackgroundNoticeSubscribe(_listener: () => void) { return Promise.resolve(() => undefined); }
 async desktopPreferencesSet(patch: DesktopPreferencesPatch) {
  const base = this.config ?? safeConfig();
  const next = { ...base, desktop: { ...base.desktop, ...patch }, configRevision: base.configRevision };
  this.config = await this.respond(next);
  return this.config;
 }
 desktopContinueInBackground() { return this.respond(undefined); }
 desktopQuit() { return this.respond(undefined); }
}

