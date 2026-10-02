import { useCallback, useMemo, useState } from "react";
import type { AppConfig, ToolConfig, ToolDescriptor } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";

type ImplementedToolKey = Exclude<keyof ToolConfig, "bash">;
type ToolMetadata = Pick<ToolDescriptor, "name" | "description"> & { key: ImplementedToolKey };

function configKeyForRuntimeName(name: string, config: AppConfig): ImplementedToolKey | null {
  const key = name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as keyof ToolConfig;
  return key !== "bash" && key in config.tools ? key : null;
}

function runtimeNameForConfigKey(key: string): string {
  return key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function metadataFromRegistry(items: ToolDescriptor[], config: AppConfig): ToolMetadata[] {
  return items
    .filter((item) => item.implemented)
    .map((item) => {
      const key = configKeyForRuntimeName(item.name, config);
      return key ? { key, name: item.name, description: item.description } : null;
    })
    .filter((item): item is ToolMetadata => item !== null);
}

function metadataFromPersistedConfig(config: AppConfig): ToolMetadata[] {
  return Object.keys(config.tools)
    .filter((key): key is ImplementedToolKey => key !== "bash")
    .map((key) => ({
      key,
      name: runtimeNameForConfigKey(key),
      description: "Built-in configurable tool.",
    }));
}

function effectiveReason(tool: ToolDiagnostic | undefined, running: boolean): string {
  if (!running) return "No active Runtime generation.";
  if (!tool) return "Runtime did not report this tool.";
  switch (tool.availability) {
    case "disabled": return "Disabled by saved Runtime configuration.";
    case "unavailable_by_policy": return "Unavailable by Runtime policy.";
    case "unavailable_by_runtime": return "Runtime capability is unavailable.";
    case "available_with_approval": return "Available after human approval.";
    case "available": return "Available in the current Runtime generation.";
  }
}

function effectiveDecision(tool: ToolDiagnostic | undefined, running: boolean): string {
  if (!running || !tool) return "NOT EVALUATED";
  return tool.availability === "disabled" ? "NOT EVALUATED" : tool.decision.toUpperCase();
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
  const [saving, setSaving] = useState<ImplementedToolKey | null>(null);
  const [saveError, setSaveError] = useState("");
  const [saveMessage, setSaveMessage] = useState("");

  const metadata = useMemo(() => {
    if (!config) return [];
    if ((registry.status === "success" || registry.status === "empty") && registry.data.items.length > 0) {
      return metadataFromRegistry(registry.data.items, config);
    }
    return running ? [] : metadataFromPersistedConfig(config);
  }, [config, registry, running]);

  const onToggle = async (tool: ToolMetadata) => {
    if (!config || saving) return;
    const enabled = config.tools[tool.key].enabled;
    setSaving(tool.key);
    setSaveError("");
    setSaveMessage("");
    const result = await saveConfig({
      ...config,
      tools: {
        ...config.tools,
        [tool.key]: { enabled: !enabled },
      },
    });
    setSaving(null);
    if (!result.ok) {
      setSaveError(result.error?.message ?? "Could not save tool configuration.");
      return;
    }
    setSaveMessage("Desired tool state saved.");
  };

  const legacyMode = !config;
  const effectiveTools = state.status === "success" ? state.data : [];
  const effectiveByName = new Map(effectiveTools.map((tool) => [tool.name, tool]));

  return <div className="page-stack">
    <SectionHeading
      eyebrow="M2 configuration"
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
        : <Card><div className="observation-table-wrap"><table className="observation-table">
          <caption className="muted">Desired state is persistent; effective state belongs to the current Runtime generation.</caption>
          <thead><tr><th scope="col">Tool name</th><th scope="col">Desired</th><th scope="col">Effective</th><th scope="col">Decision</th><th scope="col">Reason</th></tr></thead>
          <tbody>{metadata.map((tool) => {
            const desired = config.tools[tool.key].enabled;
            const effective = effectiveByName.get(tool.name);
            return <tr key={tool.name}>
              <td><span className="mono">{tool.name}</span><br /><span className="muted">{tool.description}</span></td>
              <td><label><input type="checkbox" aria-label={`Desired state for ${tool.name}`} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? "ON" : "OFF"}</label></td>
              <td>{effective?.availability ?? "not_reported"}</td>
              <td>{effectiveDecision(effective, running)}</td>
              <td>{effectiveReason(effective, running)}</td>
            </tr>;
          })}</tbody>
        </table></div></Card>}
      </ControlReadView>
    ) : (
      <Card><div className="observation-table-wrap"><table className="observation-table">
        <caption className="muted">No active Runtime generation. These toggles will apply on the next start.</caption>
        <thead><tr><th scope="col">Tool name</th><th scope="col">Desired</th><th scope="col">Effective</th><th scope="col">Reason</th></tr></thead>
        <tbody>{metadata.map((tool) => {
          const desired = config.tools[tool.key].enabled;
          return <tr key={tool.name}>
            <td><span className="mono">{tool.name}</span><br /><span className="muted">{tool.description}</span></td>
            <td><label><input type="checkbox" aria-label={`Desired state for ${tool.name}`} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? "ON" : "OFF"}</label></td>
            <td>not_running</td>
            <td>{effectiveReason(undefined, false)}</td>
          </tr>;
        })}</tbody>
      </table></div></Card>
    )}
  </div>;
}
