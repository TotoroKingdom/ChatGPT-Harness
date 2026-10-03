import type { ActivityEntry, RuntimeDiagnostics, ToolDiagnostic } from "./control-read";
import type { AppConfig, ActivityEvent, ActivityListResult, ActivitySubscribeResult, ActivityEventType, ApprovalDecision, ApprovalRequest, ApprovalsListResult, DiagnosticRun, PoliciesListResult, StartEligibility, PolicyDecision, PolicyEffectiveSnapshot, PolicyRule, RuntimeState, SecurityMode, ToolDescriptor, ToolsListResult } from "@protocol/v1";
import { HarnessClientError } from "./errors";
import type { ActivityEventListener, ComponentStates, ComponentStatesListener, HarnessControlClient, HarnessStatus, RuntimeProcessStatus, Unsubscribe } from "./client";
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
const completeConfig = (): AppConfig => ({ schemaVersion: 1, configRevision: "7", workspace: { rootPath: "D:\\example-project" }, tunnel: { tunnelId: "mock-tunnel", tunnelClientPath: "D:\\example-project\\tunnel-client-v0.0.13-windows-amd64", autoDetect: true, apiKeyRef: { provider: "memory-development", key: "tunnel-api-key" } }, security: { mode: "normal", developerTestMode: false }, ui: {}, tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } }, bash: { autoDetect: true }, policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] } });
function event(sequence: string, eventType: ActivityEventType, summary: string, metadata: Record<string,string>, requestId?: string): ActivityEvent { return { eventId: `event_mock_${sequence}`, sequence, eventType, requestId, timestamp: `2026-09-02T00:0${sequence}:00Z`, summary, metadata }; }
const tools: ToolDescriptor[] = [{name:"read_file",description:"Read a workspace file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"low"},{name:"glob",description:"Find workspace paths.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"low"},{name:"write",description:"Write a workspace file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"medium"},{name:"edit",description:"Edit a workspace text file.",implemented:true,enabled:true,exposed:true,effectivePolicy:"allow",scope:"active_workspace",readiness:"ready",riskLevel:"medium"},{name:"bash",description:"Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.",implemented:true,enabled:false,exposed:false,effectivePolicy:"ask",scope:"active_workspace",readiness:"unavailable",riskLevel:"high"}];
const policies: PolicyRule[] = [{id:"system-read-workspace",name:"Allow workspace reads",decision:"allow",scope:"active_workspace",reason:"Read-only default capability.",source:"system",editable:false,enabled:true},{id:"user-review-sensitive",name:"Review sensitive requests",decision:"ask",scope:"developer_test_capabilities",reason:"Requires explicit approval before a high-risk request.",source:"user",editable:false,enabled:true},{id:"system-deny-outside",name:"Deny outside workspace",decision:"deny",scope:"outside_active_workspace",reason:"Workspace boundary.",source:"system",editable:false,enabled:true}];
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
const approvals: ApprovalRequest[] = [{requestId:"approval_mock_01",runtimeId:"runtime_mock_01",generationId:"generation_mock_01",tool:"write",sanitizedArguments:{path:"notes.txt",content:"<redacted content>"},scopeSummary:"notes.txt",matchedRuleIds:["review-writes"],policyReason:"matched_user_rules",riskLevel:"medium",createdAt:"2026-09-02T00:03:00Z",expiresAt:"2026-09-02T00:08:00Z",status:"pending"}];
export class MockHarnessControlClient implements HarnessControlClient {
 readonly adapterKind = "mock" as const;
 private scenario: MockScenario; private config: AppConfig | null = completeConfig(); private activeTools: AppConfig["tools"] | null = completeConfig().tools; private activePolicies: AppConfig["policies"] = completeConfig().policies; private configured = true; private components: ComponentStates = {runtime:"ready",tunnel:"ready",mcp:"ready"}; private activeRevision = "7"; private nextSequence="4"; private activities: ActivityEvent[];
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
 runtimeGetActivitySnapshot(_limit: number): Promise<ActivityEntry[]> { return this.scenario === "empty" ? Promise.resolve([]) : Promise.reject({code:"RUNTIME_UNAVAILABLE"}); }
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
 configSave(config:AppConfig,apiKey?:string){
  const previousRevision = this.config?.configRevision ?? "0";
  if(apiKey?.trim()){config={...config,tunnel:{...config.tunnel,apiKeyRef:{provider:"memory-development",key:"tunnel-api-key"}}};this.configured=true;}
  const semanticsChanged = !this.config || JSON.stringify({...this.config,configRevision:"0"}) !== JSON.stringify({...config,configRevision:"0"});
  config={...config,configRevision:semanticsChanged ? incrementDecimal(previousRevision) : previousRevision};
  this.config=config; return this.respond(config);
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
 localDiagnosticsRun():Promise<DiagnosticRun>{const checks=[{id:"workspace",status:"pass" as const,summary:"Workspace folder is accessible."},{id:"tunnel_id",status:"pass" as const,summary:"Tunnel ID is configured."},{id:"tunnel_client",status:this.config?.tunnel.tunnelClientPath?"pass" as const:"warning" as const,summary:this.config?.tunnel.tunnelClientPath?"Tunnel client found":"Choose or detect a tunnel-client package folder."},{id:"api_key",status:this.configured?"pass" as const:"warning" as const,summary:this.configured?"API Key is configured in the OS credential store.":"API Key is not configured."},{id:"security",status:"pass" as const,summary:"Security settings are consistent."}]; return this.respond({runId:"diagnostic_mock",startedAt:"2026-09-02T00:00:00Z",finishedAt:"2026-09-02T00:00:01Z",overallStatus:"warning",checks});}
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
}

