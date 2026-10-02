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
import { useEffect, useMemo, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const labels: Record<PolicyDecision, string> = { allow: "ALLOW", ask: "ASK", deny: "DENY" };
const tone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";
const defaultPolicyConfig: PolicyConfig = {
  toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
  userRules: [],
};
const configurableTools = ["read_file", "glob", "write", "edit"] as const;
const defaultToolRows: Array<{ name: string; key: keyof ToolPolicyDefaults; future?: boolean }> = [
  { name: "read_file", key: "readFile" },
  { name: "glob", key: "glob" },
  { name: "write", key: "write" },
  { name: "edit", key: "edit" },
  { name: "bash", key: "bash", future: true },
];

type RuleEditorState = {
  id: string;
  name: string;
  tool: string;
  decision: PolicyDecision;
  reason: string;
  enabled: boolean;
};

const emptyEditor = (): RuleEditorState => ({
  id: "",
  name: "",
  tool: "read_file",
  decision: "allow",
  reason: "",
  enabled: true,
});

function policyConfig(config: AppConfig | null): PolicyConfig {
  return config?.policies ?? defaultPolicyConfig;
}

function ruleEditor(rule: UserPolicyRuleConfig): RuleEditorState {
  return {
    id: rule.id,
    name: rule.name,
    tool: rule.matcher.tool,
    decision: rule.decision,
    reason: rule.reason,
    enabled: rule.enabled,
  };
}

function toUserRule(editor: RuleEditorState): UserPolicyRuleConfig {
  return {
    id: editor.id.trim(),
    name: editor.name.trim(),
    enabled: editor.enabled,
    decision: editor.decision,
    matcher: { type: "tool", tool: editor.tool },
    reason: editor.reason.trim(),
  };
}

function reasonLabel(reason: PolicyReason): string {
  switch (reason) {
    case "system_guardrail": return "System guardrail";
    case "matched_user_rules": return "Matched user rules";
    case "tool_default": return "Tool default";
    case "invalid_policy_snapshot": return "Invalid policy snapshot";
    case "invalid_context": return "Invalid context";
    case "unsupported_capability": return "Unsupported capability";
    case "tool_disabled": return "Tool disabled";
  }
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

export function PoliciesPage() {
  const data = useHarnessData();
  const [draft, setDraft] = useState<PolicyConfig>(() => policyConfig(data.config));
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [editor, setEditor] = useState<RuleEditorState>(emptyEditor);
  const [editingId, setEditingId] = useState<string | null>(null);

  useEffect(() => {
    if (!dirty) setDraft(policyConfig(data.config));
  }, [data.config]);

  const effective = data.policies.status === "success" || data.policies.status === "empty"
    ? data.policies.data.effective
    : undefined;
  const legacyItems = data.policies.status === "success" || data.policies.status === "empty"
    ? data.policies.data.items
    : [];
  const effectiveSystemRules = effective?.systemRules ?? legacyItems.filter((rule) => rule.source === "system").map(legacyProjection);
  const effectiveUserRules = effective?.userRules ?? legacyItems.filter((rule) => rule.source === "user").map(legacyProjection);

  const persist = async (next: PolicyConfig, successMessage: string) => {
    if (!data.config) {
      setError("Save configuration before editing policy.");
      return false;
    }
    setSaving(true);
    setError("");
    setMessage("");
    const result = await data.saveConfig({ ...data.config, policies: next });
    setSaving(false);
    if (!result.ok) {
      setError(result.error?.message ?? "Could not save policy configuration.");
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
    await persist(draft, "Desired policy configuration saved.");
  };

  const saveRule = async () => {
    const nextRule = toUserRule(editor);
    if (!nextRule.id) return setError("User policy rule ID is required.");
    if (!nextRule.name) return setError("User policy rule name is required.");
    if (!nextRule.reason) return setError("User policy rule reason is required.");
    if (!configurableTools.includes(nextRule.matcher.tool as typeof configurableTools[number])) {
      return setError("Choose a supported built-in tool matcher.");
    }
    const duplicate = draft.userRules.some((rule) => rule.id === nextRule.id && rule.id !== editingId);
    if (duplicate) return setError(`Duplicate user policy rule ID '${nextRule.id}'.`);
    const userRules = editingId
      ? draft.userRules.map((rule) => rule.id === editingId ? nextRule : rule)
      : [...draft.userRules, nextRule];
    const saved = await persist({ ...draft, userRules }, editingId ? "User rule updated." : "User rule created.");
    if (saved) {
      setEditor(emptyEditor());
      setEditingId(null);
    }
  };

  const toggleRule = async (rule: UserPolicyRuleConfig) => {
    await persist(
      { ...draft, userRules: draft.userRules.map((item) => item.id === rule.id ? { ...item, enabled: !item.enabled } : item) },
      rule.enabled ? "User rule disabled." : "User rule enabled.",
    );
  };

  const deleteRule = async (rule: UserPolicyRuleConfig) => {
    await persist(
      { ...draft, userRules: draft.userRules.filter((item) => item.id !== rule.id) },
      "User rule deleted.",
    );
  };

  const effectiveByTool = useMemo(
    () => new Map((effective?.perTool ?? []).map((item) => [item.tool, item])),
    [effective],
  );

  return <div className="page-stack">
    <SectionHeading
      eyebrow="M3 configurable policy"
      title="Policies"
      description="Edit the next Runtime generation's desired policy and observe the current generation's effective decisions."
      action={<StatusBadge tone={effective?.valid === false ? "danger" : "neutral"}>{effective ? (effective.valid ? "Runtime snapshot valid" : "Fail closed") : "Runtime not observed"}</StatusBadge>}
    />

    {data.restartRequired && <div className="notice notice-warning" role="status"><strong>Restart Required</strong><span>Desired policy is saved, but the current Runtime generation is still using its previous snapshot.</span><button className="button button-warning" onClick={() => void data.restartHarness()}>Restart Harness</button></div>}
    {error && <div className="notice notice-danger" role="alert"><strong>Policy save failed</strong><span>{error}</span></div>}
    {message && <div className="notice notice-success" role="status">{message}</div>}

    <Card className="settings-section">
      <SectionHeading eyebrow="Desired configuration" title="Tool Defaults" description="A default is used only when no enabled User Rule matches. Bash remains a future configuration field and is not executable in M3." />
      <div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">Tool</th><th scope="col">Desired default</th><th scope="col">Current effective</th><th scope="col">Reason</th></tr></thead><tbody>
        {defaultToolRows.map((tool) => {
          const effectiveTool = effectiveByTool.get(tool.name);
          return <tr key={tool.name}>
            <td><span className="mono">{tool.name}</span>{tool.future && <><br /><span className="muted">future / not implemented</span></>}</td>
            <td><label><span className="sr-only">Desired default for {tool.name}</span><select aria-label={`Desired default for ${tool.name}`} value={draft.toolDefaults[tool.key]} disabled={saving} onChange={(event) => { setDraft((current) => ({ ...current, toolDefaults: { ...current.toolDefaults, [tool.key]: event.target.value as PolicyDecision } })); setDirty(true); setMessage(""); }}>{decisions.map((decision) => <option key={decision} value={decision}>{labels[decision]}</option>)}</select></label></td>
            <td>{effectiveTool ? labels[effectiveTool.decision] : "NOT OBSERVED"}</td>
            <td>{effectiveTool ? reasonLabel(effectiveTool.reason) : "No active Runtime generation."}</td>
          </tr>;
        })}
      </tbody></table></div>
      <div className="settings-actions"><span className={dirty ? "warning-text" : "muted"}>{dirty ? "Unsaved policy changes" : "Desired policy saved"}</span><button className="button button-primary" disabled={!dirty || saving} onClick={() => void saveDraft()}>{saving ? "Saving…" : "Save Policy Changes"}</button></div>
    </Card>

    <Card className="settings-section">
      <SectionHeading eyebrow="Desired configuration" title="User Rules" description="User Rules are persisted as typed tool matchers. Rule identity stays stable during edits." />
      {draft.userRules.length === 0 ? <p className="muted">No desired User Rules configured.</p> : <div className="policy-list">{draft.userRules.map((rule) => <DesiredRuleRow key={rule.id} rule={rule} disabled={saving} onEdit={() => { setEditor(ruleEditor(rule)); setEditingId(rule.id); setError(""); }} onToggle={() => void toggleRule(rule)} onDelete={() => void deleteRule(rule)} />)}</div>}
      <div className="policy-editor">
        <h3>{editingId ? "Edit User Rule" : "Create User Rule"}</h3>
        <div className="form-row">
          <label className="form-field grow"><span>Rule ID</span><input aria-label="User rule ID" value={editor.id} readOnly={Boolean(editingId)} onChange={(event) => setEditor((current) => ({ ...current, id: event.target.value }))} placeholder="e.g. review-writes" /></label>
          <label className="form-field grow"><span>Name</span><input aria-label="User rule name" value={editor.name} onChange={(event) => setEditor((current) => ({ ...current, name: event.target.value }))} placeholder="Review writes" /></label>
        </div>
        <div className="form-row">
          <label className="form-field"><span>Tool matcher</span><select aria-label="User rule tool matcher" value={editor.tool} onChange={(event) => setEditor((current) => ({ ...current, tool: event.target.value }))}>{configurableTools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}</select></label>
          <label className="form-field"><span>Decision</span><select aria-label="User rule decision" value={editor.decision} onChange={(event) => setEditor((current) => ({ ...current, decision: event.target.value as PolicyDecision }))}>{decisions.map((decision) => <option key={decision} value={decision}>{labels[decision]}</option>)}</select></label>
          <label className="form-field"><span>Enabled</span><input aria-label="User rule enabled" type="checkbox" checked={editor.enabled} onChange={(event) => setEditor((current) => ({ ...current, enabled: event.target.checked }))} /></label>
        </div>
        <label className="form-field"><span>Reason</span><input aria-label="User rule reason" value={editor.reason} onChange={(event) => setEditor((current) => ({ ...current, reason: event.target.value }))} placeholder="Explain the policy decision" /></label>
        <div className="settings-actions"><button className="button button-secondary" disabled={saving} onClick={() => { setEditor(emptyEditor()); setEditingId(null); setError(""); }}>Clear</button><button className="button button-primary" disabled={saving} onClick={() => void saveRule()}>{saving ? "Saving…" : editingId ? "Save User Rule" : "Create User Rule"}</button></div>
      </div>
    </Card>

    <EffectivePolicySection
      effective={effective}
      systemRules={effectiveSystemRules}
      userRules={effectiveUserRules}
      status={data.policies.status}
    />
  </div>;
}

function DesiredRuleRow({ rule, disabled, onEdit, onToggle, onDelete }: { rule: UserPolicyRuleConfig; disabled: boolean; onEdit: () => void; onToggle: () => void; onDelete: () => void }) {
  return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.enabled ? tone(rule.decision) : "neutral"}>{rule.enabled ? labels[rule.decision] : "DISABLED"}</StatusBadge></div><p>{rule.reason}</p><span className="muted mono">{rule.id} · tool:{rule.matcher.tool}</span></div><div className="policy-actions"><button className="button button-secondary" disabled={disabled} onClick={onEdit}>Edit</button><button className="button button-secondary" disabled={disabled} onClick={onToggle}>{rule.enabled ? "Disable" : "Enable"}</button><button className="button button-quiet danger-text" disabled={disabled} onClick={onDelete}>Delete</button></div></Card>;
}

function EffectivePolicySection({ effective, systemRules, userRules, status }: { effective?: PolicyEffectiveSnapshot; systemRules: EffectivePolicyRule[]; userRules: EffectivePolicyRule[]; status: string }) {
  const observed = Boolean(effective) || systemRules.length > 0 || userRules.length > 0;
  return <Card className="settings-section">
    <SectionHeading eyebrow="Effective Runtime observation" title="Current generation" description="These rows come from Runtime's compiled snapshot. System Guardrails are always read-only." />
    {!effective && status !== "success" && status !== "empty" ? <p className="muted">Runtime effective policy is unavailable until a generation is running.</p> : null}
    {effective && !effective.valid && <div className="notice notice-danger" role="status"><strong>Invalid policy snapshot — fail closed</strong><span>Policy-controlled tools are denied until a valid snapshot is compiled by the next Runtime generation.</span></div>}
    {observed && <>
      {effective && <div className="policy-effective-status"><StatusBadge tone={effective.valid ? "success" : "danger"}>{effective.status}</StatusBadge>{effective.statusReason && <span className="muted">{reasonLabel(effective.statusReason)}</span>}</div>}
      <h3>System Guardrails</h3>
      <div className="policy-list">{systemRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} />)}</div>
      <h3>User Rules in current generation</h3>
      {userRules.length ? <div className="policy-list">{userRules.map((rule) => <EffectiveRuleRow key={rule.id} rule={rule} />)}</div> : <p className="muted">No User Rules compiled in this generation.</p>}
      {effective && <><h3>Per-tool effective decision</h3>
      <div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">Tool</th><th scope="col">Decision</th><th scope="col">Matched IDs</th><th scope="col">Reason</th></tr></thead><tbody>{effective.perTool.map((tool) => <tr key={tool.tool}><td className="mono">{tool.tool}</td><td>{labels[tool.decision]}</td><td className="mono">{tool.matchedRuleIds.length ? tool.matchedRuleIds.join(", ") : "—"}</td><td>{reasonLabel(tool.reason)}</td></tr>)}</tbody></table></div></>}
    </>}
  </Card>;
}

function EffectiveRuleRow({ rule }: { rule: EffectivePolicyRule }) {
  return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.source === "system" ? "neutral" : tone(rule.decision)}>{rule.source}</StatusBadge></div><p>{rule.reason}</p></div><div className="policy-facts"><span><small>Source</small><b>{rule.source}</b></span><span><small>Matcher</small><b className="mono">{rule.matcher ? `tool:${rule.matcher.tool}` : "system guardrail"}</b></span><span><small>Decision</small><b>{labels[rule.decision]}</b></span><span><small>Mode</small><b>Read-only</b></span></div></Card>;
}
