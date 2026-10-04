import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import type {
  AppConfig,
  EffectivePolicyRule,
  PolicyConfig,
  PolicyDecision,
  PolicyEffectiveSnapshot,
  PolicyMatcherConfig,
  PolicyReason,
  PolicyRule,
  ToolPolicyDefaults,
  UserPolicyRuleConfig,
} from "@protocol/v1";
import type { TFunction } from "i18next";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PolicyDraftHandoff } from "@/app/router/routes";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import {
  toolAvailabilityKey,
  toolRemediationKey,
  policyDecisionKey,
  policyReasonKey,
  policySnapshotStatusKey,
  policySourceKey,
  presentError,
  systemPolicyNameKey,
  systemPolicyReasonKey,
} from "@/i18n/presentation";

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const tone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";

type MessageDescriptor = {
  key: string;
  values?: Record<string, string | number>;
  detail?: string;
  code?: string;
};

type ValidationResult<T> = { value?: T; error?: MessageDescriptor };
type RuleValidationResult = { rule?: UserPolicyRuleConfig; error?: MessageDescriptor };

const defaultPolicyConfig: PolicyConfig = {
  toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
  userRules: [],
};

const configurableTools = ["read_file", "glob", "write", "edit", "bash"] as const;
type ConfigurableTool = typeof configurableTools[number];
type MatcherKind = "tool" | "bash_command";

const defaultToolRows: Array<{ name: ConfigurableTool; key: keyof ToolPolicyDefaults }> = [
  { name: "read_file", key: "readFile" },
  { name: "glob", key: "glob" },
  { name: "write", key: "write" },
  { name: "edit", key: "edit" },
  { name: "bash", key: "bash" },
];

type RuleEditorState = {
  id: string;
  name: string;
  matcherType: MatcherKind;
  tool: ConfigurableTool;
  executable: string;
  argvPrefixText: string;
  decision: PolicyDecision;
  reason: string;
  enabled: boolean;
};

const emptyEditor = (): RuleEditorState => ({
  id: "",
  name: "",
  matcherType: "tool",
  tool: "read_file",
  executable: "",
  argvPrefixText: "[]",
  decision: "allow",
  reason: "",
  enabled: true,
});

function policyConfig(config: AppConfig | null): PolicyConfig {
  if (!config?.policies) return defaultPolicyConfig;
  // Config is additive across M3/M5. Keep the UI usable when an older
  // persisted object has no bash default yet; Runtime remains authoritative.
  return {
    ...config.policies,
    toolDefaults: { ...defaultPolicyConfig.toolDefaults, ...config.policies.toolDefaults },
    userRules: config.policies.userRules ?? [],
  };
}

function ruleEditor(rule: UserPolicyRuleConfig): RuleEditorState {
  if (rule.matcher.type === "bash_command") {
    return {
      id: rule.id,
      name: rule.name,
      matcherType: "bash_command",
      tool: "bash",
      executable: rule.matcher.executable,
      argvPrefixText: JSON.stringify(rule.matcher.argvPrefix),
      decision: rule.decision,
      reason: rule.reason,
      enabled: rule.enabled,
    };
  }
  const tool = configurableTools.includes(rule.matcher.tool as ConfigurableTool)
    ? rule.matcher.tool as ConfigurableTool
    : "read_file";
  return {
    id: rule.id,
    name: rule.name,
    matcherType: "tool",
    tool,
    executable: "",
    argvPrefixText: "[]",
    decision: rule.decision,
    reason: rule.reason,
    enabled: rule.enabled,
  };
}

function parseArgvPrefix(text: string): ValidationResult<string[]> {
  const source = text.trim();
  if (!source) return { value: [] };
  let parsed: unknown;
  try {
    parsed = JSON.parse(source);
  } catch {
    return { error: { key: "validation.policies.argvPrefixJson" } };
  }
  if (!Array.isArray(parsed) || !parsed.every((token): token is string => typeof token === "string")) {
    return { error: { key: "validation.policies.argvPrefixStrings" } };
  }
  if (parsed.length > 64) {
    return { error: { key: "validation.policies.argvPrefixMaxTokens" } };
  }
  const encoder = new TextEncoder();
  // Empty strings are valid logical argv tokens after quote decoding; only
  // NUL and the Runtime's frozen size bounds are rejected here.
  if (parsed.some((token) => token.includes("\0"))) {
    return { error: { key: "validation.policies.argvPrefixNul" } };
  }
  if (parsed.some((token) => encoder.encode(token).length > 1024)) {
    return { error: { key: "validation.policies.argvPrefixTokenBytes" } };
  }
  return { value: parsed };
}

function isNormalizedExecutable(value: string): boolean {
  return Boolean(value) && !/[\s"';&|<>$`(){}\[\]]/.test(value) && !value.includes("\0");
}

function toUserRule(editor: RuleEditorState): RuleValidationResult {
  const id = editor.id.trim();
  const name = editor.name.trim();
  const reason = editor.reason.trim();
  if (!id) return { error: { key: "validation.policies.ruleIdRequired" } };
  if (!name) return { error: { key: "validation.policies.ruleNameRequired" } };
  if (!reason) return { error: { key: "validation.policies.ruleReasonRequired" } };
  let matcher: PolicyMatcherConfig;
  if (editor.matcherType === "tool") {
    if (!configurableTools.includes(editor.tool)) return { error: { key: "validation.policies.toolMatcherRequired" } };
    matcher = { type: "tool", tool: editor.tool };
  } else {
    const executable = editor.executable.trim();
    if (!isNormalizedExecutable(executable)) {
      return { error: { key: "validation.policies.executableInvalid" } };
    }
    const prefix = parseArgvPrefix(editor.argvPrefixText);
    if (prefix.error || !prefix.value) return { error: prefix.error ?? { key: "validation.policies.argvPrefixInvalid" } };
    matcher = { type: "bash_command", executable, argvPrefix: prefix.value };
  }
  return {
    rule: { id, name, enabled: editor.enabled, decision: editor.decision, matcher, reason },
  };
}

function matcherLabel(t: TFunction, matcher: PolicyMatcherConfig | undefined): string {
  if (!matcher) return t("policies.matcher.systemGuardrail");
  if (matcher.type === "tool") return t("policies.matcher.tool", { tool: matcher.tool });
  const suffix = matcher.argvPrefix.length ? ` ${matcher.argvPrefix.join(" ")}` : "";
  return t("policies.matcher.bashCommand", { executable: matcher.executable, argvPrefix: suffix });
}

function reasonLabel(t: TFunction, reason: PolicyReason): string {
  return t(policyReasonKey(reason));
}

function legacyProjection(rule: PolicyRule): EffectivePolicyRule {
  return {
    id: rule.id,
    name: rule.name,
    decision: rule.decision,
    matcher: rule.source === "user" ? { type: "tool", tool: rule.scope.replace(/^tool:/, "") } : undefined,
    reason: rule.reason,
    source: rule.source,
    editable: rule.editable,
    enabled: rule.enabled,
  };
}

function editorFromDraft(draft: PolicyDraftHandoff): RuleEditorState {
  const base = emptyEditor();
  if (draft.matcher?.type === "bash_command") {
    return {
      ...base,
      id: "",
      name: draft.name,
      matcherType: "bash_command",
      tool: "bash",
      executable: draft.matcher.executable,
      argvPrefixText: JSON.stringify(draft.matcher.argvPrefix),
      decision: "ask",
      reason: draft.reason,
    };
  }
  if (draft.matcher?.type === "tool") {
    return {
      ...base,
      name: draft.name,
      matcherType: "tool",
      tool: configurableTools.includes(draft.matcher.tool as ConfigurableTool) ? draft.matcher.tool as ConfigurableTool : "bash",
      decision: draft.matcher.tool === "bash" ? "ask" : base.decision,
      reason: draft.reason,
    };
  }
  return {
    ...base,
    name: draft.name,
    matcherType: "tool",
    tool: configurableTools.includes(draft.tool as ConfigurableTool) ? draft.tool as ConfigurableTool : "read_file",
    decision: draft.tool === "bash" ? "ask" : base.decision,
    reason: draft.reason,
  };
}

function messageFromError(error: unknown): MessageDescriptor {
  const fallback = error && typeof error === "object" && "message" in error
    ? (error as { message?: unknown }).message
    : undefined;
  const presentation = presentError(error, typeof fallback === "string" ? fallback : undefined);
  return {
    key: presentation.summaryKey,
    detail: presentation.detail,
    code: presentation.code,
  };
}

function renderMessage(t: TFunction, message: MessageDescriptor): string {
  const summary = t(message.key, message.values);
  return message.detail ? `${summary} ${message.detail}` : summary;
}

export function PoliciesPage({ initialDraft = null }: { initialDraft?: PolicyDraftHandoff | null }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [draft, setDraft] = useState<PolicyConfig>(() => policyConfig(data.config));
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<MessageDescriptor | null>(null);
  const [error, setError] = useState<MessageDescriptor | null>(null);
  const [editor, setEditor] = useState<RuleEditorState>(emptyEditor);
  const [editingId, setEditingId] = useState<string | null>(null);

  useEffect(() => {
    if (!dirty) setDraft(policyConfig(data.config));
  }, [data.config, dirty]);

  useEffect(() => {
    if (!initialDraft) return;
    setEditor(editorFromDraft(initialDraft));
    setEditingId(null);
    setError(null);
    setMessage({ key: "policies.approvalDraftPrepared" });
  }, [initialDraft]);

  const effective = data.policies.status === "success" || data.policies.status === "empty"
    ? data.policies.data.effective
    : undefined;
  const legacyItems = data.policies.status === "success" || data.policies.status === "empty"
    ? data.policies.data.items
    : [];
  const effectiveSystemRules = effective?.systemRules ?? legacyItems.filter((rule) => rule.source === "system").map(legacyProjection);
  const effectiveUserRules = effective?.userRules ?? legacyItems.filter((rule) => rule.source === "user").map(legacyProjection);

  const persist = async (next: PolicyConfig, successMessage: MessageDescriptor) => {
    if (!data.config) {
      setError({ key: "validation.policies.saveConfigurationFirst" });
      return false;
    }
    setSaving(true);
    setError(null);
    setMessage(null);
    const result = await data.saveConfig({ ...data.config, policies: next });
    setSaving(false);
    if (!result.ok) {
      setError(messageFromError(result.error ?? { code: "CONFIG_WRITE_FAILED" }));
      setDraft(policyConfig(data.config));
      setDirty(false);
      return false;
    }
    setDraft(next);
    setDirty(false);
    setMessage(successMessage);
    return true;
  };

  const saveDraft = async () => {
    await persist(draft, { key: "policies.desiredSaved" });
  };

  const saveRule = async () => {
    const result = toUserRule(editor);
    if (result.error || !result.rule) {
      setError(result.error ?? { key: "validation.policies.ruleInvalid" });
      return;
    }
    const nextRule = result.rule;
    const duplicate = draft.userRules.some((rule) => rule.id === nextRule.id && rule.id !== editingId);
    if (duplicate) {
      setError({ key: "validation.policies.duplicateRuleId", values: { id: nextRule.id } });
      return;
    }
    const userRules = editingId
      ? draft.userRules.map((rule) => rule.id === editingId ? nextRule : rule)
      : [...draft.userRules, nextRule];
    const saved = await persist(
      { ...draft, userRules },
      editingId ? { key: "policies.userRuleUpdated" } : { key: "policies.userRuleCreated" },
    );
    if (saved) {
      setEditor(emptyEditor());
      setEditingId(null);
    }
  };

  const toggleRule = async (rule: UserPolicyRuleConfig) => {
    await persist(
      { ...draft, userRules: draft.userRules.map((item) => item.id === rule.id ? { ...item, enabled: !item.enabled } : item) },
      rule.enabled ? { key: "policies.userRuleDisabled" } : { key: "policies.userRuleEnabled" },
    );
  };

  const deleteRule = async (rule: UserPolicyRuleConfig) => {
    await persist(
      { ...draft, userRules: draft.userRules.filter((item) => item.id !== rule.id) },
      { key: "policies.userRuleDeleted" },
    );
  };

  const effectiveByTool = useMemo(
    () => new Map((effective?.perTool ?? []).map((item) => [item.tool, item])),
    [effective],
  );

  return <div className="page-stack">
    <SectionHeading
      eyebrow={t("policies.eyebrow")}
      title={t("policies.title")}
      description={t("policies.description")}
      action={<StatusBadge tone={effective?.valid === false ? "danger" : "neutral"}>{effective ? (effective.valid ? t("policies.runtimeSnapshotValid") : t("policies.failClosed")) : t("policies.runtimeNotObserved")}</StatusBadge>}
    />

    <ConfigRevisionStatus descriptionKey="policies.restartRequiredDescription" onRestart={() => void data.restartHarness()} />
    {error && <div className="notice notice-danger" role="alert"><strong>{t("policies.saveFailed")}</strong><span>{t(error.key, error.values)}{error.detail && <><br /><small>{error.detail}</small></>}{error.code && <><br /><small className="mono">{error.code}</small></>}</span></div>}
    {message && <div className="notice notice-success" role="status">{renderMessage(t, message)}</div>}

    <Card className="settings-section">
      <SectionHeading eyebrow={t("policies.desiredConfiguration")} title={t("policies.toolDefaults")} description={t("policies.toolDefaultsDescription")} />
      <div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">{t("policies.tool")}</th><th scope="col">{t("policies.desiredDefault")}</th><th scope="col">{t("policies.currentEffective")}</th><th scope="col">{t("policies.reason")}</th></tr></thead><tbody>
        {defaultToolRows.map((tool) => {
          const effectiveTool = effectiveByTool.get(tool.name);
          return <tr key={tool.name}>
            <td><span className="mono">{tool.name}</span>{tool.name === "bash" && <><br /><span className="muted">{t("policies.highRiskDeveloperTest")}</span></>}</td>
            <td><label><span className="sr-only">{t("policies.desiredDefaultFor", { tool: tool.name })}</span><select aria-label={t("policies.desiredDefaultFor", { tool: tool.name })} value={draft.toolDefaults[tool.key]} disabled={saving} onChange={(event) => { setDraft((current) => ({ ...current, toolDefaults: { ...current.toolDefaults, [tool.key]: event.target.value as PolicyDecision } })); setDirty(true); setMessage(null); }}>{decisions.map((decision) => <option key={decision} value={decision}>{t(policyDecisionKey(decision))}</option>)}</select></label></td>
            <td>{effectiveTool ? t(policyDecisionKey(effectiveTool.decision)) : t("policies.notObserved")}</td>
            <td>{effectiveTool ? reasonLabel(t, effectiveTool.reason) : t("policies.noActiveRuntime")}</td>
          </tr>;
        })}
      </tbody></table></div>
      <div className="settings-actions"><span className={dirty ? "warning-text" : "muted"}>{dirty ? t("policies.unsavedChanges") : t("policies.desiredSaved")}</span><button className="button button-primary" disabled={!dirty || saving} onClick={() => void saveDraft()}>{saving ? t("policies.saving") : t("policies.saveChanges")}</button></div>
    </Card>

    <Card className="settings-section">
      <SectionHeading eyebrow={t("policies.desiredConfiguration")} title={t("policies.userRules")} description={t("policies.userRulesDescription")} />
      <p className="settings-help muted">{t("policies.userRulesHelp")}</p>
      {draft.userRules.length === 0 ? <p className="muted">{t("policies.noDesiredUserRules")}</p> : <div className="policy-list">{draft.userRules.map((rule) => <DesiredRuleRow key={rule.id} rule={rule} disabled={saving} t={t} onEdit={() => { setEditor(ruleEditor(rule)); setEditingId(rule.id); setError(null); }} onToggle={() => void toggleRule(rule)} onDelete={() => void deleteRule(rule)} />)}</div>}
      <div className="policy-editor">
        <h3>{editingId ? t("policies.editUserRule") : t("policies.createUserRule")}</h3>
        {initialDraft && <div className="notice notice-warning" role="note"><strong>{t("policies.approvalHandoff")}</strong><span>{t(initialDraft.matcher?.type === "bash_command" ? "policies.approvalBashCommandDraft" : initialDraft.tool === "bash" ? "policies.approvalBashToolDraft" : "policies.approvalToolDraft", { scope: initialDraft.scope })}</span></div>}
        <div className="form-row">
          <label className="form-field grow"><span>{t("policies.ruleId")}</span><input aria-label={t("policies.userRuleIdAria")} value={editor.id} readOnly={Boolean(editingId)} onChange={(event) => setEditor((current) => ({ ...current, id: event.target.value }))} placeholder={t("policies.ruleIdPlaceholder")} /></label>
          <label className="form-field grow"><span>{t("policies.name")}</span><input aria-label={t("policies.userRuleNameAria")} value={editor.name} onChange={(event) => setEditor((current) => ({ ...current, name: event.target.value }))} placeholder={t("policies.namePlaceholder")} /></label>
        </div>
        <div className="form-row">
          <label className="form-field"><span>{t("policies.matcherType")}</span><select aria-label={t("policies.userRuleMatcherTypeAria")} value={editor.matcherType} onChange={(event) => setEditor((current) => ({ ...current, matcherType: event.target.value as MatcherKind }))}><option value="tool">{t("policies.tool")}</option><option value="bash_command">{t("policies.bashCommand")}</option></select></label>
          {editor.matcherType === "tool" ? <label className="form-field"><span>{t("policies.toolMatcher")}</span><select aria-label={t("policies.userRuleToolMatcherAria")} value={editor.tool} onChange={(event) => setEditor((current) => ({ ...current, tool: event.target.value as ConfigurableTool }))}>{configurableTools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}</select></label> : <label className="form-field grow"><span>{t("policies.bashExecutable")}</span><input aria-label={t("policies.bashExecutableAria")} value={editor.executable} onChange={(event) => setEditor((current) => ({ ...current, executable: event.target.value }))} placeholder={t("policies.bashExecutablePlaceholder")} /></label>}
          <label className="form-field"><span>{t("policies.decision")}</span><select aria-label={t("policies.userRuleDecisionAria")} value={editor.decision} onChange={(event) => setEditor((current) => ({ ...current, decision: event.target.value as PolicyDecision }))}>{decisions.map((decision) => <option key={decision} value={decision}>{t(policyDecisionKey(decision))}</option>)}</select></label>
          <label className="form-field"><span>{t("policies.enabled")}</span><input aria-label={t("policies.userRuleEnabledAria")} type="checkbox" checked={editor.enabled} onChange={(event) => setEditor((current) => ({ ...current, enabled: event.target.checked }))} /></label>
        </div>
        {editor.matcherType === "bash_command" && <label className="form-field"><span>{t("policies.bashArgvPrefix")}</span><textarea aria-label={t("policies.bashArgvPrefixAria")} rows={2} value={editor.argvPrefixText} onChange={(event) => setEditor((current) => ({ ...current, argvPrefixText: event.target.value }))} placeholder={t("policies.bashArgvPrefixPlaceholder")} /><small>{t("policies.bashArgvPrefixHelp")}</small></label>}
        <label className="form-field"><span>{t("policies.reason")}</span><input aria-label={t("policies.userRuleReasonAria")} value={editor.reason} onChange={(event) => setEditor((current) => ({ ...current, reason: event.target.value }))} placeholder={t("policies.reasonPlaceholder")} /></label>
        <div className="settings-actions"><button className="button button-secondary" disabled={saving} onClick={() => { setEditor(emptyEditor()); setEditingId(null); setError(null); }}>{t("policies.clear")}</button><button className="button button-primary" disabled={saving} onClick={() => void saveRule()}>{saving ? t("policies.saving") : editingId ? t("policies.saveUserRule") : t("policies.createUserRule")}</button></div>
      </div>
    </Card>

    <EffectivePolicySection
      t={t}
      effective={effective}
      systemRules={effectiveSystemRules}
      userRules={effectiveUserRules}
      status={data.policies.status}
    />
  </div>;
}

function DesiredRuleRow({ rule, disabled, t, onEdit, onToggle, onDelete }: { rule: UserPolicyRuleConfig; disabled: boolean; t: TFunction; onEdit: () => void; onToggle: () => void; onDelete: () => void }) {
  return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.enabled ? tone(rule.decision) : "neutral"}>{rule.enabled ? t(policyDecisionKey(rule.decision)) : t("policies.disabled")}</StatusBadge></div><p>{rule.reason}</p><span className="muted mono">{rule.id} · {matcherLabel(t, rule.matcher)}</span></div><div className="policy-actions"><button className="button button-secondary" disabled={disabled} onClick={onEdit}>{t("policies.edit")}</button><button className="button button-secondary" disabled={disabled} onClick={onToggle}>{rule.enabled ? t("policies.disable") : t("policies.enable")}</button><button className="button button-quiet danger-text" disabled={disabled} onClick={onDelete}>{t("policies.delete")}</button></div></Card>;
}

function EffectivePolicySection({ t, effective, systemRules, userRules, status }: { t: TFunction; effective?: PolicyEffectiveSnapshot; systemRules: EffectivePolicyRule[]; userRules: EffectivePolicyRule[]; status: string }) {
  const observed = Boolean(effective) || systemRules.length > 0 || userRules.length > 0;
  return <Card className="settings-section">
    <SectionHeading eyebrow={t("policies.effectiveObservation")} title={t("policies.currentGeneration")} description={t("policies.effectiveDescription")} />
    {!effective && status !== "success" && status !== "empty" ? <p className="muted">{t("policies.effectiveUnavailable")}</p> : null}
    {effective && !effective.valid && <div className="notice notice-danger" role="status"><strong>{t("policies.invalidSnapshot")}</strong><span>{t("policies.invalidSnapshotDescription")}</span><span>{t("m7.remediation.invalid_policy_snapshot")}</span></div>}
    {observed && <>
      {effective && <div className="policy-effective-status"><StatusBadge tone={effective.valid ? "success" : "danger"}>{t(policySnapshotStatusKey(effective.status))}</StatusBadge>{effective.statusReason && <span className="muted">{reasonLabel(t, effective.statusReason)}</span>}</div>}
      {effective?.valid !== false && <><h3>{t("policies.systemGuardrails")}</h3>
      <div className="policy-list">{systemRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} t={t} />)}</div>
      <h3>{t("policies.userRulesCurrentGeneration")}</h3>
      {userRules.length ? <div className="policy-list">{userRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} t={t} />)}</div> : <p className="muted">{t("policies.noCompiledUserRules")}</p>}
      </>}
      {effective && <><h3>{t("policies.perToolDecision")}</h3>
      <div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">{t("policies.tool")}</th><th scope="col">{t("policies.decision")}</th><th scope="col">{t("tools.table.availability")}</th><th scope="col">{t("policies.matchedIds")}</th><th scope="col">{t("policies.reason")}</th></tr></thead><tbody>{effective.perTool.map((tool) => <tr key={tool.tool}><td className="mono">{tool.tool}</td><td>{t(policyDecisionKey(tool.decision))}</td><td>{tool.availability === "unavailable_by_policy" && <><StatusBadge tone="danger">{t("m7.states.denied")}</StatusBadge><br /></>}{t(toolAvailabilityKey(tool.availability))}</td><td className="mono">{tool.matchedRuleIds.length ? tool.matchedRuleIds.join(", ") : t("policies.none")}</td><td>{reasonLabel(t, tool.reason)}<p>{t(toolRemediationKey(policyToolRemediation(tool)))}</p></td></tr>)}</tbody></table></div></>}
    </>}
  </Card>;
}

function EffectiveRuleRow({ t, rule }: { t: TFunction; rule: EffectivePolicyRule }) {
  const displayName = rule.source === "system" ? t(systemPolicyNameKey(rule.id)) : rule.name;
  const displayReason = rule.source === "system" ? t(systemPolicyReasonKey(rule.id)) : rule.reason;
  return <Card className="policy-row"><div><div className="policy-title"><strong>{displayName}</strong><StatusBadge tone={rule.source === "system" ? "neutral" : tone(rule.decision)}>{t(policySourceKey(rule.source))}</StatusBadge></div><p>{displayReason}</p></div><div className="policy-facts"><span><small>{t("policies.source")}</small><b>{t(policySourceKey(rule.source))}</b></span><span><small>{t("policies.ruleId")}</small><b className="mono">{rule.id}</b></span><span><small>{t("policies.matcherLabel")}</small><b className="mono">{matcherLabel(t, rule.matcher)}</b></span><span><small>{t("policies.decision")}</small><b>{t(policyDecisionKey(rule.decision))}</b></span><span><small>{t("policies.mode")}</small><b>{t("policies.readOnly")}</b></span></div></Card>;
}

function policyToolRemediation(tool: import("@protocol/v1").EffectivePolicyTool): import("@protocol/v1").ToolStatusReason {
  if (tool.reason === "invalid_policy_snapshot") return "invalid_policy_snapshot";
  if (tool.reason === "invalid_context") return "invalid_policy_context";
  if (tool.availability === "disabled") return "tool_disabled";
  if (tool.availability === "available_with_approval") return "approval_required";
  if (tool.availability === "available") return "available";
  if (tool.reason === "unsupported_capability") return "unsupported_capability";
  if (tool.availability === "unavailable_by_policy") return "policy_denied";
  return "runtime_not_ready";
}
