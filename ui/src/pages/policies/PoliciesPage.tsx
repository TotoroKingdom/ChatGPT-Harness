import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import type {
  AppConfig,
  EffectivePolicyRule,
  PolicyConfig,
  PolicyDecision,
  PolicyEffectiveSnapshot,
  PolicyReason,
  PolicyRule,
  ToolPolicyDefaults,
  UserPolicyRuleConfig,
} from "@protocol/v1";
import type { TFunction } from "i18next";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useHarnessData, type SaveResult } from "@/app/providers/HarnessDataProvider";
import type { Navigate, PolicyDraftHandoff } from "@/app/router/routes";
import { assertUnchanged } from "@/app/providers/config-mutations";
import { HarnessClientError } from "@/services/harness/errors";
import { useDraftGuard, useDraftNavigation } from "@/app/router/DraftNavigation";
import {
  Button,
  Card,
  Dialog,
  Disclosure,
  InlineFeedback,
  Inspector,
  Row,
  Section,
  SectionHeading,
  Segmented,
  Sheet,
  StatusBadge,
  Switch,
} from "@/components/common/primitives";
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
import {
  editorFromDraft,
  editorFromRule,
  matcherSummary,
  newRuleEditor,
  ruleFromEditor,
  RuleEditor,
  type RuleEditorError,
  type RuleEditorValue,
} from "./RuleEditor";

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const defaultPolicyConfig: PolicyConfig = {
  toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" },
  userRules: [],
};
const defaultToolRows: Array<{ name: string; key: keyof ToolPolicyDefaults }> = [
  { name: "read_file", key: "readFile" },
  { name: "glob", key: "glob" },
  { name: "grep", key: "grep" },
  { name: "write", key: "write" },
  { name: "edit", key: "edit" },
  { name: "bash", key: "bash" },
];

type Message = { key: string; values?: Record<string, string | number>; detail?: string; code?: string };
function policyConfig(config: AppConfig | null): PolicyConfig {
  if (!config?.policies) return { ...defaultPolicyConfig, userRules: [] };
  return {
    ...config.policies,
    toolDefaults: { ...defaultPolicyConfig.toolDefaults, ...config.policies.toolDefaults },
    userRules: config.policies.userRules ?? [],
  };
}

function cloneRule(rule: UserPolicyRuleConfig): UserPolicyRuleConfig {
  return {
    ...rule,
    matcher: rule.matcher.type === "tool"
      ? { type: "tool", tool: rule.matcher.tool }
      : { type: "bash_command", executable: rule.matcher.executable, argvPrefix: [...rule.matcher.argvPrefix] },
  };
}

function legacyProjection(rule: PolicyRule): EffectivePolicyRule {
  const matcher = rule.source === "user" && rule.scope.startsWith("tool:")
    ? { type: "tool" as const, tool: rule.scope.slice("tool:".length) }
    : undefined;
  return { id: rule.id, name: rule.name, decision: rule.decision, matcher, reason: rule.reason, source: rule.source, editable: rule.editable, enabled: rule.enabled };
}

function reasonLabel(t: TFunction, reason: PolicyReason): string {
  return t(policyReasonKey(reason));
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

function tx(t: TFunction, key: string, fallback: string, values?: Record<string, string | number>) {
  return t(key, { ...values, defaultValue: fallback });
}

function errorMessage(error: unknown): Message {
  const fallback = error && typeof error === "object" && "message" in error ? (error as { message?: unknown }).message : undefined;
  const presentation = presentError(error, typeof fallback === "string" ? fallback : undefined);
  return { key: presentation.summaryKey, detail: presentation.detail, code: presentation.code };
}

function renderMessage(t: TFunction, message: Message): string {
  return `${t(message.key, message.values)}${message.detail ? ` ${message.detail}` : ""}`;
}

function isConflict(message: Message | null): boolean {
  return message?.code === "CONFIG_CONFLICT";
}

function ruleDisplayName(t: TFunction, rule: EffectivePolicyRule): string {
  return rule.source === "system" ? t(systemPolicyNameKey(rule.id)) : rule.name;
}

function ruleDisplayReason(t: TFunction, rule: EffectivePolicyRule): string {
  return rule.source === "system" ? t(systemPolicyReasonKey(rule.id)) : rule.reason;
}

export function PoliciesPage({
  initialDraft = null,
  initialRuleId,
  onNavigate,
}: {
  initialDraft?: PolicyDraftHandoff | null;
  initialRuleId?: string;
  onNavigate?: Navigate;
}) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const desiredFromConfig = useMemo(() => policyConfig(data.config), [data.config]);
  const desired = desiredFromConfig;
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editor, setEditor] = useState<RuleEditorValue | null>(null);
  const [editorInitial, setEditorInitial] = useState<RuleEditorValue | null>(null);
  const [editorBaseline, setEditorBaseline] = useState<UserPolicyRuleConfig | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [approvalDraft, setApprovalDraft] = useState<PolicyDraftHandoff | null>(null);
  const [saving, setSaving] = useState(false);
  const [pendingDefaults, setPendingDefaults] = useState<Record<string, boolean>>({});
  const [defaultFeedback, setDefaultFeedback] = useState<Record<string, { message: Message; error: boolean } | undefined>>({});
  const [pendingRuleIds, setPendingRuleIds] = useState<Record<string, boolean>>({});
  const [ruleFeedback, setRuleFeedback] = useState<Record<string, { message: Message; error: boolean } | undefined>>({});
  const [message, setMessage] = useState<Message | null>(null);
  const [error, setError] = useState<Message | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<UserPolicyRuleConfig | null>(null);
  const [deleteFeedback, setDeleteFeedback] = useState<{ message: Message; error: boolean } | null>(null);

  const effective = data.policies.status === "success" || data.policies.status === "empty" ? data.policies.data.effective : undefined;
  const legacyItems = data.policies.status === "success" || data.policies.status === "empty" ? data.policies.data.items : [];
  const effectiveSystemRules = effective?.systemRules ?? legacyItems.filter((rule) => rule.source === "system").map(legacyProjection);
  const effectiveUserRules = effective?.userRules ?? legacyItems.filter((rule) => rule.source === "user").map(legacyProjection);
  const effectiveByTool = useMemo(() => new Map((effective?.perTool ?? []).map((item) => [item.tool, item])), [effective]);
  const configurationConfirmed = Boolean(data.config);
  const editingId = editorBaseline?.id ?? null;
  const editorDirty = Boolean(editor && editorInitial && JSON.stringify(editor) !== JSON.stringify(editorInitial));
  const { finishDraft } = useDraftNavigation();
  const requestLeave = useDraftGuard({ dirty: editorDirty, saving });
  const consumedInitialRuleId = useRef<string | null>(null);

  useEffect(() => {
    if (initialDraft) {
      if (!configurationConfirmed) {
        setMessage({ key: "m8Policies.configUnavailable" });
        return;
      }
      setApprovalDraft(initialDraft);
      setCreateOpen(true);
      setSelectedId(null);
      const nextEditor = editorFromDraft(initialDraft, desired.userRules.map((rule) => rule.id));
      setEditor(nextEditor);
      setEditorInitial(nextEditor);
      setEditorBaseline(null);
      setMessage({ key: "policies.approvalDraftPrepared" });
    }
  // The router owns the handoff object; only react when its identity changes.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialDraft?.approvalRequestId, initialDraft?.scope, configurationConfirmed]);

  useEffect(() => {
    if (!initialRuleId) {
      consumedInitialRuleId.current = null;
      return;
    }
    if (consumedInitialRuleId.current === initialRuleId) return;
    consumedInitialRuleId.current = initialRuleId;
    const rule = desired.userRules.find((item) => item.id === initialRuleId);
    if (rule) {
      setSelectedId(rule.id);
      setEditor(null);
      setEditorInitial(null);
      setEditorBaseline(null);
    }
  }, [desired.userRules, initialRuleId]);

  useEffect(() => {
    if (!editorBaseline || saving) return;
    const observed = desired.userRules.find((item) => item.id === editorBaseline.id);
    if (!observed || JSON.stringify(observed) !== JSON.stringify(editorBaseline)) {
      setError({ key: "m8.conflict", code: "CONFIG_CONFLICT" });
    }
  }, [desired.userRules, editorBaseline, saving]);

  const runIntent = useCallback(async (
    intent: (latest: AppConfig) => AppConfig,
    success: Message,
  ): Promise<SaveResult> => {
    if (!data.config) {
      setError({ key: "validation.policies.saveConfigurationFirst" });
      return { ok: false, error: new HarnessClientError("error", "Save configuration first.", undefined, "CONFIG_REQUIRED") };
    }
    setError((current) => isConflict(current) ? current : null);
    setMessage(null);
    const result = await data.mutateConfig(intent);
    if (!result.ok) {
      const nextError = errorMessage(result.error ?? { code: "CONFIG_WRITE_FAILED" });
      setError((current) => isConflict(current) ? current : nextError);
      return result;
    }
    setMessage(result.refreshUnconfirmed ? { key: "m8.refreshUnconfirmed" } : success);
    return result;
  }, [data]);

  const saveDefault = async (key: keyof ToolPolicyDefaults, decision: PolicyDecision) => {
    if (!configurationConfirmed) return;
    const id = String(key);
    if (pendingDefaults[id]) return;
    setDefaultFeedback((current) => ({ ...current, [id]: undefined }));
    setPendingDefaults((current) => ({ ...current, [id]: true }));
    const saved = await runIntent(
      (latest) => ({ ...latest, policies: { ...latest.policies, toolDefaults: { ...latest.policies.toolDefaults, [key]: decision } } }),
      { key: "m8.saved" },
    );
    setPendingDefaults((current) => { const next = { ...current }; delete next[id]; return next; });
    setDefaultFeedback((current) => ({ ...current, [id]: { message: { key: saved.ok ? (saved.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved") : "policies.saveFailed" }, error: !saved.ok } }));
  };

  const closeEditor = useCallback(() => {
    requestLeave(() => {
      setEditor(null);
      setEditorInitial(null);
      setEditorBaseline(null);
      setSelectedId(null);
      setCreateOpen(false);
    });
  }, [requestLeave]);

  const returnToApproval = useCallback(() => {
    if (approvalDraft?.approvalRequestId && onNavigate) onNavigate("approvals", { objectId: approvalDraft.approvalRequestId });
    setApprovalDraft(null);
    setEditor(null);
    setEditorInitial(null);
    setCreateOpen(false);
  }, [approvalDraft, onNavigate]);

  const openCreate = () => {
    requestLeave(() => {
      const nextEditor = newRuleEditor(desired.userRules.map((rule) => rule.id));
      setEditor(nextEditor);
      setEditorInitial(nextEditor);
      setEditorBaseline(null);
      setSelectedId(null);
      setApprovalDraft(null);
      setCreateOpen(true);
      setError(null);
    });
  };

  const openEdit = (rule: UserPolicyRuleConfig) => {
    requestLeave(() => {
      setSelectedId(rule.id);
      setEditor(null);
      setEditorInitial(null);
      setEditorBaseline(null);
      setError(null);
    });
  };

  const enterEditMode = (rule: UserPolicyRuleConfig) => {
    const nextEditor = editorFromRule(rule);
    setEditor(nextEditor);
    setEditorInitial(nextEditor);
    setEditorBaseline(cloneRule(rule));
    setError(null);
  };

  const saveRule = async () => {
    if (!editor) return;
    const parsed = ruleFromEditor(editor);
    if (parsed.error || !parsed.rule) {
      setError({ key: typeof parsed.error === "string" ? parsed.error : parsed.error?.key ?? "validation.policies.ruleInvalid" });
      return;
    }
    const nextRule = parsed.rule;
    const baseline = editorBaseline;
    setSaving(true);
    const saved = await runIntent(
      (latest) => {
        const currentRules = latest.policies.userRules ?? [];
        if (baseline) {
          const latestRule = currentRules.find((rule) => rule.id === baseline.id);
          if (!latestRule) throw new HarnessClientError("error", "This object changed. Reload before saving.", undefined, "CONFIG_CONFLICT");
          assertUnchanged(baseline, latestRule);
          return { ...latest, policies: { ...latest.policies, userRules: currentRules.map((rule) => rule.id === baseline.id ? nextRule : rule) } };
        }
        // A retry must not create the same UI-managed object twice if the
        // previous response was lost after persistence.
        const sameId = currentRules.find((rule) => rule.id === nextRule.id);
        if (sameId) {
          if (JSON.stringify(sameId) === JSON.stringify(nextRule)) return latest;
          throw new HarnessClientError("error", "This object changed. Reload before saving.", undefined, "CONFIG_CONFLICT");
        }
        return { ...latest, policies: { ...latest.policies, userRules: [...currentRules, nextRule] } };
      },
      { key: baseline ? "policies.userRuleUpdated" : "policies.userRuleCreated" },
    );
    setSaving(false);
    if (!saved.ok) return;
    finishDraft();
    setEditor(null);
    setEditorInitial(null);
    setEditorBaseline(null);
    setSelectedId(baseline ? baseline.id : null);
    setCreateOpen(false);
    if (!baseline && approvalDraft) returnToApproval();
  };

  const toggleRule = async (rule: UserPolicyRuleConfig) => {
    if (editingId === rule.id || pendingRuleIds[rule.id]) return;
    const nextEnabled = !rule.enabled;
    setRuleFeedback((current) => ({ ...current, [rule.id]: undefined }));
    setPendingRuleIds((current) => ({ ...current, [rule.id]: true }));
    const saved = await runIntent(
      (latest) => ({ ...latest, policies: { ...latest.policies, userRules: (latest.policies.userRules ?? []).map((item) => item.id === rule.id ? { ...item, enabled: nextEnabled } : item) } }),
      { key: nextEnabled ? "policies.userRuleEnabled" : "policies.userRuleDisabled" },
    );
    setPendingRuleIds((current) => { const next = { ...current }; delete next[rule.id]; return next; });
    setRuleFeedback((current) => ({ ...current, [rule.id]: { message: { key: saved.ok ? (saved.refreshUnconfirmed ? "m8.refreshUnconfirmed" : nextEnabled ? "policies.userRuleEnabled" : "policies.userRuleDisabled") : "policies.saveFailed" }, error: !saved.ok } }));
  };

  const requestDelete = (rule: UserPolicyRuleConfig) => {
    requestLeave(() => {
      if (editor || editorBaseline || selectedId === rule.id) {
        setEditor(null);
        setEditorInitial(null);
        setEditorBaseline(null);
        setSelectedId(null);
        setCreateOpen(false);
      }
      setDeleteFeedback(null);
      setDeleteTarget(rule);
    });
  };

  const deleteRule = async () => {
    const target = deleteTarget;
    if (!target) return;
    setSaving(true);
    const saved = await runIntent(
      (latest) => {
        const currentRules = latest.policies.userRules ?? [];
        if (!currentRules.some((item) => item.id === target.id)) throw new HarnessClientError("error", "This object changed. Reload before saving.", undefined, "CONFIG_CONFLICT");
        return { ...latest, policies: { ...latest.policies, userRules: currentRules.filter((item) => item.id !== target.id) } };
      },
      { key: "policies.userRuleDeleted" },
    );
    setSaving(false);
    if (saved.ok) {
      setDeleteTarget(null);
      setDeleteFeedback(null);
    } else {
      setDeleteFeedback({ message: { key: "m8Policies.deleteFailed" }, error: true });
    }
    if (saved.ok && selectedId === target.id) {
      setSelectedId(null);
      setEditor(null);
      setEditorInitial(null);
      setEditorBaseline(null);
    }
  };

  const rule = selectedId ? desired.userRules.find((item) => item.id === selectedId) : undefined;
  const reloadConflictedRule = () => {
    if (!editorBaseline) return;
    const latest = desiredFromConfig.userRules.find((item) => item.id === editorBaseline.id);
    if (!latest) {
      setError({ key: "m8.conflict" });
      return;
    }
    const nextEditor = editorFromRule(latest);
    setEditor(nextEditor);
    setEditorInitial(nextEditor);
    setEditorBaseline(cloneRule(latest));
    setError(null);
  };
  const statusTone = effective?.valid === false ? "danger" : effective?.valid ? "success" : "neutral";
  const statusText = effective ? (effective.valid ? t("policies.runtimeSnapshotValid") : t("policies.failClosed")) : t("policies.runtimeNotObserved");
  const runtimeRestartRequired = configurationConfirmed && data.restartRequired;

  return <div className="page-stack">
    <SectionHeading eyebrow={t("policies.eyebrow")} title={t("policies.title")} description={t("policies.description")} action={<StatusBadge tone={statusTone}>{statusText}</StatusBadge>} />
    {!configurationConfirmed && <div className="notice notice-warning" role="status"><strong>{tx(t, "m8Policies.configUnavailableTitle", "Saved configuration is not confirmed")}</strong><span>{tx(t, "m8Policies.configUnavailable", "Policy defaults and user rules remain unavailable until the saved configuration is loaded.")}</span></div>}
    {effective && !effective.valid && <div className="notice notice-danger" role="alert"><strong>{tx(t, "m8Policies.criticalInvalid", "Critical: invalid Policy snapshot")}</strong><span>{t("policies.invalidSnapshotDescription")}</span><span>{t("m7.remediation.invalid_policy_snapshot")}</span></div>}
    {runtimeRestartRequired && <section aria-label={t("policies.restartRequired")}><div className="notice notice-warning" role="status"><strong>{t("policies.restartRequired")}</strong><span>{t("policies.restartRequiredDescription")}</span><Button variant="secondary" onClick={() => void data.restartHarness()}>{t("overview.restart")}</Button></div></section>}
    {error && <InlineFeedback error>{t(error.key, { ...error.values, defaultValue: error.key })}{error.detail ? ` ${error.detail}` : ""}{error.code ? ` (${error.code})` : ""}</InlineFeedback>}
    {isConflict(error) && editorBaseline && <div className="secondary-actions"><Button variant="secondary" onClick={reloadConflictedRule}>{t("m8.reload")}</Button><Button variant="quiet" onClick={closeEditor}>{tx(t, "common.cancel", "Cancel")}</Button></div>}
    {message && <InlineFeedback>{renderMessage(t, message)}</InlineFeedback>}

    <Section title={tx(t, "m8Policies.defaults", "Tool defaults")} action={<span className="muted">{configurationConfirmed ? tx(t, "m8Policies.defaultsHint", "Saved configuration · changes apply to the next Runtime generation") : tx(t, "m8Policies.configUnavailable", "Saved configuration is not confirmed")}</span>}>
      <div className="workspace-list">{defaultToolRows.map((tool) => {
        const effectiveTool = effectiveByTool.get(tool.name);
        const value = desired.toolDefaults[tool.key];
        const isSaving = Boolean(pendingDefaults[String(tool.key)]);
        const control = configurationConfirmed
          ? <Segmented value={value} label={t("policies.desiredDefaultFor", { tool: tool.name })} disabled={isSaving} options={decisions.map((decision) => ({ value: decision, label: t(policyDecisionKey(decision)) }))} onChange={(decision) => void saveDefault(tool.key, decision)} />
          : <span className="muted">{tx(t, "m8Policies.configNotConfirmed", "Saved value not confirmed")}</span>;
        const feedback = defaultFeedback[String(tool.key)];
        return <div key={tool.name}><Row title={<><strong className="mono">{tool.name}</strong>{tool.name === "bash" && <StatusBadge tone="warning">{t("policies.highRiskDeveloperTest")}</StatusBadge>}</>} summary={<>{configurationConfirmed ? <>{tx(t, "m8Policies.savedValue", "Saved")}: {t(policyDecisionKey(value))}</> : tx(t, "m8Policies.configNotConfirmed", "Saved value not confirmed")} · {tx(t, "m8Policies.currentRuntime", "Current Runtime")}: {effectiveTool ? t(policyDecisionKey(effectiveTool.decision)) : t("policies.notObserved")}</>} controls={control} />{feedback && <InlineFeedback error={feedback.error}>{t(feedback.message.key, { defaultValue: feedback.message.key })}</InlineFeedback>}</div>;
      })}</div>
    </Section>

    <Section title={tx(t, "m8Policies.userRules", "User rules")} action={<Button variant="primary" disabled={!configurationConfirmed} onClick={openCreate}>{tx(t, "m8Policies.addRule", "Add rule")}</Button>}>
      <p className="settings-help muted">{t("policies.userRulesDescription")}</p>
      <p className="settings-help muted">{t("policies.userRulesHelp")}</p>
      {!configurationConfirmed ? <p className="empty-inline">{tx(t, "m8Policies.configUnavailable", "Saved configuration is not confirmed")}</p> : desired.userRules.length === 0 ? <p className="empty-inline">{t("policies.noDesiredUserRules")}</p> : <div className="workspace-list">{desired.userRules.map((item) => <RuleRow key={item.id} rule={item} t={t} editingId={editingId} saving={saving} pending={Boolean(pendingRuleIds[item.id])} feedback={ruleFeedback[item.id]} onInspect={() => openEdit(item)} onToggle={() => void toggleRule(item)} onDelete={() => requestDelete(item)} />)}</div>}
    </Section>

    <Section title={tx(t, "m8Policies.guardrails", "System guardrails")}>
      <Disclosure title={tx(t, "m8Policies.guardrailsReadOnly", "Read-only system guardrails")}>
        <p className="muted">{tx(t, "m8Policies.guardrailsHelp", "These constraints are enforced by Runtime and cannot be edited here.")}</p>
        <p className="muted">{tx(t, "m8Policies.guardrailsEvidence", "The current Runtime snapshot below retains the authoritative guardrail evidence.")}</p>
        {effectiveSystemRules.length ? <div className="policy-list">{effectiveSystemRules.map((item) => <GuardrailHumanRow key={item.id} rule={item} t={t} />)}</div> : <p className="muted">{tx(t, "m8Policies.guardrailsUnavailable", "System guardrail names and reasons are not confirmed in the current snapshot.")}</p>}
      </Disclosure>
    </Section>

    <EffectivePolicySection t={t} effective={effective} systemRules={effectiveSystemRules} userRules={effectiveUserRules} status={data.policies.status} />

    {rule && !editor && <RuleInspector rule={rule} t={t} pending={Boolean(pendingRuleIds[rule.id])} onClose={closeEditor} onEdit={() => enterEditMode(rule)} onToggle={() => void toggleRule(rule)} onDelete={() => requestDelete(rule)} editing={false} />}
    {editor && (createOpen ? <Sheet title={tx(t, "m8Policies.createTitle", "Create rule")} onClose={approvalDraft ? () => requestLeave(returnToApproval) : closeEditor}><RuleEditor value={editor} onChange={setEditor} onSave={() => void saveRule()} onCancel={approvalDraft ? () => requestLeave(returnToApproval) : closeEditor} saving={saving} blocked={isConflict(error)} error={error?.key as RuleEditorError | undefined} mode="create" /></Sheet> : <RuleInspector rule={rule} t={t} onClose={closeEditor} onEdit={() => undefined} onToggle={() => undefined} onDelete={() => undefined} editing editingValue={editor} onEditorChange={setEditor} onSave={() => void saveRule()} onCancel={closeEditor} saving={saving} blocked={isConflict(error)} error={error?.key as RuleEditorError | undefined} />)}
    {deleteTarget && <Dialog title={tx(t, "m8Policies.deleteTitle", "Delete rule?")} onClose={() => { if (!saving) setDeleteTarget(null); }}><p>{tx(t, "m8Policies.deleteBody", "This removes the selected user rule from the desired configuration.")}</p><p><strong>{deleteTarget.name}</strong></p><p className="mono">{deleteTarget.id}</p>{deleteFeedback && <InlineFeedback error={deleteFeedback.error}>{tx(t, deleteFeedback.message.key, "Delete failed. The rule is still present; review the error and retry.")}</InlineFeedback>}<div className="settings-actions"><Button variant="quiet" disabled={saving} onClick={() => setDeleteTarget(null)}>{tx(t, "common.cancel", "Cancel")}</Button><Button variant="danger" disabled={saving} onClick={() => void deleteRule()}>{saving ? t("m8.saving") : tx(t, "m8Policies.confirmDelete", "Delete rule")}</Button></div></Dialog>}
  </div>;
}

function RuleRow({ rule, t, editingId, saving, pending, feedback, onInspect, onToggle, onDelete }: { rule: UserPolicyRuleConfig; t: TFunction; editingId: string | null; saving: boolean; pending: boolean; feedback?: { message: Message; error: boolean }; onInspect: () => void; onToggle: () => void; onDelete: () => void }) {
  const editing = editingId === rule.id;
  return <div><Row title={<><strong>{rule.name}</strong><StatusBadge tone={rule.enabled ? (rule.decision === "allow" ? "success" : rule.decision === "ask" ? "warning" : "danger") : "neutral"}>{rule.enabled ? t(policyDecisionKey(rule.decision)) : t("policies.disabled")}</StatusBadge></>} summary={<><span>{matcherSummary(t, rule.matcher)}</span><span className="muted">{rule.reason}</span></>} onInspect={onInspect} controls={<><Switch checked={rule.enabled} label={`${rule.name} ${rule.enabled ? t("policies.disable") : t("policies.enable")}`} disabled={saving || editing || pending} onChange={() => onToggle()} />{editing && <span className="muted">{t("m8Policies.editingEnabled")}</span>}{pending && <span className="muted">{t("m8.saving")}</span>}<Button variant="quiet" className="danger-text" disabled={saving || pending} onClick={onDelete}>{t("policies.delete")}</Button></>} />{feedback && <InlineFeedback error={feedback.error}>{t(feedback.message.key, { defaultValue: feedback.message.key })}</InlineFeedback>}</div>;
}

function RuleInspector({ rule, t, onClose, onEdit, onToggle, onDelete, editing, editingValue, onEditorChange, onSave, onCancel, saving, pending = false, blocked, error }: { rule?: UserPolicyRuleConfig; t: TFunction; onClose: () => void; onEdit: () => void; onToggle: () => void; onDelete: () => void; editing: boolean; editingValue?: RuleEditorValue; onEditorChange?: (value: RuleEditorValue) => void; onSave?: () => void; onCancel?: () => void; saving?: boolean; pending?: boolean; blocked?: boolean; error?: RuleEditorError }) {
  if (!rule && !editingValue) return null;
  return <Inspector title={editing ? t("policies.editUserRule") : rule?.name ?? t("m8Policies.rule", { defaultValue: "Rule" })} onClose={onClose}>
    {editing && editingValue && onEditorChange && onSave && onCancel ? <RuleEditor value={editingValue} onChange={onEditorChange} onSave={onSave} onCancel={onCancel} saving={saving} blocked={blocked} error={error} mode="edit" /> : rule && <><p>{rule.reason}</p><dl className="local-facts"><div><dt>{t("policies.matcherLabel")}</dt><dd className="mono">{matcherSummary(t, rule.matcher)}</dd></div><div><dt>{t("policies.decision")}</dt><dd>{t(policyDecisionKey(rule.decision))}</dd></div><div><dt>{t("policies.mode")}</dt><dd>{t("policies.readOnly")}</dd></div></dl><InlineFeedback>{tx(t, "m8Policies.guardrailReminder", "This explains matcher scope only; Allow does not override system guardrails, approval, or risk constraints.")}</InlineFeedback><div className="settings-actions"><Button variant="primary" onClick={onEdit}>{t("policies.edit")}</Button><Switch checked={rule.enabled} label={`${rule.name} ${rule.enabled ? t("policies.disable") : t("policies.enable")}`} disabled={pending} onChange={() => onToggle()} /><Button variant="quiet" className="danger-text" disabled={pending} onClick={onDelete}>{t("policies.delete")}</Button></div><Disclosure title={t("m8.advanced")}><pre className="mono">{JSON.stringify(rule, null, 2)}</pre></Disclosure></>}
  </Inspector>;
}

function EffectivePolicySection({ t, effective, systemRules, userRules, status }: { t: TFunction; effective?: PolicyEffectiveSnapshot; systemRules: EffectivePolicyRule[]; userRules: EffectivePolicyRule[]; status: string }) {
  const observed = Boolean(effective) || systemRules.length > 0 || userRules.length > 0;
  return <Section title={tx(t, "m8Policies.effective", "Effective policy diagnostics")}>
    {!effective && status !== "success" && status !== "empty" && <p className="muted">{t("policies.effectiveUnavailable")}</p>}
    <Disclosure title={tx(t, "m8Policies.revisionDetails", "Configuration revision details")}><ConfigRevisionStatus descriptionKey="policies.restartRequiredDescription" /></Disclosure>
    {observed && <Disclosure title={tx(t, "m8Policies.effectiveDetails", "Runtime effective snapshot")} open={Boolean(effective && !effective.valid)}>
      <h3>{t("policies.currentGeneration")}</h3>
      {effective && <div className="policy-effective-status"><StatusBadge tone={effective.valid ? "success" : "danger"}>{t(policySnapshotStatusKey(effective.status))}</StatusBadge>{effective.statusReason && <span className="muted">{reasonLabel(t, effective.statusReason)}</span>}</div>}
      <h3>{t("policies.systemGuardrails")}</h3><div className="policy-list">{systemRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} t={t} />)}</div>
      <h3>{t("policies.userRulesCurrentGeneration")}</h3>{userRules.length ? <div className="policy-list">{userRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} t={t} />)}</div> : <p className="muted">{t("policies.noCompiledUserRules")}</p>}
      {effective && <><h3>{t("policies.perToolDecision")}</h3><div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">{t("policies.tool")}</th><th scope="col">{t("policies.decision")}</th><th scope="col">{t("tools.table.availability")}</th><th scope="col">{t("policies.matchedIds")}</th><th scope="col">{t("policies.reason")}</th></tr></thead><tbody>{effective.perTool.map((tool) => <tr key={tool.tool}><td className="mono">{tool.tool}</td><td>{t(policyDecisionKey(tool.decision))}</td><td>{tool.availability === "unavailable_by_policy" && <><StatusBadge tone="danger">{t("m7.states.denied")}</StatusBadge><br /></>}{t(toolAvailabilityKey(tool.availability))}</td><td className="mono">{tool.matchedRuleIds.length ? tool.matchedRuleIds.join(", ") : t("policies.none")}</td><td>{reasonLabel(t, tool.reason)}<p>{t(toolRemediationKey(policyToolRemediation(tool)))}</p></td></tr>)}</tbody></table></div></>}
    </Disclosure>}
  </Section>;
}

function GuardrailHumanRow({ t, rule }: { t: TFunction; rule: EffectivePolicyRule }) {
  return <div className="policy-row"><div><div className="policy-title"><strong>{ruleDisplayName(t, rule)}</strong><StatusBadge tone="neutral">{t("policies.readOnly")}</StatusBadge></div><p>{ruleDisplayReason(t, rule)}</p></div></div>;
}

function EffectiveRuleRow({ t, rule }: { t: TFunction; rule: EffectivePolicyRule }) {
  return <Card className="policy-row"><div><div className="policy-title"><strong>{ruleDisplayName(t, rule)}</strong><StatusBadge tone={rule.source === "system" ? "neutral" : rule.decision === "allow" ? "success" : rule.decision === "ask" ? "warning" : "danger"}>{t(policySourceKey(rule.source))}</StatusBadge></div><p>{ruleDisplayReason(t, rule)}</p></div><div className="policy-facts"><span><small>{t("policies.source")}</small><b>{t(policySourceKey(rule.source))}</b></span><span><small>{t("policies.ruleId")}</small><b className="mono">{rule.id}</b></span><span><small>{t("policies.matcherLabel")}</small><b className="mono">{matcherSummary(t, rule.matcher)}</b></span><span><small>{t("policies.decision")}</small><b>{t(policyDecisionKey(rule.decision))}</b></span><span><small>{t("policies.mode")}</small><b>{t("policies.readOnly")}</b></span></div></Card>;
}
