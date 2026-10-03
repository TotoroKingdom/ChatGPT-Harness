import { useCallback, useEffect, useMemo, useState } from "react";
import type { AppConfig, BashConfig, ToolConfig, ToolDescriptor } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";

type ImplementedToolKey = keyof ToolConfig;
type ToolMetadata = Pick<ToolDescriptor, "name" | "description"> & { key: ImplementedToolKey };
const defaultBashConfig: BashConfig = { autoDetect: true };

function configKeyForRuntimeName(name: string, config: AppConfig): ImplementedToolKey | null {
  const key = name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as keyof ToolConfig;
  return key in config.tools ? key : null;
}

function runtimeNameForConfigKey(key: string): string {
  return key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function metadataFromRegistry(items: ToolDescriptor[], config: AppConfig): ToolMetadata[] {
  const metadata = items
    .filter((item) => item.implemented)
    .map((item) => {
      const key = configKeyForRuntimeName(item.name, config);
      return key ? { key, name: item.name, description: item.description } : null;
    })
    .filter((item): item is ToolMetadata => item !== null);
  // A stale M4 registry can omit Bash even though the desired v1 config has
  // the additive field. Keep the formal row visible while leaving effective
  // availability as an explicit Runtime observation (or "Not reported").
  if (!metadata.some((item) => item.key === "bash") && "bash" in config.tools) {
    metadata.push({
      key: "bash",
      name: "bash",
      description: "Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox.",
    });
  }
  return metadata;
}

function metadataFromPersistedConfig(config: AppConfig): ToolMetadata[] {
  return Object.keys(config.tools)
    .filter((key): key is ImplementedToolKey => key in config.tools)
    .map((key) => ({
      key,
      name: runtimeNameForConfigKey(key),
      description: key === "bash"
        ? "Run a controlled Git Bash command from the active Workspace. The cwd is an initial working directory, not a filesystem sandbox."
        : "Built-in configurable tool.",
    }));
}

function effectiveReason(tool: ToolDiagnostic | undefined, running: boolean): string {
  if (!running) return "No active Runtime generation.";
  if (!tool) return "Runtime did not report this tool.";
  if (tool.reason) return tool.reason;
  switch (tool.availability) {
    case "disabled": return "Disabled by saved Runtime configuration.";
    case "unavailable_by_mode": return "Unavailable by mode.";
    case "unavailable_by_policy": return "Unavailable by Runtime policy.";
    case "unavailable_by_runtime": return "Runtime capability is unavailable.";
    case "available_with_approval": return "Available after human approval.";
    case "available": return "Available in the current Runtime generation.";
  }
}

function effectiveDecision(tool: ToolDiagnostic | undefined, running: boolean): string {
  if (!running || !tool) return "NOT EVALUATED";
  // Mode, disabled, and Runtime-unavailable states are eligibility failures;
  // do not render the Runtime's fail-closed DENY as their cause.
  if (tool.availability === "disabled" || tool.availability === "unavailable_by_mode" || tool.availability === "unavailable_by_runtime") return "NOT EVALUATED";
  return tool.decision.toUpperCase();
}

function bashAvailability(tool: ToolDiagnostic | undefined, running: boolean): string {
  if (!running || !tool) return "Not reported";
  switch (tool.availability) {
    case "unavailable_by_mode": return "Unavailable by mode";
    case "unavailable_by_policy": return "Unavailable by Runtime policy";
    case "unavailable_by_runtime": return "Unavailable by Runtime";
    case "available_with_approval": return "Available with approval";
    case "available": return "Available";
    case "disabled": return "Disabled";
  }
}

function LegacyEffectiveTools({ tools }: { tools: ToolDiagnostic[] }) {
  return tools.length === 0
    ? <Card><strong>No tools reported</strong><p className="muted">The Control Plane returned an empty catalog, not a readiness assertion.</p></Card>
    : <Card><div className="observation-table-wrap"><table className="observation-table">
      <caption className="muted">Read-only catalog snapshot · refresh for current observations</caption>
      <thead><tr><th scope="col">Tool name</th><th scope="col">Availability</th><th scope="col">Decision</th><th scope="col">Reason</th></tr></thead>
      <tbody>{tools.map((tool) => <tr key={tool.name}>
        <td className="mono">{tool.name}</td><td>{tool.availability}</td><td>{tool.decision.toUpperCase()}</td><td>Not provided by Control Plane</td>
      </tr>)}</tbody>
    </table></div></Card>;
}

export function ToolsPage() {
  const {
    client,
    config,
    tools: registry,
    components,
    restartRequired,
    saveConfig,
  } = useHarnessData();
  const running = components.runtime === "ready" || components.runtime === "degraded";
  const read = useCallback(() => client.runtimeGetEffectiveTools(), [client]);
  const { state, refresh } = useControlRead(read, running);
  const [saving, setSaving] = useState<string | null>(null);
  const [saveError, setSaveError] = useState("");
  const [saveMessage, setSaveMessage] = useState("");
  const [bashDraft, setBashDraft] = useState<BashConfig>(defaultBashConfig);

  useEffect(() => {
    setBashDraft(config?.bash ?? defaultBashConfig);
  }, [config]);

  const metadata = useMemo(() => {
    if (!config) return [];
    if ((registry.status === "success" || registry.status === "empty") && registry.data.items.length > 0) {
      return metadataFromRegistry(registry.data.items, config);
    }
    return running ? [] : metadataFromPersistedConfig(config);
  }, [config, registry, running]);

  const saveDesired = async (next: AppConfig, key: string, message = "Desired configuration saved.") => {
    if (saving) return false;
    setSaving(key);
    setSaveError("");
    setSaveMessage("");
    try {
      const result = await saveConfig(next);
      if (!result.ok) {
        setSaveError(result.error?.message ?? "Could not save desired configuration.");
        return false;
      }
      setSaveMessage(message);
      return true;
    } catch {
      setSaveError("Could not save desired configuration.");
      return false;
    } finally {
      setSaving(null);
    }
  };

  const onToggle = async (tool: ToolMetadata) => {
    if (!config || saving) return;
    const enabled = config.tools[tool.key].enabled;
    await saveDesired({
      ...config,
      tools: {
        ...config.tools,
        [tool.key]: { enabled: !enabled },
      },
    }, `tool:${tool.key}`, "Desired tool state saved.");
  };

  const saveBash = async (next: BashConfig) => {
    if (!config || saving) return;
    const previous = bashDraft;
    setBashDraft(next);
    const saved = await saveDesired({ ...config, bash: next }, "bash:config", "Desired Bash configuration saved.");
    if (!saved) setBashDraft(previous);
  };

  const chooseBash = async () => {
    if (!config || saving || typeof client.chooseBashExecutable !== "function") return;
    setSaveError("");
    try {
      const path = await client.chooseBashExecutable();
      if (!path) return;
      await saveBash({ ...bashDraft, executablePath: path });
    } catch {
      setSaveError("Could not choose a Git Bash executable.");
    }
  };

  const clearBashOverride = async () => {
    await saveBash({ ...bashDraft, executablePath: undefined });
  };

  const toggleAutoDetect = async () => {
    await saveBash({ ...bashDraft, autoDetect: !bashDraft.autoDetect });
  };

  const legacyMode = !config;
  const effectiveTools = state.status === "success" ? state.data : [];
  const effectiveByName = new Map(effectiveTools.map((tool) => [tool.name, tool]));
  const bashEffective = effectiveByName.get("bash");

  return <div className="page-stack">
    <SectionHeading
      eyebrow="M5 controlled Bash"
      title="Tools"
      description={legacyMode
        ? "Availability and decisions are reported by Runtime, never inferred by this UI."
        : "Set the next Runtime generation's desired tool state. Current effective state stays immutable until restart."}
      action={<button className="button button-secondary" onClick={refresh} disabled={!running}>Refresh tools</button>}
    />

    {restartRequired && <div className="notice notice-warning" role="status">
      <strong>Restart Required</strong>
      <span>Saved desired tool state differs from the active Runtime generation.</span>
    </div>}
    {saveError && <div className="notice notice-danger" role="alert"><strong>Save failed</strong><span>{saveError}</span></div>}
    {saveMessage && <div className="notice notice-success" role="status"><span>{saveMessage}</span></div>}

    {legacyMode ? (
      <ControlReadView state={state}>{(data) => <LegacyEffectiveTools tools={data} />}</ControlReadView>
    ) : running ? (
      <ControlReadView state={state}>{(data) => data.length === 0
        ? <Card><strong>No tools reported</strong><p className="muted">The Control Plane returned an empty catalog, not a readiness assertion.</p></Card>
        : <>
          <Card><div className="observation-table-wrap"><table className="observation-table">
            <caption className="muted">Desired state is persistent; effective state belongs to the current Runtime generation.</caption>
            <thead><tr><th scope="col">Tool name</th><th scope="col">Desired</th><th scope="col">Effective</th><th scope="col">Decision</th><th scope="col">Reason</th></tr></thead>
            <tbody>{metadata.map((tool) => {
              const desired = config.tools[tool.key].enabled;
              const effective = effectiveByName.get(tool.name);
              if (tool.key === "bash") {
                return <tr key={tool.name}>
                  <td><span className="mono">{tool.name}</span><br /><span className="muted">{tool.description}</span></td>
                  <td><label><input type="checkbox" aria-label="Desired state for bash" checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? "ON" : "OFF"}</label></td>
                  <td>{bashAvailability(effective, running)}<br /><span className="muted">Readiness: {effective?.readiness ?? "not reported"}</span></td>
                  <td>{effectiveDecision(effective, running)}</td>
                  <td>{effectiveReason(effective, running)}{effective?.readinessReason && <><br /><span className="muted">Remediation: {effective.readinessReason}</span></>}</td>
                </tr>;
              }
              return <tr key={tool.name}>
                <td><span className="mono">{tool.name}</span><br /><span className="muted">{tool.description}</span></td>
                <td><label><input type="checkbox" aria-label={`Desired state for ${tool.name}`} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? "ON" : "OFF"}</label></td>
                <td>{effective?.availability ?? "not_reported"}</td>
                <td>{effectiveDecision(effective, running)}</td>
                <td>{effectiveReason(effective, running)}</td>
              </tr>;
            })}</tbody>
          </table></div></Card>
          <Card>
            <h3>Git Bash configuration</h3>
            <p className="muted">These controls save desired configuration only. A manual override takes precedence and is validated exactly; Auto Detect applies only when no manual path is configured. Runtime validates the selected executable when the next generation starts.</p>
            <div className="form-grid">
              <label className="form-field"><span>Auto detect Git Bash</span><input type="checkbox" aria-label="Auto detect Git Bash" checked={bashDraft.autoDetect} disabled={saving !== null} onChange={() => void toggleAutoDetect()} /></label>
              <label className="form-field"><span>Manual Git Bash executable</span><div className="input-action-row"><input className="path-input" aria-label="Manual Git Bash executable" value={bashDraft.executablePath ?? ""} readOnly placeholder="No manual override" title={bashDraft.executablePath || undefined} /><button className="button button-secondary" onClick={() => void chooseBash()} disabled={saving !== null}>Choose bash.exe</button><button className="button button-secondary" onClick={() => void clearBashOverride()} disabled={saving !== null || !bashDraft.executablePath}>Clear override</button></div></label>
            </div>
            <p className="muted">Bash uses the active Workspace as its initial working directory. It can still access files outside that directory and the network; Policy, Approval, hard limits, and process ownership are the controls.</p>
            {bashEffective?.reason && <p className="muted">Runtime readiness: {bashEffective.reason}</p>}
          </Card>
        </>}
      </ControlReadView>
    ) : (
      <>
        <Card><div className="observation-table-wrap"><table className="observation-table">
          <caption className="muted">No active Runtime generation. These toggles will apply on the next start.</caption>
          <thead><tr><th scope="col">Tool name</th><th scope="col">Desired</th><th scope="col">Effective</th><th scope="col">Reason</th></tr></thead>
          <tbody>{metadata.map((tool) => {
            const desired = config.tools[tool.key].enabled;
            return <tr key={tool.name}>
              <td><span className="mono">{tool.name}</span><br /><span className="muted">{tool.description}</span></td>
              <td><label><input type="checkbox" aria-label={`Desired state for ${tool.name}`} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? "ON" : "OFF"}</label></td>
              <td>not_running</td>
              <td>No active Runtime generation.</td>
            </tr>;
          })}</tbody>
        </table></div></Card>
        <Card>
          <h3>Git Bash configuration</h3>
          <p className="muted">Desired configuration is saved for the next Runtime generation. A manual override takes precedence and is validated exactly; Auto Detect applies only when no manual path is configured. Current readiness is unavailable until Runtime reports it.</p>
          <div className="form-grid">
            <label className="form-field"><span>Auto detect Git Bash</span><input type="checkbox" aria-label="Auto detect Git Bash" checked={bashDraft.autoDetect} disabled={saving !== null} onChange={() => void toggleAutoDetect()} /></label>
            <label className="form-field"><span>Manual Git Bash executable</span><div className="input-action-row"><input className="path-input" aria-label="Manual Git Bash executable" value={bashDraft.executablePath ?? ""} readOnly placeholder="No manual override" title={bashDraft.executablePath || undefined} /><button className="button button-secondary" onClick={() => void chooseBash()} disabled={saving !== null}>Choose bash.exe</button><button className="button button-secondary" onClick={() => void clearBashOverride()} disabled={saving !== null || !bashDraft.executablePath}>Clear override</button></div></label>
          </div>
          <p className="muted">Bash uses the active Workspace as its initial working directory. It can still access files outside that directory and the network; Policy, Approval, hard limits, and process ownership are the controls.</p>
        </Card>
      </>
    )}
  </div>;
}
