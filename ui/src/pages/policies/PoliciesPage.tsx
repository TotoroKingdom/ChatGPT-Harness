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
import { useEffect, useMemo, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PolicyDraftHandoff } from "@/app/router/routes";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const labels: Record<PolicyDecision, string> = { allow: "ALLOW", ask: "ASK", deny: "DENY" };
const tone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";

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

function parseArgvPrefix(text: string): { value?: string[]; error?: string } {
  const source = text.trim();
  if (!source) return { value: [] };
  let parsed: unknown;
  try {
    parsed = JSON.parse(source);
  } catch {
    return { error: "Bash argv prefix must be a JSON array, for example [\"status\", \"--short\"]." };
  }
  if (!Array.isArray(parsed) || !parsed.every((token): token is string => typeof token === "string")) {
    return { error: "Bash argv prefix must be a JSON array of string tokens." };
  }
  if (parsed.length > 64) {
    return { error: "Bash argv prefix cannot contain more than 64 tokens." };
  }
  const encoder = new TextEncoder();
  // Empty strings are valid logical argv tokens after quote decoding; only
  // NUL and the Runtime's frozen size bounds are rejected here.
  if (parsed.some((token) => token.includes("\0"))) {
    return { error: "Bash argv prefix tokens cannot contain NUL." };
  }
  if (parsed.some((token) => encoder.encode(token).length > 1024)) {
    return { error: "Each Bash argv prefix token must be at most 1024 UTF-8 bytes." };
  }
  return { value: parsed };
}

function isNormalizedExecutable(value: string): boolean {
  return Boolean(value) && !/[\s"';&|<>$`(){}\[\]]/.test(value) && !value.includes("\0");
}

function toUserRule(editor: RuleEditorState): { rule?: UserPolicyRuleConfig; error?: string } {
  const id = editor.id.trim();
  const name = editor.name.trim();
  const reason = editor.reason.trim();
  if (!id) return { error: "User policy rule ID is required." };
  if (!name) return { error: "User policy rule name is required." };
  if (!reason) return { error: "User policy rule reason is required." };
  let matcher: PolicyMatcherConfig;
  if (editor.matcherType === "tool") {
    if (!configurableTools.includes(editor.tool)) return { error: "Choose a supported built-in tool matcher." };
    matcher = { type: "tool", tool: editor.tool };
  } else {
    const executable = editor.executable.trim();
    if (!isNormalizedExecutable(executable)) {
      return { error: "Bash executable must be one normalized command token without shell operators." };
    }
    const prefix = parseArgvPrefix(editor.argvPrefixText);
    if (prefix.error || !prefix.value) return { error: prefix.error ?? "Bash argv prefix is invalid." };
    matcher = { type: "bash_command", executable, argvPrefix: prefix.value };
  }
  return {
    rule: { id, name, enabled: editor.enabled, decision: editor.decision, matcher, reason },
  };
}

function matcherLabel(matcher: PolicyMatcherConfig | undefined): string {
  if (!matcher) return "system guardrail";
  if (matcher.type === "tool") return `tool:${matcher.tool}`;
  const suffix = matcher.argvPrefix.length ? ` ${matcher.argvPrefix.join(" ")}` : "";
  return `bash command:${matcher.executable}${suffix}`;
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

export function PoliciesPage({ initialDraft = null }: { initialDraft?: PolicyDraftHandoff | null }) {
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
  }, [data.config, dirty]);

  useEffect(() => {
    if (!initialDraft) return;
    setEditor(editorFromDraft(initialDraft));
    setEditingId(null);
    setError("");
    setMessage("Approval scope prepared as a policy draft. Review and save it explicitly.");
  }, [initialDraft]);

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
    const result = toUserRule(editor);
    if (result.error || !result.rule) {
      setError(result.error ?? "User policy rule is invalid.");
      return;
    }
    const nextRule = result.rule;
    const duplicate = draft.userRules.some((rule) => rule.id === nextRule.id && rule.id !== editingId);
    if (duplicate) {
      setError(`Duplicate user policy rule ID '${nextRule.id}'.`);
      return;
    }
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
      eyebrow="M5 controlled bash"
      title="Policies"
      description="Edit the next Runtime generation's desired policy and observe the current generation's effective decisions. Runtime compilation remains authoritative."
      action={<StatusBadge tone={effective?.valid === false ? "danger" : "neutral"}>{effective ? (effective.valid ? "Runtime snapshot valid" : "Fail closed") : "Runtime not observed"}</StatusBadge>}
    />

    {data.restartRequired && <div className="notice notice-warning" role="status"><strong>Restart Required</strong><span>Desired policy is saved, but the current Runtime generation is still using its previous snapshot.</span><button className="button button-warning" onClick={() => void data.restartHarness()}>Restart Harness</button></div>}
    {error && <div className="notice notice-danger" role="alert"><strong>Policy save failed</strong><span>{error}</span></div>}
    {message && <div className="notice notice-success" role="status">{message}</div>}

    <Card className="settings-section">
      <SectionHeading eyebrow="Desired configuration" title="Tool Defaults" description="A default is used only when no enabled User Rule matches. Bash defaults to ASK and remains subject to Runtime analyzer risk floors." />
      <div className="observation-table-wrap"><table className="observation-table"><thead><tr><th scope="col">Tool</th><th scope="col">Desired default</th><th scope="col">Current effective</th><th scope="col">Reason</th></tr></thead><tbody>
        {defaultToolRows.map((tool) => {
          const effectiveTool = effectiveByTool.get(tool.name);
          return <tr key={tool.name}>
            <td><span className="mono">{tool.name}</span>{tool.name === "bash" && <><br /><span className="muted">High risk · Developer/Test capability</span></>}</td>
            <td><label><span className="sr-only">Desired default for {tool.name}</span><select aria-label={`Desired default for ${tool.name}`} value={draft.toolDefaults[tool.key]} disabled={saving} onChange={(event) => { setDraft((current) => ({ ...current, toolDefaults: { ...current.toolDefaults, [tool.key]: event.target.value as PolicyDecision } })); setDirty(true); setMessage(""); }}>{decisions.map((decision) => <option key={decision} value={decision}>{labels[decision]}</option>)}</select></label></td>
            <td>{effectiveTool ? labels[effectiveTool.decision] : "NOT OBSERVED"}</td>
            <td>{effectiveTool ? reasonLabel(effectiveTool.reason) : "No active Runtime generation."}</td>
          </tr>;
        })}
      </tbody></table></div>
      <div className="settings-actions"><span className={dirty ? "warning-text" : "muted"}>{dirty ? "Unsaved policy changes" : "Desired policy saved"}</span><button className="button button-primary" disabled={!dirty || saving} onClick={() => void saveDraft()}>{saving ? "Saving…" : "Save Policy Changes"}</button></div>
    </Card>

    <Card className="settings-section">
      <SectionHeading eyebrow="Desired configuration" title="User Rules" description="Rules are persisted as typed matchers. Bash Command prefixes match exact normalized tokens; they are not regular expressions or substrings." />
      <p className="settings-help muted">Dynamic or unclassified Bash expressions stay at least ASK. A Tool-level Bash ALLOW is not a filesystem sandbox and does not bypass Runtime approval or risk floors.</p>
      {draft.userRules.length === 0 ? <p className="muted">No desired User Rules configured.</p> : <div className="policy-list">{draft.userRules.map((rule) => <DesiredRuleRow key={rule.id} rule={rule} disabled={saving} onEdit={() => { setEditor(ruleEditor(rule)); setEditingId(rule.id); setError(""); }} onToggle={() => void toggleRule(rule)} onDelete={() => void deleteRule(rule)} />)}</div>}
      <div className="policy-editor">
        <h3>{editingId ? "Edit User Rule" : "Create User Rule"}</h3>
        {initialDraft && <div className="notice notice-warning" role="note"><strong>Approval handoff</strong><span>Current approval scope: <span className="mono">{initialDraft.scope}</span>. {initialDraft.matcher?.type === "bash_command" ? "This Bash Command matcher is a reviewable exact-token draft; creating a rule never resolves the current approval." : initialDraft.tool === "bash" ? "This starts from a Bash Tool ASK draft; review it before saving because creating a rule never resolves the current approval." : "The persistent M3 tool rule below is broader than this exact scope; review it before saving."}</span></div>}
        <div className="form-row">
          <label className="form-field grow"><span>Rule ID</span><input aria-label="User rule ID" value={editor.id} readOnly={Boolean(editingId)} onChange={(event) => setEditor((current) => ({ ...current, id: event.target.value }))} placeholder="e.g. review-writes" /></label>
          <label className="form-field grow"><span>Name</span><input aria-label="User rule name" value={editor.name} onChange={(event) => setEditor((current) => ({ ...current, name: event.target.value }))} placeholder="Review writes" /></label>
        </div>
        <div className="form-row">
          <label className="form-field"><span>Matcher type</span><select aria-label="User rule matcher type" value={editor.matcherType} onChange={(event) => setEditor((current) => ({ ...current, matcherType: event.target.value as MatcherKind }))}><option value="tool">Tool</option><option value="bash_command">Bash Command</option></select></label>
          {editor.matcherType === "tool" ? <label className="form-field"><span>Tool matcher</span><select aria-label="User rule tool matcher" value={editor.tool} onChange={(event) => setEditor((current) => ({ ...current, tool: event.target.value as ConfigurableTool }))}>{configurableTools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}</select></label> : <label className="form-field grow"><span>Bash executable</span><input aria-label="Bash executable" value={editor.executable} onChange={(event) => setEditor((current) => ({ ...current, executable: event.target.value }))} placeholder="git" /></label>}
          <label className="form-field"><span>Decision</span><select aria-label="User rule decision" value={editor.decision} onChange={(event) => setEditor((current) => ({ ...current, decision: event.target.value as PolicyDecision }))}>{decisions.map((decision) => <option key={decision} value={decision}>{labels[decision]}</option>)}</select></label>
          <label className="form-field"><span>Enabled</span><input aria-label="User rule enabled" type="checkbox" checked={editor.enabled} onChange={(event) => setEditor((current) => ({ ...current, enabled: event.target.checked }))} /></label>
        </div>
        {editor.matcherType === "bash_command" && <label className="form-field"><span>Bash argv prefix tokens (JSON array)</span><textarea aria-label="Bash argv prefix tokens" rows={2} value={editor.argvPrefixText} onChange={(event) => setEditor((current) => ({ ...current, argvPrefixText: event.target.value }))} placeholder='["status", "--short"]' /><small>Exact normalized token prefix; this is not a regex or substring. Use [] for any argv after the executable.</small></label>}
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
  return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.enabled ? tone(rule.decision) : "neutral"}>{rule.enabled ? labels[rule.decision] : "DISABLED"}</StatusBadge></div><p>{rule.reason}</p><span className="muted mono">{rule.id} · {matcherLabel(rule.matcher)}</span></div><div className="policy-actions"><button className="button button-secondary" disabled={disabled} onClick={onEdit}>Edit</button><button className="button button-secondary" disabled={disabled} onClick={onToggle}>{rule.enabled ? "Disable" : "Enable"}</button><button className="button button-quiet danger-text" disabled={disabled} onClick={onDelete}>Delete</button></div></Card>;
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
  return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.source === "system" ? "neutral" : tone(rule.decision)}>{rule.source}</StatusBadge></div><p>{rule.reason}</p></div><div className="policy-facts"><span><small>Source</small><b>{rule.source}</b></span><span><small>Matcher</small><b className="mono">{matcherLabel(rule.matcher)}</b></span><span><small>Decision</small><b>{labels[rule.decision]}</b></span><span><small>Mode</small><b>Read-only</b></span></div></Card>;
}
