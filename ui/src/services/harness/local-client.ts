import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, DiagnosticRun, RuntimeState } from "@protocol/v1";
import type { ComponentStates, ConfigMetadata, HarnessControlClient, ActivityEventListener, Unsubscribe } from "./client";
import { MockHarnessControlClient } from "./mock-client";

const tauriAvailable = () => typeof window !== "undefined" && Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export class TauriHarnessControlClient implements HarnessControlClient {
  private readonly mock = new MockHarnessControlClient("ready");
  constructor() {
    this.mock.setComponentStates({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });
  }
  private call<T>(cmd: string, args?: Record<string, unknown>) { return invoke<T>(cmd, args); }
  async configGet() {
    const config = await this.call<AppConfig | null>("config_get");
    const apiKeyConfigured = await this.apiKeyStatus();
    this.mock.syncConfig(config, apiKeyConfigured);
    return config;
  }
  configMetadata() { return this.call<ConfigMetadata>("config_metadata"); }
  async configSave(config: AppConfig, apiKey?: string) {
    const saved = await this.call<AppConfig>("config_save", { config, apiKey: apiKey ?? null });
    this.mock.syncConfig(saved, apiKey?.trim() ? true : await this.apiKeyStatus());
    return saved;
  }
  async configReset() {
    await this.call<void>("config_reset");
    this.mock.syncConfig(null, false);
  }
  apiKeyStatus() { return this.call<boolean>("secret_api_key_status"); }
  async apiKeyDelete() {
    await this.call<void>("secret_api_key_delete");
    const config = await this.call<AppConfig | null>("config_get");
    this.mock.syncConfig(config, false);
  }
  chooseWorkspace() { return this.call<string | null>("workspace_choose"); }
  chooseTunnelClient() { return this.call<string | null>("tunnel_client_choose"); }
  detectTunnelClient() { return this.call<string | null>("tunnel_client_detect"); }
  validatePath(path: string, file: boolean) { return this.call<boolean>("validate_path", { path, file }); }
  localDiagnosticsRun() { return this.call<DiagnosticRun>("local_diagnostics_run"); }
  runtimeStart() { return this.mock.runtimeStart(); }
  runtimeStatus() { return this.mock.runtimeStatus(); }
  runtimeStop() { return this.mock.runtimeStop(); }
  getComponentStates() { return this.mock.getComponentStates(); }
  setComponentStates(s: ComponentStates) { this.mock.setComponentStates(s); }
  activityList() { return this.mock.activityList(); }
  activitySubscribe(l: ActivityEventListener) { return this.mock.activitySubscribe(l); }
  toolsList() { return this.mock.toolsList(); }
  policiesList() { return this.mock.policiesList(); }
  approvalsList() { return this.mock.approvalsList(); }
  approvalsResolve(requestId: string, decision: import("@protocol/v1").ApprovalDecision) { return this.mock.approvalsResolve(requestId, decision); }
}

export function createLocalClient(): HarnessControlClient {
  return tauriAvailable() ? new TauriHarnessControlClient() : new MockHarnessControlClient("ready");
}
