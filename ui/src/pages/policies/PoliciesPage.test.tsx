import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, PolicyEffectiveSnapshot, PoliciesListResult, UserPolicyRuleConfig } from "@protocol/v1";
import { HarnessClientError } from "@/services/harness/errors";
import { i18n } from "@/i18n";
import { DraftNavigationProvider, useDraftNavigation } from "@/app/router/DraftNavigation";
import { PoliciesPage } from "./PoliciesPage";

const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => state.context }));
afterEach(() => cleanup());

function config(userRules: UserPolicyRuleConfig[] = []): AppConfig {
  return {
    schemaVersion: 3,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: { readFile: { enabled: true }, glob: { enabled: true }, grep: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
    bash: { autoDetect: true },
    policies: { toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules },
    desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false },
    history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
    search: { globTimeoutSeconds: 30, grepTimeoutSeconds: 30 },
  };
}

function effective(valid = true): PolicyEffectiveSnapshot {
  return {
    valid,
    status: valid ? "valid" : "invalid",
    statusReason: valid ? undefined : "invalid_policy_snapshot",
    systemRules: [{ id: "system-read-workspace", name: "Allow workspace reads", decision: "allow", reason: "Workspace baseline", source: "system", editable: false, enabled: true }],
    userRules: [],
    toolDefaults: { readFile: "allow", glob: "allow", grep: "allow", write: "allow", edit: "allow", bash: "ask" },
    perTool: [{ tool: "read_file", decision: valid ? "allow" : "deny", matchedRuleIds: valid ? [] : ["invalid-rule"], reason: valid ? "tool_default" : "invalid_policy_snapshot", availability: valid ? "available" : "unavailable_by_policy" }],
  };
}

function setup({ userRules = [], snapshot = effective(), onNavigate, configValue, withNavigation = false }: { userRules?: UserPolicyRuleConfig[]; snapshot?: PolicyEffectiveSnapshot; onNavigate?: (page: string, options?: unknown) => void; configValue?: AppConfig | null; withNavigation?: boolean } = {}) {
  let current = config(userRules);
  const mutateConfig = vi.fn(async (intent: (latest: AppConfig) => AppConfig) => {
    try {
      current = intent(current);
      state.context.config = current;
      return { ok: true };
    } catch (error) {
      return { ok: false, error: error instanceof HarnessClientError ? error : new HarnessClientError("error", "save rejected", undefined, "CONFIG_WRITE_FAILED") };
    }
  });
  state.context = {
    config: configValue === undefined ? current : configValue,
    policies: { status: "success", data: { items: [], effective: snapshot } satisfies PoliciesListResult },
    restartRequired: false,
    mutateConfig,
    restartHarness: vi.fn().mockResolvedValue({ ok: true }),
  };
  const view = render(withNavigation ? <DraftNavigationProvider><PoliciesPage onNavigate={onNavigate as any} /></DraftNavigationProvider> : <PoliciesPage onNavigate={onNavigate as any} />);
  return { mutateConfig, getConfig: () => current, rerender: view.rerender };
}

function NavigationProbe() {
  const requestLeave = useDraftNavigation().requestLeave;
  return <button type="button" onClick={() => requestLeave(() => undefined)}>Sidebar navigation</button>;
}

beforeEach(async () => { await i18n.changeLanguage("en"); });

describe("M8 PoliciesPage", () => {
  it("keeps configured defaults, user rules, read-only guardrails, and effective evidence in separate layers", () => {
    setup();
    expect(screen.getByText("Tool defaults")).toBeInTheDocument();
    expect(screen.getByText("User rules")).toBeInTheDocument();
    expect(screen.getByText("System guardrails")).toBeInTheDocument();
    expect(screen.getByText("Effective policy diagnostics")).toBeInTheDocument();
    expect(screen.getAllByText("Allow workspace capabilities").length).toBeGreaterThan(0);
    expect(screen.getByText("Current generation")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
  });

  it("auto-saves a default through scoped mutateConfig and retains desired/effective distinction", async () => {
    const { mutateConfig } = setup();
    const group = screen.getByRole("radiogroup", { name: "Desired default for write" });
    fireEvent.click(within(group).getByRole("radio", { name: "DENY" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    expect((mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig)(config()).policies.toolDefaults.write).toBe("deny");
    expect(screen.getAllByText(/Current Runtime/).length).toBeGreaterThan(0);
  });

  it("creates a new rule only from an explicit scope, starts ASK, and preserves the generated ID for retry", async () => {
    const { mutateConfig, getConfig } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Add rule" }));
    const dialog = screen.getByRole("dialog");
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: "ASK" })).toHaveAttribute("aria-checked", "true");
    expect(within(dialog).getByRole("button", { name: "Create rule" })).toBeDisabled();
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Scope" }), { target: { value: "tool" } });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Tool" }), { target: { value: "write" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create rule" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const created = getConfig().policies.userRules[0];
    expect(created).toMatchObject({ decision: "ask", matcher: { type: "tool", tool: "write" } });
    expect(created.id).toMatch(/^rule-/);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("uses natural Bash prefix parsing and preserves the existing matcher when only metadata changes", async () => {
    const existing: UserPolicyRuleConfig = { id: "stable-rule", name: "Keep identity", enabled: true, decision: "ask", matcher: { type: "bash_command", executable: "git", argvPrefix: ["status", ""] }, reason: "Existing matcher" };
    const { mutateConfig, getConfig } = setup({ userRules: [existing] });
    fireEvent.click(screen.getByRole("button", { name: /Keep identity/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Rule name"), { target: { value: "Renamed only" } });
    fireEvent.click(screen.getByRole("button", { name: "Save rule" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    expect(getConfig().policies.userRules[0]).toMatchObject({ id: "stable-rule", name: "Renamed only", matcher: existing.matcher });
  });

  it("rejects unsupported natural syntax and exposes Advanced literal token editing", () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Add rule" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Scope" }), { target: { value: "natural" } });
    fireEvent.change(screen.getByLabelText("Bash command"), { target: { value: "git status && pwd" } });
    expect(screen.getByText("This command uses unsupported shell syntax. Use Advanced literal matcher.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open Advanced literal editor" }));
    fireEvent.change(screen.getByLabelText("Normalized executable"), { target: { value: "git" } });
    fireEvent.click(screen.getByRole("button", { name: "Add token" }));
    expect(screen.getByLabelText("Argument token 1")).toHaveValue("");
  });

  it("pauses same-rule Enabled while editing and sends an explicit boolean for other queued toggles", async () => {
    const existing: UserPolicyRuleConfig = { id: "rule-write", name: "Write review", enabled: true, decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "Review writes" };
    const { mutateConfig } = setup({ userRules: [existing] });
    fireEvent.click(screen.getByRole("button", { name: /Write review/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const sameRuleSwitch = screen.getByRole("switch", { name: /Write review/ });
    expect(sameRuleSwitch).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(screen.getByRole("switch", { name: /Write review/ }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const next = (mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig)(config([existing]));
    expect(next.policies.userRules[0].enabled).toBe(false);
  });

  it("requires target-ID confirmation before deleting a rule", async () => {
    const existing: UserPolicyRuleConfig = { id: "delete-me", name: "Delete me", enabled: true, decision: "deny", matcher: { type: "tool", tool: "edit" }, reason: "cleanup" };
    const { mutateConfig } = setup({ userRules: [existing] });
    fireEvent.click(screen.getByRole("button", { name: /Delete me/ }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    expect(screen.getByRole("dialog")).toHaveTextContent("delete-me");
    expect(mutateConfig).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("dialog").querySelector("button.button-danger")!);
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledOnce());
    const next = (mutateConfig.mock.calls[0][0] as (latest: AppConfig) => AppConfig)(config([existing]));
    expect(next.policies.userRules).toEqual([]);
  });

  it("keeps an edit draft on CONFIG_CONFLICT and offers Reload", async () => {
    const existing: UserPolicyRuleConfig = { id: "conflict", name: "Conflict", enabled: true, decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "before" };
    const { mutateConfig } = setup({ userRules: [existing] });
    fireEvent.click(screen.getByRole("button", { name: /Conflict/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Reason"), { target: { value: "draft" } });
    mutateConfig.mockImplementationOnce(async () => ({ ok: false, error: new HarnessClientError("error", "This object changed. Reload before saving.", undefined, "CONFIG_CONFLICT") }));
    fireEvent.click(screen.getByRole("button", { name: "Save rule" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument());
    expect(screen.getByLabelText("Reason")).toHaveValue("draft");
  });

  it("returns an Approval handoff to its request after Cancel without creating a rule", () => {
    const onNavigate = vi.fn();
    const { mutateConfig } = setup({ onNavigate });
    cleanup();
    render(<PoliciesPage onNavigate={onNavigate as any} initialDraft={{ tool: "bash", scope: "dynamic command", name: "Review Bash", reason: "Review dynamic Bash", approvalRequestId: "approval-1" }} />);
    expect(screen.getByRole("combobox", { name: "Scope" })).toHaveValue("choose");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onNavigate).toHaveBeenCalledWith("approvals", { objectId: "approval-1" });
    expect(mutateConfig).not.toHaveBeenCalled();
  });

  it("does not present fallback defaults or an empty User Rules state before config is confirmed", () => {
    setup({ configValue: null });
    expect(screen.getByRole("status")).toHaveTextContent("Saved configuration is not confirmed");
    expect(screen.getAllByText(/Saved value not confirmed/).length).toBeGreaterThan(0);
    expect(screen.getByText(i18n.t("m8Policies.configUnavailable"), { selector: ".empty-inline" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add rule" })).toBeDisabled();
    expect(screen.queryByRole("radiogroup", { name: "Desired default for write" })).not.toBeInTheDocument();
  });

  it("shows authoritative human guardrail names and reasons while keeping IDs in diagnostics", () => {
    setup();
    expect(screen.getAllByText("Allow workspace capabilities").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Workspace capability baseline.").length).toBeGreaterThan(0);
    expect(screen.getByText("system-read-workspace")).toBeInTheDocument();
    expect(screen.getByText("Configuration revision details")).toBeInTheDocument();
  });

  it("detects an observed same-rule change immediately and preserves a disabled draft Save", () => {
    const first: UserPolicyRuleConfig = { id: "observed", name: "Observed", enabled: true, decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "before" };
    const second: UserPolicyRuleConfig = { id: "other", name: "Other", enabled: true, decision: "deny", matcher: { type: "tool", tool: "edit" }, reason: "other" };
    const { rerender } = setup({ userRules: [first, second] });
    fireEvent.click(screen.getByRole("button", { name: /Observed/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Reason"), { target: { value: "draft remains" } });
    state.context.config = config([{ ...first, reason: "external change" }, second]);
    rerender(<PoliciesPage />);
    expect(screen.getByRole("button", { name: "Save rule" })).toBeDisabled();
    expect(screen.getByLabelText("Reason")).toHaveValue("draft remains");
    expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
  });

  it("guards dirty Close, Esc, sidebar navigation, and Delete through the shared draft guard", () => {
    const rule: UserPolicyRuleConfig = { id: "dirty", name: "Dirty rule", enabled: true, decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "before" };
    cleanup();
    state.context = {
      ...state.context,
      config: config([rule]),
      policies: { status: "success", data: { items: [], effective: effective() } },
      mutateConfig: vi.fn(),
    };
    render(<DraftNavigationProvider><PoliciesPage /><NavigationProbe /></DraftNavigationProvider>);
    fireEvent.click(screen.getByRole("button", { name: /Dirty rule/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Reason"), { target: { value: "dirty" } });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getAllByRole("dialog").at(-1)).toHaveTextContent("Discard changes?");
    fireEvent.click(screen.getByRole("button", { name: "Stay editing" }));
    expect(screen.getByLabelText("Reason")).toHaveValue("dirty");
    fireEvent.keyDown(document.querySelector(".context-surface")!, { key: "Escape" });
    expect(screen.getAllByRole("dialog").at(-1)).toHaveTextContent("Discard changes?");
    fireEvent.click(screen.getByRole("button", { name: "Stay editing" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    expect(screen.getAllByRole("dialog").at(-1)).toHaveTextContent("Discard changes?");
    fireEvent.click(screen.getByRole("button", { name: "Stay editing" }));
    fireEvent.click(screen.getByRole("button", { name: "Sidebar navigation" }));
    expect(screen.getAllByRole("dialog").at(-1)).toHaveTextContent("Discard changes?");
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(screen.getByLabelText("Reason")).toHaveValue("dirty");
  });

  it("does not clear an initialRuleId editor when another object saves", () => {
    const first: UserPolicyRuleConfig = { id: "initial", name: "Initial", enabled: true, decision: "ask", matcher: { type: "tool", tool: "write" }, reason: "before" };
    const second: UserPolicyRuleConfig = { id: "other", name: "Other", enabled: true, decision: "deny", matcher: { type: "tool", tool: "edit" }, reason: "other" };
    const current = config([first, second]);
    state.context = { config: current, policies: { status: "success", data: { items: [], effective: effective() } }, mutateConfig: vi.fn(), restartHarness: vi.fn() };
    const view = render(<PoliciesPage initialRuleId="initial" />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Rule name"), { target: { value: "Draft name" } });
    state.context.config = config([first, { ...second, reason: "saved elsewhere" }]);
    view.rerender(<PoliciesPage initialRuleId="initial" />);
    expect(screen.getByLabelText("Rule name")).toHaveValue("Draft name");
  });

  it("keeps the Delete dialog target and local feedback after a failed delete", async () => {
    const rule: UserPolicyRuleConfig = { id: "delete-fail", name: "Keep on failure", enabled: true, decision: "deny", matcher: { type: "tool", tool: "edit" }, reason: "cleanup" };
    const { mutateConfig } = setup({ userRules: [rule] });
    mutateConfig.mockResolvedValueOnce({ ok: false, error: new HarnessClientError("error", "delete rejected", undefined, "CONFIG_WRITE_FAILED") });
    fireEvent.click(screen.getByRole("button", { name: /Keep on failure/ }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    fireEvent.click(screen.getByRole("dialog").querySelector("button.button-danger")!);
    await waitFor(() => expect(screen.getByRole("dialog")).toHaveTextContent("Delete failed"));
    expect(screen.getByRole("dialog")).toHaveTextContent("Keep on failure");
    expect(screen.getByRole("dialog")).toHaveTextContent("delete-fail");
  });

  it("keeps rapid default mutations independently pending and reports refresh uncertainty beside the row", async () => {
    let releaseFirst!: () => void;
    let releaseSecond!: () => void;
    const firstGate = new Promise<void>((resolve) => { releaseFirst = resolve; });
    const secondGate = new Promise<void>((resolve) => { releaseSecond = resolve; });
    const { mutateConfig } = setup();
    mutateConfig.mockImplementationOnce(async () => { await firstGate; return { ok: true, refreshUnconfirmed: true }; }).mockImplementationOnce(async () => { await secondGate; return { ok: true }; });
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Desired default for write" })).getByRole("radio", { name: "DENY" }));
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Desired default for read_file" })).getByRole("radio", { name: "ASK" }));
    await waitFor(() => expect(mutateConfig).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("radiogroup", { name: "Desired default for write" }).querySelector("button")).toBeDisabled();
    expect(screen.getByRole("radiogroup", { name: "Desired default for read_file" }).querySelector("button")).toBeDisabled();
    releaseFirst();
    releaseSecond();
    await waitFor(() => expect(screen.getByText("Saved; status refresh is unconfirmed.")).toBeInTheDocument());
  });

  it("raises invalid Runtime policy snapshots as Critical evidence", () => {
    setup({ snapshot: effective(false) });
    expect(screen.getByRole("alert")).toHaveTextContent("Critical: invalid Policy snapshot");
    expect(screen.getByRole("alert")).toHaveTextContent("Execution remains denied until valid");
    expect(screen.getByText("invalid-rule")).toBeInTheDocument();
  });
});
