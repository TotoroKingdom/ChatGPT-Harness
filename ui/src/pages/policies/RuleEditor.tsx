import type { PolicyDecision, PolicyMatcherConfig, UserPolicyRuleConfig } from "@protocol/v1";
import type { TFunction } from "i18next";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Disclosure, InlineFeedback, Select, Segmented, TextField, Button } from "@/components/common/primitives";
import type { PolicyDraftHandoff } from "@/app/router/routes";
import { naturalCommandText, parseLiteralMatcher, parseNaturalCommand } from "./matcher";
import { policyDecisionKey } from "@/i18n/presentation";

export type RuleMatcherMode = "choose" | "tool" | "natural" | "literal";

export type RuleEditorValue = {
  id: string;
  name: string;
  reason: string;
  decision: PolicyDecision;
  enabled: boolean;
  matcherMode: RuleMatcherMode;
  tool: string;
  naturalCommand: string;
  executable: string;
  argvPrefix: string[];
  matcherTouched: boolean;
  originalMatcher?: PolicyMatcherConfig;
};

export type RuleEditorError = string | { key: string; values?: Record<string, string | number> };

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const tools = ["read_file", "glob", "write", "edit", "bash"] as const;

function cloneMatcher(matcher: PolicyMatcherConfig | undefined): PolicyMatcherConfig | undefined {
  if (!matcher) return undefined;
  return matcher.type === "tool"
    ? { type: "tool", tool: matcher.tool }
    : { type: "bash_command", executable: matcher.executable, argvPrefix: [...matcher.argvPrefix] };
}

export function makeRuleId(existingIds: Iterable<string> = []): string {
  const taken = new Set(existingIds);
  const seed = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
    ? crypto.randomUUID().slice(0, 8)
    : Math.random().toString(36).slice(2, 10);
  let id = `rule-${seed}`;
  let index = 2;
  while (taken.has(id)) id = `rule-${seed}-${index++}`;
  return id;
}

export function newRuleEditor(
  existingIds: Iterable<string> = [],
  seed?: { name?: string; reason?: string; tool?: string; matcher?: PolicyMatcherConfig },
): RuleEditorValue {
  const matcher = cloneMatcher(seed?.matcher);
  if (matcher?.type === "tool") {
    return {
      id: makeRuleId(existingIds),
      name: seed?.name || "Review tool requests",
      reason: seed?.reason || "Require an explicit review before execution.",
      decision: "ask",
      enabled: true,
      matcherMode: "tool",
      tool: matcher.tool,
      naturalCommand: "",
      executable: "",
      argvPrefix: [],
      matcherTouched: true,
      originalMatcher: matcher,
    };
  }
  if (matcher?.type === "bash_command") {
    const natural = naturalCommandText(matcher);
    return {
      id: makeRuleId(existingIds),
      name: seed?.name || "Review Bash requests",
      reason: seed?.reason || "Require an explicit review before execution.",
      decision: "ask",
      enabled: true,
      matcherMode: natural ? "natural" : "literal",
      tool: "bash",
      naturalCommand: natural || "",
      executable: matcher.executable,
      argvPrefix: [...matcher.argvPrefix],
      matcherTouched: true,
      originalMatcher: matcher,
    };
  }
  return {
    id: makeRuleId(existingIds),
    name: seed?.name || (seed?.tool ? `Review ${seed.tool} requests` : "Review tool requests"),
    reason: seed?.reason || "Require an explicit review before execution.",
    decision: "ask",
    enabled: true,
    matcherMode: "choose",
    tool: seed?.tool || "",
    naturalCommand: "",
    executable: "",
    argvPrefix: [],
    matcherTouched: false,
  };
}

export function editorFromRule(rule: UserPolicyRuleConfig): RuleEditorValue {
  const matcher = cloneMatcher(rule.matcher)!;
  if (matcher.type === "tool") {
    return {
      id: rule.id,
      name: rule.name,
      reason: rule.reason,
      decision: rule.decision,
      enabled: rule.enabled,
      matcherMode: "tool",
      tool: matcher.tool,
      naturalCommand: "",
      executable: "",
      argvPrefix: [],
      matcherTouched: false,
      originalMatcher: matcher,
    };
  }
  const natural = naturalCommandText(matcher);
  return {
    id: rule.id,
    name: rule.name,
    reason: rule.reason,
    decision: rule.decision,
    enabled: rule.enabled,
    matcherMode: natural ? "natural" : "literal",
    tool: "bash",
    naturalCommand: natural || "",
    executable: matcher.executable,
    argvPrefix: [...matcher.argvPrefix],
    matcherTouched: false,
    originalMatcher: matcher,
  };
}

export function editorFromDraft(
  draft: PolicyDraftHandoff,
  existingIds: Iterable<string> = [],
): RuleEditorValue {
  return newRuleEditor(existingIds, {
    name: draft.name,
    reason: draft.reason,
    tool: draft.tool,
    matcher: draft.matcher,
  });
}

export function matcherFromEditor(value: RuleEditorValue): { matcher?: PolicyMatcherConfig; error?: string } {
  if (!value.matcherTouched && value.originalMatcher) return { matcher: cloneMatcher(value.originalMatcher) };
  if (value.matcherMode === "tool") {
    if (!tools.includes(value.tool as typeof tools[number])) return { error: "m8Policies.scopeRequired" };
    return { matcher: { type: "tool", tool: value.tool } };
  }
  if (value.matcherMode === "natural") return parseNaturalCommand(value.naturalCommand);
  if (value.matcherMode === "literal") {
    return parseLiteralMatcher(value.executable, JSON.stringify(value.argvPrefix));
  }
  return { error: "m8Policies.scopeRequired" };
}

export function ruleFromEditor(value: RuleEditorValue): { rule?: UserPolicyRuleConfig; error?: RuleEditorError } {
  const { id, name, reason } = value;
  if (!id.trim()) return { error: "validation.policies.ruleIdRequired" };
  if (!name.trim()) return { error: "validation.policies.ruleNameRequired" };
  if (!reason.trim()) return { error: "validation.policies.ruleReasonRequired" };
  const result = matcherFromEditor(value);
  if (result.error || !result.matcher) return { error: result.error || "validation.policies.ruleInvalid" };
  return {
    rule: {
      id,
      name,
      enabled: value.enabled,
      decision: value.decision,
      matcher: result.matcher,
      reason,
    },
  };
}

export function matcherSummary(t: TFunction, matcher: PolicyMatcherConfig | undefined): string {
  if (!matcher) return t("policies.matcher.systemGuardrail");
  if (matcher.type === "tool") return t("policies.matcher.tool", { tool: matcher.tool });
  const displayToken = (token: string) => token === "" || /\s/.test(token) || !/^[\p{L}\p{N}_\-./:@+%,]+$/u.test(token) ? JSON.stringify(token) : token;
  const command = [matcher.executable, ...matcher.argvPrefix].map(displayToken).join(" ");
  return matcher.argvPrefix.length
    ? t("m8Policies.prefixSummary", { command, executable: matcher.executable })
    : t("m8Policies.emptyPrefixSummary", { executable: matcher.executable });
}

function tx(t: TFunction, key: string, fallback: string, values?: Record<string, string | number>) {
  return t(key, { ...values, defaultValue: fallback });
}

function editorError(t: TFunction, error: RuleEditorError | undefined): string | undefined {
  if (!error) return undefined;
  if (typeof error === "string") return t(error, { defaultValue: error });
  return t(error.key, { ...error.values, defaultValue: error.key });
}

export function RuleEditor({
  value,
  onChange,
  onSave,
  onCancel,
  saving = false,
  blocked = false,
  error,
  mode,
}: {
  value: RuleEditorValue;
  onChange: (next: RuleEditorValue) => void;
  onSave: () => void;
  onCancel: () => void;
  saving?: boolean;
  blocked?: boolean;
  error?: RuleEditorError;
  mode: "create" | "edit";
}) {
  const { t } = useTranslation();
  const preview = useMemo(() => matcherFromEditor(value), [value]);
  const update = (patch: Partial<RuleEditorValue>) => onChange({ ...value, ...patch });
  const selectMode = (matcherMode: RuleMatcherMode) => {
    if (matcherMode === "tool") update({ matcherMode, matcherTouched: true, tool: value.tool || "read_file" });
    else update({ matcherMode, matcherTouched: true });
  };
  const parsedPreview = preview.matcher ? matcherSummary(t, preview.matcher) : undefined;
  const naturalUnsupported = value.matcherMode === "natural" && value.naturalCommand.length > 0 && Boolean(preview.error);

  return <div className="rule-editor">
    <div className="form-row">
      <TextField
        label={tx(t, "m8Policies.ruleName", "Rule name")}
        value={value.name}
        onChange={(event) => update({ name: event.target.value })}
        placeholder={tx(t, "m8Policies.ruleNamePlaceholder", "Review requests")}
        autoFocus={mode === "create"}
      />
      <TextField
        label={tx(t, "m8Policies.ruleReason", "Reason")}
        value={value.reason}
        onChange={(event) => update({ reason: event.target.value })}
        placeholder={tx(t, "m8Policies.ruleReasonPlaceholder", "Require an explicit review before execution.")}
      />
    </div>

    <Select
      label={tx(t, "m8Policies.scope", "Scope")}
      value={value.matcherMode}
      onChange={(event) => selectMode(event.target.value as RuleMatcherMode)}
    >
      <option value="choose">{tx(t, "m8Policies.scopeChoose", "Choose a scope")}</option>
      <option value="tool">{tx(t, "m8Policies.scopeTool", "Entire tool")}</option>
      <option value="natural">{tx(t, "m8Policies.scopeNatural", "Bash command prefix")}</option>
      <option value="literal">{tx(t, "m8Policies.scopeAdvanced", "Advanced literal matcher")}</option>
    </Select>

    {value.matcherMode === "tool" && <Select
      label={tx(t, "m8Policies.tool", "Tool")}
      value={value.tool}
      onChange={(event) => update({ tool: event.target.value, matcherTouched: true })}
    >
      <option value="">{tx(t, "m8Policies.toolChoose", "Choose a tool")}</option>
      {tools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}
    </Select>}

    {value.matcherMode === "natural" && <>
      <TextField
        label={tx(t, "m8Policies.naturalCommand", "Bash command")}
        value={value.naturalCommand}
        onChange={(event) => update({ naturalCommand: event.target.value, matcherTouched: true })}
        placeholder={t("m8Policies.commandExample", { defaultValue: "git status --short" })}
        helper={tx(t, "m8Policies.naturalHelp", "Literal tokens only. Additional arguments remain in scope.")}
        error={naturalUnsupported ? tx(t, "m8Policies.naturalUnsupported", "This command uses unsupported shell syntax. Use Advanced literal matcher.") : undefined}
      />
      {naturalUnsupported && <Button variant="quiet" onClick={() => update({ matcherMode: "literal", executable: "", argvPrefix: [], matcherTouched: true })}>
        {tx(t, "m8Policies.openAdvanced", "Open Advanced literal editor")}
      </Button>}
    </>}

    {value.matcherMode === "literal" && <LiteralMatcherEditor value={value} onChange={onChange} />}

    {value.matcherMode === "choose" && <InlineFeedback error>{tx(t, "m8Policies.scopeRequired", "Choose an explicit rule scope before saving.")}</InlineFeedback>}
    <Segmented
      value={value.decision}
      label={tx(t, "m8Policies.decision", "Decision")}
      options={decisions.map((decision) => ({ value: decision, label: t(policyDecisionKey(decision)) }))}
      onChange={(decision) => update({ decision })}
    />
    {parsedPreview && <div className="notice notice-warning" role="note">
      <strong>{tx(t, "m8Policies.scopePreview", "Scope preview")}</strong>
      <span className="mono">{parsedPreview}</span>
      <span>{tx(t, "m8Policies.guardrailReminder", "This explains matcher scope only; Allow does not override system guardrails, approval, or risk constraints.")}</span>
    </div>}
    {error && <InlineFeedback error>{editorError(t, error)}</InlineFeedback>}

    <Disclosure title={tx(t, "m8Policies.advanced", "Advanced")}>
      <TextField label={tx(t, "m8Policies.ruleId", "Rule ID")} value={value.id} readOnly helper={tx(t, "m8Policies.ruleIdManaged", "Managed by the UI; existing IDs remain stable.")} />
      {value.originalMatcher && <pre className="mono">{JSON.stringify(value.originalMatcher, null, 2)}</pre>}
    </Disclosure>
    <div className="settings-actions">
      <Button variant="quiet" onClick={onCancel} disabled={saving}>{tx(t, "common.cancel", "Cancel")}</Button>
      <Button variant="primary" onClick={onSave} disabled={saving || blocked || !preview.matcher}>{saving ? tx(t, "m8.saving", "Saving…") : mode === "create" ? tx(t, "m8Policies.create", "Create rule") : tx(t, "m8Policies.save", "Save rule")}</Button>
    </div>
  </div>;
}

function LiteralMatcherEditor({ value, onChange }: { value: RuleEditorValue; onChange: (next: RuleEditorValue) => void }) {
  const { t } = useTranslation();
  const updateToken = (index: number, token: string) => onChange({ ...value, argvPrefix: value.argvPrefix.map((current, item) => item === index ? token : current), matcherTouched: true });
  const addToken = () => onChange({ ...value, argvPrefix: [...value.argvPrefix, ""], matcherTouched: true });
  const removeToken = (index: number) => onChange({ ...value, argvPrefix: value.argvPrefix.filter((_, item) => item !== index), matcherTouched: true });
  return <div className="literal-matcher-editor">
    <TextField
      label={t("m8Policies.executable", { defaultValue: "Normalized executable" })}
      value={value.executable}
      onChange={(event) => onChange({ ...value, executable: event.target.value, matcherTouched: true })}
      helper={t("m8Policies.executableHelp", { defaultValue: "The executable is normalized before it is persisted." })}
    />
    <div className="form-field">
      <span>{t("m8Policies.argvPrefix", { defaultValue: "Argument prefix tokens" })}</span>
      {value.argvPrefix.map((token, index) => <div className="form-row" key={index}>
        <TextField label={t("m8Policies.argvToken", { defaultValue: "Argument token {{index}}", index: index + 1 })} value={token} onChange={(event) => updateToken(index, event.target.value)} />
        <Button variant="quiet" className="danger-text" onClick={() => removeToken(index)}>{t("m8Policies.removeToken", { defaultValue: "Remove" })}</Button>
      </div>)}
      <Button variant="quiet" onClick={addToken}>{t("m8Policies.addToken", { defaultValue: "Add token" })}</Button>
      <small className="muted">{t("m8Policies.argvHelp", { defaultValue: "Empty and whitespace tokens are preserved exactly. Up to 64 tokens, 1024 UTF-8 bytes each." })}</small>
    </div>
  </div>;
}
