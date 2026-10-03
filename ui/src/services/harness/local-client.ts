import { parseActivity, parseDiagnostics, parseTools } from "./control-read";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  StartEligibility,
  ToolsListResult,
} from "@protocol/v1";
import type {
  ActivityEventListener,
  ComponentStates,
  ComponentStatesListener,
  ConfigMetadata,
  HarnessControlClient,
  HarnessAdapterKind,
  HarnessStatus,
  RuntimeProcessEvent,
  RuntimeProcessStatus,
  Unsubscribe,
} from "./client";
import { MockHarnessControlClient } from "./mock-client";

const tauriAvailable = () =>
  typeof window !== "undefined" &&
  Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

function componentStatusFromRuntime(state: RuntimeState): ComponentStates["runtime"] {
  if (state.lifecycle === "error") return "error";
  if (!state.runtimeId) return "stopped";
  if (state.lifecycle === "stopping") return "stopping";
  if (state.lifecycle === "ready" && state.readiness === "ready") return "ready";
  if (state.readiness === "degraded") return "degraded";
  return "starting";
}

function componentStatesFromRuntime(state: RuntimeState): ComponentStates {
  const runtime = componentStatusFromRuntime(state);
  // In M5 the MCP endpoint is hosted by the Runtime process, so RuntimeState is
  // the authority for both components. Tunnel remains unimplemented until M7.
  return { runtime, tunnel: "stopped", mcp: runtime };
}

function componentStatesFromHarness(state: HarnessStatus): ComponentStates {
  return { runtime: state.runtime, tunnel: state.tunnel, mcp: state.mcp };
}

function componentStatesFromProcess(
  event: RuntimeProcessEvent,
  current: ComponentStates,
): ComponentStates {
  switch (event.phase) {
    case "starting":
      return { runtime: "starting", tunnel: "stopped", mcp: "starting" };
    case "running":
      // A process event has no readiness field. Preserve a previously confirmed
      // RuntimeState; otherwise wait for runtime_status before declaring MCP ready.
      return {
        runtime: current.runtime === "ready" || current.runtime === "degraded" ? current.runtime : "starting",
        tunnel: "stopped",
        mcp: current.mcp === "ready" || current.mcp === "degraded" ? current.mcp : "starting",
      };
    case "stopping":
      return { runtime: "stopping", tunnel: "stopped", mcp: "stopping" };
    case "crashed":
      return { runtime: "error", tunnel: "stopped", mcp: "error" };
    case "stopped":
      return { runtime: "stopped", tunnel: "stopped", mcp: "stopped" };
  }
}
export class TauriHarnessControlClient implements HarnessControlClient {
  readonly adapterKind: HarnessAdapterKind = "real";
  private components: ComponentStates = { runtime: "stopped", tunnel: "stopped", mcp: "stopped" };
  private harness: HarnessStatus = {
    lifecycle: "stopped",
    runtime: "stopped",
    tunnel: "stopped",
    mcp: "stopped",
    activeSessionId: null,
    configRevision: null,
    lastError: null,
    tunnelReadiness: null,
  };
  private harnessListener?: Unsubscribe;
  private componentListener?: ComponentStatesListener;

  private call<T>(cmd: string, args?: Record<string, unknown>) {
    return invoke<T>(cmd, args);
  }

  private applyRuntimeState(state: RuntimeState, listener?: ComponentStatesListener) {
    this.components = componentStatesFromRuntime(state);
    listener?.(this.components, state);
  }

  private applyHarnessStatus(state: HarnessStatus) {
    this.harness = state;
    this.components = componentStatesFromHarness(state);
    this.componentListener?.(this.components);
  }

  private async ensureHarnessListener() {
    if (this.harnessListener) return;
    this.harnessListener = await listen<HarnessStatus>("harness:status", (event) => {
      this.applyHarnessStatus(event.payload);
    });
  }

  async configGet() {
    return this.call<AppConfig | null>("config_get");
  }

  configMetadata() {
    return this.call<ConfigMetadata>("config_metadata");
  }

  configSave(config: AppConfig, apiKey?: string) {
    return this.call<AppConfig>("config_save", { config, apiKey: apiKey ?? null });
  }

  configReset() {
    return this.call<void>("config_reset");
  }

  apiKeyStatus() {
    return this.call<boolean>("secret_api_key_status");
  }

  apiKeyDelete() {
    return this.call<void>("secret_api_key_delete");
  }

  chooseWorkspace() {
    return this.call<string | null>("workspace_choose");
  }

  chooseTunnelClient() {
    return this.call<string | null>("tunnel_client_choose");
  }

  chooseBashExecutable() {
    return this.call<string | null>("bash_executable_choose");
  }

  detectTunnelClient() {
    return this.call<string | null>("tunnel_client_detect");
  }

  validatePath(path: string, file: boolean) {
    return this.call<boolean>("validate_path", { path, file });
  }

  async runtimeGetDiagnostics() {
    return parseDiagnostics(await this.call<unknown>("runtime_get_diagnostics"));
  }

  async runtimeGetActivitySnapshot(limit: number) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 256) throw { code: "INVALID_REQUEST" };
    return parseActivity(await this.call<unknown>("runtime_get_activity_snapshot", { limit }));
  }

  async runtimeGetEffectiveTools() {
    return parseTools(await this.call<unknown>("runtime_get_effective_tools"));
  }

  localDiagnosticsRun() {
    return this.call<DiagnosticRun>("local_diagnostics_run");
  }

  runtimeStartEligibility() {
    return this.call<StartEligibility>("runtime_start_eligibility");
  }

  async runtimeStart() {
    this.components = { runtime: "starting", tunnel: "stopped", mcp: "starting" };
    const state = await this.call<RuntimeState>("runtime_start");
    this.applyRuntimeState(state);
    return state;
  }

  async runtimeStatus() {
    const state = await this.call<RuntimeState>("runtime_status");
    this.applyRuntimeState(state);
    return state;
  }

  runtimeProcessStatus() {
    return this.call<RuntimeProcessStatus>("runtime_process_status");
  }

  async runtimeStop() {
    this.components = { runtime: "stopping", tunnel: "stopped", mcp: "stopping" };
    const result = await this.call<RuntimeStopResult>("runtime_stop");
    if (result.accepted) {
      this.components = { runtime: "stopped", tunnel: "stopped", mcp: "stopped" };
    }
    return result;
  }

  harnessStartEligibility() {
    return this.call<StartEligibility>("harness_start_eligibility");
  }

  async harnessStart() {
    const status = await this.call<HarnessStatus>("harness_start");
    this.applyHarnessStatus(status);
    return status;
  }

  async harnessStatus() {
    await this.ensureHarnessListener();
    const status = await this.call<HarnessStatus>("harness_status");
    this.applyHarnessStatus(status);
    return status;
  }

  async harnessStop() {
    const status = await this.call<HarnessStatus>("harness_stop");
    this.applyHarnessStatus(status);
    return status;
  }

  async harnessRestart() {
    const status = await this.call<HarnessStatus>("harness_restart");
    this.applyHarnessStatus(status);
    return status;
  }

  getHarnessStatus() {
    return this.harness;
  }

  getComponentStates() {
    return this.components;
  }

  setComponentStates(states: ComponentStates) {
    this.components = states;
  }

  async componentStatesSubscribe(listener: ComponentStatesListener): Promise<Unsubscribe> {
    this.componentListener = listener;
    const runtimeUnlisten = await listen<RuntimeProcessEvent>("runtime:process", (event) => {
      if (this.harnessListener) return;
      this.components = componentStatesFromProcess(event.payload, this.components);
      listener(this.components, undefined, event.payload);

      if (event.payload.phase === "running") {
        void this.runtimeStatus()
          .then((state) => listener(this.components, state))
          .catch(() => undefined);
      }
    });
    return () => {
      runtimeUnlisten();
      this.harnessListener?.();
      this.harnessListener = undefined;
      this.componentListener = undefined;
    };
  }

  activityList(params: ActivityListParams = {}) {
    return this.call<ActivityListResult>("runtime_activity_list", { params });
  }

  async activitySubscribe(
    listener: ActivityEventListener,
    afterSequence?: string,
  ): Promise<Unsubscribe> {
    const unlisten = await listen<ActivityEvent>("runtime:activity", (event) =>
      listener(event.payload),
    );
    try {
      await this.call("runtime_activity_subscribe", {
        params: afterSequence === undefined ? {} : { afterSequence },
      });
      return unlisten;
    } catch (error) {
      unlisten();
      throw error;
    }
  }

  toolsList() {
    return this.call<ToolsListResult>("runtime_tools_list");
  }

  policiesList() {
    return this.call<PoliciesListResult>("runtime_policies_list");
  }

  approvalsList(params: ApprovalsListParams = {}) {
    return this.call<ApprovalsListResult>("runtime_approvals_list", { params });
  }

  approvalsResolve(requestId: string, decision: ApprovalDecision) {
    return this.call<ApprovalsResolveResult>("runtime_approvals_resolve", {
      params: { requestId, decision },
    });
  }
}

export function createLocalClient(): HarnessControlClient {
  return tauriAvailable() ? new TauriHarnessControlClient() : new MockHarnessControlClient("ready");
}
