import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppConfig, BashConfig, PolicyDecision, ToolConfig, ToolDescriptor } from "@protocol/v1";
import { useHarnessData, type SaveResult } from "@/app/providers/HarnessDataProvider";
import { useControlRead, type ReadState } from "@/app/providers/useControlRead";
import {
  Disclosure,
  InlineFeedback,
  Inspector,
  Row,
  Section,
  SectionHeading,
  Segmented,
  StatusBadge,
  Switch,
  Button,
} from "@/components/common/primitives";
import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import {
  toolAvailabilityKey,
  toolRemediationKey,
  toolStatusReasonKey,
  presentError,
  errorCodeKey,
  type ErrorPresentation,
  type TranslationKey,
} from "@/i18n/presentation";
import { currentToolState, enabledMismatch } from "@/i18n/state-presentation";

type ImplementedToolKey = keyof ToolConfig;
type ToolMetadata = Pick<ToolDescriptor, "name" | "description"> & { key: ImplementedToolKey };
type Feedback =
  | { status: "saving" }
  | { status: "saved"; refreshUnconfirmed?: boolean }
  | { status: "error"; error: ErrorPresentation };

const defaultBashConfig: BashConfig = { autoDetect: true };
const policyChoices: { value: PolicyDecision; labelKey: TranslationKey }[] = [
  { value: "allow", labelKey: "enum.policyDecision.allow" },
  { value: "ask", labelKey: "enum.policyDecision.ask" },
  { value: "deny", labelKey: "enum.policyDecision.deny" },
];

const builtInToolDescriptionKeys: Record<string, TranslationKey> = {
  read_file: "tools.builtInDescriptions.readFile",
  glob: "tools.builtInDescriptions.glob",
  grep: "tools.builtInDescriptions.grep",
  write: "tools.builtInDescriptions.write",
  edit: "tools.builtInDescriptions.edit",
  bash: "tools.builtInDescriptions.bash",
};

function configKeyForRuntimeName(name: string, config: AppConfig): ImplementedToolKey | null {
  const key = name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as ImplementedToolKey;
  return key in config.tools ? key : null;
}

function runtimeNameForConfigKey(key: string): string {
  return key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/** Keep rows based on saved config even when registry/control reads are stale or unavailable. */
function metadataFor(config: AppConfig | null, registry: { status: string; data?: { items: ToolDescriptor[] } }): ToolMetadata[] {
  const byKey = new Map<ImplementedToolKey, ToolMetadata>();
  if (config && (registry.status === "success" || registry.status === "empty")) {
    for (const item of registry.data?.items ?? []) {
      if (!item.implemented) continue;
      const key = configKeyForRuntimeName(item.name, config);
      if (key) byKey.set(key, { key, name: item.name, description: item.description });
    }
  }
  if (config) {
    for (const key of Object.keys(config.tools) as ImplementedToolKey[]) {
      if (!byKey.has(key)) byKey.set(key, { key, name: runtimeNameForConfigKey(key), description: "" });
    }
  } else if (registry.status === "success" || registry.status === "empty") {
    for (const item of registry.data?.items ?? []) {
      if (item.implemented) {
        const key = item.name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()) as ImplementedToolKey;
        byKey.set(key, { key, name: item.name, description: item.description });
      }
    }
  }
  return [...byKey.values()];
}

function statusTone(tool: ToolDiagnostic | undefined): "success" | "warning" | "danger" | "neutral" {
  if (!tool) return "neutral";
  if (tool.availability === "available") return "success";
  if (tool.availability === "available_with_approval") return "warning";
  if (tool.availability === "unavailable_by_policy" || tool.availability === "disabled") return "danger";
  return "neutral";
}

function toolDecisionIsDifferent(tool: ToolDiagnostic | undefined, saved: PolicyDecision | undefined): boolean {
  return Boolean(tool && saved && tool.decision !== saved);
}

function isReadEnabled(status: string): boolean {
  return status === "ready" || status === "degraded";
}

function errorMessage(error: ErrorPresentation, t: (key: string) => string) {
  return <>{t(error.summaryKey)}{error.detail && <span className="muted mono"> · {error.detail}</span>}</>;
}

function FeedbackView({ feedback, t }: { feedback: Feedback | undefined; t: (key: string) => string }) {
  if (!feedback) return null;
  if (feedback.status === "saving") return <InlineFeedback>{t("m8Tools.saving")}</InlineFeedback>;
  if (feedback.status === "error") return <InlineFeedback error>{errorMessage(feedback.error, t)}</InlineFeedback>;
  return <InlineFeedback>{t(feedback.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved")}</InlineFeedback>;
}

function RuntimeEvidence({ state, component, tool, t }: {
  state: ReadState<ToolDiagnostic[]>;
  component: Parameters<typeof currentToolState>[0];
  tool?: ToolDiagnostic;
  t: (key: string, options?: Record<string, unknown>) => string;
}) {
  const presentationState = component === "error" && state.status === "loading"
    ? { status: "error" as const, error: { code: "CONTROL_READ_FAILED" as const } }
    : state;
  const key = currentToolState(component, presentationState, tool);
  const label = t(key);
  if (tool?.availability === "unavailable_by_policy") {
    return <span className="status-group"><StatusBadge tone="danger">{t("m7.states.denied")}</StatusBadge><span className="muted">{label}</span></span>;
  }
  return <StatusBadge tone={statusTone(tool)}>{label}</StatusBadge>;
}

export function ToolsPage() {
  const { t } = useTranslation();
  const data = useHarnessData();
  const { client, config, tools: registry, components, mutateConfig } = data;
  const readEnabled = isReadEnabled(components.runtime);
  const read = useCallback(() => client.runtimeGetEffectiveTools(), [client]);
  const { state, refresh } = useControlRead(read, readEnabled);
  const [selectedKey, setSelectedKey] = useState<ImplementedToolKey | null>(null);
  const [feedback, setFeedback] = useState<Record<string, Feedback>>({});
  const [bashDraft, setBashDraft] = useState<BashConfig>(defaultBashConfig);

  useEffect(() => {
    setBashDraft(config?.bash ?? defaultBashConfig);
  }, [config]);

  const metadata = useMemo(() => metadataFor(config, registry), [config, registry]);
  const effectiveTools = state.status === "success" ? state.data : [];
  const effectiveByName = useMemo(() => new Map(effectiveTools.map((tool) => [tool.name, tool])), [effectiveTools]);
  const selectedTool = metadata.find((tool) => tool.key === selectedKey) ?? null;
  const runningOrObserved = components.runtime === "ready" || components.runtime === "degraded";

  const setFeedbackFor = (key: string, next: Feedback | undefined) => {
    setFeedback((current) => {
      const updated = { ...current };
      if (next) updated[key] = next;
      else delete updated[key];
      return updated;
    });
  };

  const runIntent = async (feedbackKey: string, intent: (latest: AppConfig) => AppConfig): Promise<SaveResult | null> => {
    if (!config || feedback[feedbackKey]?.status === "saving") return null;
    setFeedbackFor(feedbackKey, { status: "saving" });
    try {
      // T2's coordinator is the authoritative path. The callback is applied
      // against the latest persisted config at queue execution time.
      const result = await mutateConfig(intent);
      if (!result.ok) {
        setFeedbackFor(feedbackKey, { status: "error", error: presentError(result.error, result.error?.message) });
      } else {
        setFeedbackFor(feedbackKey, { status: "saved", refreshUnconfirmed: result.refreshUnconfirmed });
      }
      return result;
    } catch (error) {
      setFeedbackFor(feedbackKey, { status: "error", error: presentError(error) });
      return { ok: false };
    }
  };

  const saveToolEnabled = (tool: ToolMetadata) => {
    const nextEnabled = !enabledFor(tool);
    return runIntent(`tool:${tool.key}:enabled`, (latest) => ({
      ...latest,
      tools: { ...latest.tools, [tool.key]: { ...latest.tools[tool.key], enabled: nextEnabled } },
    }));
  };

  const saveToolDefault = (tool: ToolMetadata, value: PolicyDecision) => runIntent(`tool:${tool.key}:default`, (latest) => ({
    ...latest,
    policies: { ...latest.policies, toolDefaults: { ...latest.policies.toolDefaults, [tool.key]: value } },
  }));

  const saveBash = async (patch: Partial<BashConfig>) => {
    if (!config || feedback["bash:config"]?.status === "saving") return;
    const previous = bashDraft;
    const next = { ...bashDraft, ...patch };
    setBashDraft(next);
    const result = await runIntent("bash:config", (latest) => ({ ...latest, bash: { ...latest.bash, ...patch } }));
    if (!result?.ok) setBashDraft(previous);
  };

  const chooseBash = async () => {
    if (!config || feedback["bash:config"]?.status === "saving" || typeof client.chooseBashExecutable !== "function") return;
    try {
      const path = await client.chooseBashExecutable();
      // A cancelled picker has no mutation and no success feedback.
      if (!path) return;
      await saveBash({ executablePath: path });
    } catch (error) {
      setFeedbackFor("bash:config", { status: "error", error: presentError(error) });
    }
  };

  const clearBash = () => saveBash({ executablePath: undefined });
  const toggleBashAutoDetect = () => saveBash({ autoDetect: !bashDraft.autoDetect });

  const description = (tool: ToolMetadata) => {
    const key = builtInToolDescriptionKeys[tool.name];
    return key ? t(key) : tool.description || t("tools.builtInGeneric");
  };

  const policyFor = (tool: ToolMetadata): PolicyDecision | undefined => config?.policies.toolDefaults[tool.key];
  const enabledFor = (tool: ToolMetadata): boolean => config?.tools[tool.key]?.enabled ?? false;

  const runtimeDetail = (tool: ToolMetadata, effective?: ToolDiagnostic) => {
    if (!effective && state.status === "success" && readEnabled) return t("m8Tools.runtimeNotReported");
    const mismatch = enabledMismatch(enabledFor(tool), effective);
    if (mismatch === "offStillAvailable") return t("m8Tools.mismatchOffStillAvailable");
    if (mismatch === "onStillDisabled") return t("m8Tools.mismatchOnStillDisabled");
    const saved = policyFor(tool);
    if (effective && ["available", "available_with_approval", "unavailable_by_policy"].includes(effective.availability) && toolDecisionIsDifferent(effective, saved)) return t("m8Tools.policyDiff", { saved: saved ? t(`enum.policyDecision.${saved}`) : t("m8Tools.notAvailable"), current: t(`enum.policyDecision.${effective.decision}`) });
    if (data.restartRequired && effective) return t("m8Tools.restartPending");
    if (data.restartRequired && !effective) return t("m8Tools.runtimeValuePending");
    return null;
  };

  const detailFor = (tool: ToolMetadata, effective?: ToolDiagnostic) => {
    const mismatch = runtimeDetail(tool, effective);
    return mismatch ? <span className="row-summary">{mismatch}</span> : undefined;
  };

  return <div className="page-stack">
    <h2 className="sr-only">{t("tools.title")}</h2>
    <div className="page-summary-strip">
      <div className="page-summary-main">
        <div><strong>{t("tools.summaryTitle")}</strong><p>{t("m8Tools.description")}</p></div>
        <span className="summary-fact"><b>{metadata.filter((tool) => enabledFor(tool)).length}</b>{t("tools.summaryEnabled")}</span>
        <span className="summary-fact"><b>{metadata.length}</b>{t("tools.summaryTotal")}</span>
      </div>
      <Button variant="secondary" onClick={refresh} disabled={state.status === "loading"}>{t("tools.refresh")}</Button>
    </div>

    {data.restartRequired && <InlineFeedback>{t("tools.restartRequired")}: {t("tools.restartDescription")}</InlineFeedback>}

    {(state.status === "error" || components.runtime === "error" || data.runtime.status === "error") && <InlineFeedback error>{t("m8Tools.runtimeReadError")} {state.status === "error" && <>{t(errorCodeKey(state.error.code))}</>}</InlineFeedback>}
    {!config && <InlineFeedback error>{t("m8Tools.configUnavailable")}</InlineFeedback>}

    <Section title={t("m8Tools.savedConfiguration")} action={<span className="muted">{t("m8Tools.savedConfigurationHint")}</span>}>
      {metadata.length === 0 ? <InlineFeedback>{t("tools.empty.description")}</InlineFeedback> : <div className="data-table">
        <div className="data-table-head tool-table-head" aria-hidden="true">
          <span>{t("tools.table.toolName")} / {t("tools.table.purpose")}</span>
          <span>{t("m8Tools.enabled")}</span>
          <span>{t("m8Tools.currentRuntime")}</span>
          <span>{t("m8Tools.defaultPolicy")}</span>
        </div>
        {metadata.map((tool) => {
          const effective = effectiveByName.get(tool.name);
          const enabledFeedback = feedback[`tool:${tool.key}:enabled`];
          const defaultFeedback = feedback[`tool:${tool.key}:default`];
          return <div className="data-row tool-table-row" key={tool.key}>
            <button type="button" className="tool-identity data-cell" onClick={() => setSelectedKey(tool.key)}>
              <strong>{tool.name}</strong>
              <span className="tool-purpose">{description(tool)}</span>
              {detailFor(tool, effective) && <span className="tool-detail-note">{detailFor(tool, effective)}</span>}
            </button>
            <div className="data-cell">
              <span className="tool-column-label">{t("m8Tools.enabled")}</span>
              <Switch checked={enabledFor(tool)} onChange={() => void saveToolEnabled(tool)} disabled={!config || enabledFeedback?.status === "saving"} label={t("m8Tools.enabledFor", { tool: tool.name })} />
            </div>
            <div className="data-cell">
              <span className="tool-column-label">{t("m8Tools.currentRuntime")}</span>
              <RuntimeEvidence state={state} component={components.runtime} tool={effective} t={t} />
            </div>
            <div className="data-cell data-cell-actions">
              <span className="tool-column-label">{t("m8Tools.defaultPolicy")}</span>
              <Segmented
                value={policyFor(tool) ?? "ask"}
                options={policyChoices.map((choice) => ({ value: choice.value, label: t(choice.labelKey) }))}
                onChange={(value) => void saveToolDefault(tool, value)}
                disabled={!config || defaultFeedback?.status === "saving"}
                label={t("m8Tools.defaultFor", { tool: tool.name })}
              />
              {tool.key === "bash" && policyFor(tool) === "allow" && <p className="muted">{t("policies.bashTrustModeToolsHint")}</p>}
            </div>
            {(enabledFeedback || defaultFeedback) && <div className="tool-feedback">{enabledFeedback && <FeedbackView feedback={enabledFeedback} t={t} />}{defaultFeedback && <FeedbackView feedback={defaultFeedback} t={t} />}</div>}
          </div>;
        })}
      </div>}
    </Section>

    <Disclosure title={t("m8.advanced")}><ConfigRevisionStatus descriptionKey="tools.restartDescription" /></Disclosure>

    {selectedTool && config && <Inspector title={`${selectedTool.name} · ${t("m8Tools.inspector")}`} onClose={() => setSelectedKey(null)}>
      {(() => {
        const effective = effectiveByName.get(selectedTool.name);
        return <>
          <p className="muted">{description(selectedTool)}</p>
          <Section title={t("m8Tools.savedConfiguration")}>
            <dl className="detail-list">
              <div><dt>{t("m8Tools.enabled")}</dt><dd>{enabledFor(selectedTool) ? t("tools.on") : t("tools.off")}</dd></div>
              <div><dt>{t("m8Tools.defaultPolicy")}</dt><dd>{t(`enum.policyDecision.${policyFor(selectedTool) ?? "ask"}`)}</dd></div>
            </dl>
          </Section>

          <Section title={t("m8Tools.runtimeObservation")}>
            <p>{t("m8Tools.currentRuntime")}: <RuntimeEvidence state={state} component={components.runtime} tool={effective} t={t} /></p>
            <p className="muted">{runtimeDetail(selectedTool, effective) ?? t("m8Tools.noMismatch")}</p>
            {effective?.statusReason && <p className="muted">{t(toolStatusReasonKey(effective.statusReason))} · {t(toolRemediationKey(effective.statusReason))}</p>}
          </Section>

          {selectedTool.key === "bash" && <Section title={t("m8Tools.bash.title")}>
            <p className="muted">{t("m8Tools.bash.description")}</p>
            <div className="workspace-row"><span className="row-identity">{t("m8Tools.bash.autoDetect")}</span><div className="row-controls"><Switch checked={bashDraft.autoDetect} onChange={() => void toggleBashAutoDetect()} disabled={feedback["bash:config"]?.status === "saving"} label={t("m8Tools.bash.autoDetect")} /></div></div>
            <label className="form-field"><span>{t("m8Tools.bash.override")}</span><div className="input-action-row"><input className="path-input" aria-label={t("m8Tools.bash.override")} value={bashDraft.executablePath ?? ""} readOnly placeholder={t("tools.bash.noOverride")} title={bashDraft.executablePath || undefined} /><Button variant="secondary" onClick={() => void chooseBash()} disabled={feedback["bash:config"]?.status === "saving"}>{t("m8Tools.bash.choose")}</Button><Button variant="quiet" onClick={() => void clearBash()} disabled={feedback["bash:config"]?.status === "saving" || !bashDraft.executablePath}>{t("m8Tools.bash.clear")}</Button></div></label>
            <p className="muted">{t("m8Tools.bash.validationBoundary")}</p>
            <FeedbackView feedback={feedback["bash:config"]} t={t} />
          </Section>}

          <Disclosure title={t("m8.advanced")}>
            <dl className="detail-list">
              <div><dt>{t("m8Tools.availability")}</dt><dd>{effective ? t(toolAvailabilityKey(effective.availability)) : t("m8Tools.notAvailable")}</dd></div>
              <div><dt>{t("m8Tools.decision")}</dt><dd>{effective ? t(`enum.policyDecision.${effective.decision}`) : t("m8Tools.notAvailable")}</dd></div>
              <div><dt>{t("m8Tools.statusReason")}</dt><dd>{effective?.statusReason ? t(toolStatusReasonKey(effective.statusReason)) : t("m8Tools.notReported")}</dd></div>
              <div><dt>{t("m8Tools.readiness")}</dt><dd>{effective?.readiness ? t(`enum.toolReadiness.${effective.readiness}`) : t("m8Tools.notReported")}</dd></div>
              {effective?.reason && <div><dt>{t("m8Tools.runtimeReason")}</dt><dd className="mono">{effective.reason}</dd></div>}
              {effective?.readinessReason && <div><dt>{t("m8Tools.readinessReason")}</dt><dd className="mono">{effective.readinessReason}</dd></div>}
            </dl>
          </Disclosure>
        </>;
      })()}
    </Inspector>}

    {!selectedTool && metadata.length > 0 && runningOrObserved && state.status === "success" && effectiveTools.length === 0 && <InlineFeedback>{t("tools.empty.description")}</InlineFeedback>}
  </div>;
}
