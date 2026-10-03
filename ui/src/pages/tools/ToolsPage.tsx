import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppConfig, BashConfig, ToolConfig, ToolDescriptor } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";
import {
  policyDecisionKey,
  presentError,
  toolAvailabilityKey,
  toolReadinessKey,
  type ErrorPresentation,
  type TranslationKey,
} from "@/i18n/presentation";

type ImplementedToolKey = keyof ToolConfig;
type ToolMetadata = Pick<ToolDescriptor, "name" | "description"> & { key: ImplementedToolKey };
const defaultBashConfig: BashConfig = { autoDetect: true };

const builtInToolDescriptionKeys: Record<string, TranslationKey> = {
  read_file: "tools.builtInDescriptions.readFile",
  glob: "tools.builtInDescriptions.glob",
  write: "tools.builtInDescriptions.write",
  edit: "tools.builtInDescriptions.edit",
  bash: "tools.builtInDescriptions.bash",
};

const availabilityReasonKeys: Record<ToolDiagnostic["availability"], TranslationKey> = {
  available: "tools.availabilityReasons.available",
  available_with_approval: "tools.availabilityReasons.availableWithApproval",
  unavailable_by_mode: "tools.availabilityReasons.unavailableByMode",
  unavailable_by_policy: "tools.availabilityReasons.unavailableByPolicy",
  unavailable_by_runtime: "tools.availabilityReasons.unavailableByRuntime",
  disabled: "tools.availabilityReasons.disabled",
};

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
      description: "",
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
      description: "",
    }));
}

function effectiveReason(tool: ToolDiagnostic | undefined, running: boolean): { key: TranslationKey; detail?: string } {
  if (!running) return { key: "tools.availabilityReasons.noActiveRuntime" };
  if (!tool) return { key: "tools.availabilityReasons.notReported" };
  return { key: availabilityReasonKeys[tool.availability], detail: tool.reason };
}

function effectiveDecision(tool: ToolDiagnostic | undefined, running: boolean, translate: (key: string) => string): string {
  if (!running || !tool) return translate("tools.notEvaluated");
  // Mode, disabled, and Runtime-unavailable states are eligibility failures;
  // do not render the Runtime's fail-closed DENY as their cause.
  if (tool.availability === "disabled" || tool.availability === "unavailable_by_mode" || tool.availability === "unavailable_by_runtime") return translate("tools.notEvaluated");
  return translate(policyDecisionKey(tool.decision));
}

function availabilityLabel(tool: ToolDiagnostic | undefined, running: boolean, translate: (key: string) => string): string {
  if (!running || !tool) return translate("tools.notReported");
  return translate(toolAvailabilityKey(tool.availability));
}

function LegacyEffectiveTools({ tools }: { tools: ToolDiagnostic[] }) {
  const { t } = useTranslation();
  return tools.length === 0
    ? <Card><strong>{t("tools.empty.title")}</strong><p className="muted">{t("tools.empty.description")}</p></Card>
    : <Card><div className="observation-table-wrap"><table className="observation-table">
      <caption className="muted">{t("tools.table.legacyCaption")}</caption>
      <thead><tr><th scope="col">{t("tools.table.toolName")}</th><th scope="col">{t("tools.table.availability")}</th><th scope="col">{t("tools.table.decision")}</th><th scope="col">{t("tools.table.reason")}</th></tr></thead>
      <tbody>{tools.map((tool) => <tr key={tool.name}>
        <td className="mono">{tool.name}</td><td>{t(toolAvailabilityKey(tool.availability))}</td><td>{t(policyDecisionKey(tool.decision))}</td><td>{t("tools.notReportedByControlPlane")}</td>
      </tr>)}</tbody>
    </table></div></Card>;
}

export function ToolsPage() {
  const { t } = useTranslation();
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
  const [saveError, setSaveError] = useState<ErrorPresentation | null>(null);
  const [saveMessageKey, setSaveMessageKey] = useState<TranslationKey | null>(null);
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

  const saveDesired = async (next: AppConfig, key: string, messageKey: TranslationKey = "tools.messages.configSaved") => {
    if (saving) return false;
    setSaving(key);
    setSaveError(null);
    setSaveMessageKey(null);
    try {
      const result = await saveConfig(next);
      if (!result.ok) {
        setSaveError(presentError(result.error, result.error?.message));
        return false;
      }
      setSaveMessageKey(messageKey);
      return true;
    } catch (error) {
      setSaveError(presentError(error));
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
    }, `tool:${tool.key}`, "tools.messages.toolSaved");
  };

  const saveBash = async (next: BashConfig) => {
    if (!config || saving) return;
    const previous = bashDraft;
    setBashDraft(next);
    const saved = await saveDesired({ ...config, bash: next }, "bash:config", "tools.messages.bashSaved");
    if (!saved) setBashDraft(previous);
  };

  const chooseBash = async () => {
    if (!config || saving || typeof client.chooseBashExecutable !== "function") return;
    setSaveError(null);
    try {
      const path = await client.chooseBashExecutable();
      if (!path) return;
      await saveBash({ ...bashDraft, executablePath: path });
    } catch {
      setSaveError({ summaryKey: "tools.errors.chooseBash" });
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

  const toolDescription = (tool: ToolMetadata) => {
    const key = builtInToolDescriptionKeys[tool.name];
    return key ? t(key) : tool.description || t("tools.builtInGeneric");
  };
  const technicalDetail = (value?: string) => value
    ? <><br /><span className="muted mono">{t("tools.technicalDetail")}: {value}</span></>
    : null;
  const reason = (tool: ToolDiagnostic | undefined) => {
    const presentation = effectiveReason(tool, running);
    return <>{t(presentation.key)}{technicalDetail(presentation.detail)}</>;
  };
  const readiness = (tool: ToolDiagnostic | undefined) => tool?.readiness
    ? t(toolReadinessKey(tool.readiness))
    : t("tools.notReported");

  return <div className="page-stack">
    <SectionHeading
      eyebrow={t("tools.eyebrow")}
      title={t("tools.title")}
      description={legacyMode
        ? t("tools.descriptionLegacy")
        : t("tools.description")}
      action={<button className="button button-secondary" onClick={refresh} disabled={!running}>{t("tools.refresh")}</button>}
    />

    {restartRequired && <div className="notice notice-warning" role="status">
      <strong>{t("tools.restartRequired")}</strong>
      <span>{t("tools.restartDescription")}</span>
    </div>}
    {saveError && <div className="notice notice-danger" role="alert"><strong>{t("tools.saveFailed")}</strong><span>{t(saveError.summaryKey)}</span>{saveError.detail && <span className="muted mono">{t("tools.technicalDetail")}: {saveError.detail}</span>}</div>}
    {saveMessageKey && <div className="notice notice-success" role="status"><span>{t(saveMessageKey)}</span></div>}

    {legacyMode ? (
      <ControlReadView state={state}>{(data) => <LegacyEffectiveTools tools={data} />}</ControlReadView>
    ) : running ? (
      <ControlReadView state={state}>{(data) => data.length === 0
        ? <Card><strong>{t("tools.empty.title")}</strong><p className="muted">{t("tools.empty.description")}</p></Card>
        : <>
          <Card><div className="observation-table-wrap"><table className="observation-table">
            <caption className="muted">{t("tools.table.runningCaption")}</caption>
            <thead><tr><th scope="col">{t("tools.table.toolName")}</th><th scope="col">{t("tools.table.desired")}</th><th scope="col">{t("tools.table.effective")}</th><th scope="col">{t("tools.table.decision")}</th><th scope="col">{t("tools.table.reason")}</th></tr></thead>
            <tbody>{metadata.map((tool) => {
              const desired = config.tools[tool.key].enabled;
              const effective = effectiveByName.get(tool.name);
              if (tool.key === "bash") {
                return <tr key={tool.name}>
                  <td><span className="mono">{tool.name}</span><br /><span className="muted">{toolDescription(tool)}</span></td>
                  <td><label><input type="checkbox" aria-label={t("tools.desiredStateFor", { tool: tool.name })} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? t("tools.on") : t("tools.off")}</label></td>
                  <td>{availabilityLabel(effective, running, t)}<br /><span className="muted">{t("tools.table.readiness")}: {readiness(effective)}</span></td>
                  <td>{effectiveDecision(effective, running, t)}</td>
                  <td>{reason(effective)}{technicalDetail(effective?.readinessReason)}</td>
                </tr>;
              }
              return <tr key={tool.name}>
                <td><span className="mono">{tool.name}</span><br /><span className="muted">{toolDescription(tool)}</span></td>
                <td><label><input type="checkbox" aria-label={t("tools.desiredStateFor", { tool: tool.name })} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? t("tools.on") : t("tools.off")}</label></td>
                <td>{availabilityLabel(effective, running, t)}</td>
                <td>{effectiveDecision(effective, running, t)}</td>
                <td>{reason(effective)}</td>
              </tr>;
            })}</tbody>
          </table></div></Card>
          <Card>
            <h3>{t("tools.bash.title")}</h3>
            <p className="muted">{t("tools.bash.runningDescription")}</p>
            <div className="form-grid">
              <label className="form-field"><span>{t("tools.bash.autoDetect")}</span><input type="checkbox" aria-label={t("tools.bash.autoDetect")} checked={bashDraft.autoDetect} disabled={saving !== null} onChange={() => void toggleAutoDetect()} /></label>
              <label className="form-field"><span>{t("tools.bash.manualExecutable")}</span><div className="input-action-row"><input className="path-input" aria-label={t("tools.bash.manualExecutable")} value={bashDraft.executablePath ?? ""} readOnly placeholder={t("tools.bash.noOverride")} title={bashDraft.executablePath || undefined} /><button className="button button-secondary" onClick={() => void chooseBash()} disabled={saving !== null}>{t("tools.bash.choose")}</button><button className="button button-secondary" onClick={() => void clearBashOverride()} disabled={saving !== null || !bashDraft.executablePath}>{t("tools.bash.clear")}</button></div></label>
            </div>
            <p className="muted">{t("tools.bash.noSandboxWarning")} {t("tools.bash.boundary")}</p>
            {bashEffective?.reason && <p className="muted">{t("tools.bash.runtimeDetail")}: {bashEffective.reason}</p>}
          </Card>
        </>}
      </ControlReadView>
    ) : (
      <>
        <Card><div className="observation-table-wrap"><table className="observation-table">
          <caption className="muted">{t("tools.table.stoppedCaption")}</caption>
          <thead><tr><th scope="col">{t("tools.table.toolName")}</th><th scope="col">{t("tools.table.desired")}</th><th scope="col">{t("tools.table.effective")}</th><th scope="col">{t("tools.table.reason")}</th></tr></thead>
          <tbody>{metadata.map((tool) => {
            const desired = config.tools[tool.key].enabled;
            return <tr key={tool.name}>
              <td><span className="mono">{tool.name}</span><br /><span className="muted">{toolDescription(tool)}</span></td>
              <td><label><input type="checkbox" aria-label={t("tools.desiredStateFor", { tool: tool.name })} checked={desired} disabled={saving !== null} onChange={() => void onToggle(tool)} /> {desired ? t("tools.on") : t("tools.off")}</label></td>
              <td>{t("tools.notRunning")}</td>
              <td>{t("tools.availabilityReasons.noActiveRuntime")}</td>
            </tr>;
          })}</tbody>
        </table></div></Card>
        <Card>
          <h3>{t("tools.bash.title")}</h3>
          <p className="muted">{t("tools.bash.stoppedDescription")}</p>
          <div className="form-grid">
            <label className="form-field"><span>{t("tools.bash.autoDetect")}</span><input type="checkbox" aria-label={t("tools.bash.autoDetect")} checked={bashDraft.autoDetect} disabled={saving !== null} onChange={() => void toggleAutoDetect()} /></label>
            <label className="form-field"><span>{t("tools.bash.manualExecutable")}</span><div className="input-action-row"><input className="path-input" aria-label={t("tools.bash.manualExecutable")} value={bashDraft.executablePath ?? ""} readOnly placeholder={t("tools.bash.noOverride")} title={bashDraft.executablePath || undefined} /><button className="button button-secondary" onClick={() => void chooseBash()} disabled={saving !== null}>{t("tools.bash.choose")}</button><button className="button button-secondary" onClick={() => void clearBashOverride()} disabled={saving !== null || !bashDraft.executablePath}>{t("tools.bash.clear")}</button></div></label>
          </div>
          <p className="muted">{t("tools.bash.noSandboxWarning")} {t("tools.bash.boundary")}</p>
        </Card>
      </>
    )}
  </div>;
}
