import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, PolicyEffectiveSnapshot, PoliciesListResult } from "@protocol/v1";
import { i18n } from "@/i18n";
import { PoliciesPage } from "./PoliciesPage";

const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => state.context }));
afterEach(() => cleanup());

function config(): AppConfig {
  return {
    schemaVersion: 2,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
    bash: { autoDetect: true },
    policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
  };
}

function effective(): PolicyEffectiveSnapshot {
  return {
    valid: true,
    status: "valid",
    systemRules: [{ id: "system-read-workspace", name: "Allow workspace reads", decision: "allow", reason: "Workspace baseline", source: "system", editable: false, enabled: true }],
    userRules: [],
    toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" },
    perTool: [
      { tool: "read_file", decision: "allow", matchedRuleIds: [], reason: "tool_default", availability: "available" },
      { tool: "write", decision: "allow", matchedRuleIds: [], reason: "tool_default", availability: "available" },
    ],
  };
}

function setup(overrides: Record<string, unknown> = {}) {
  const current = config();
  const saveConfig = vi.fn().mockResolvedValue({ ok: true });
  const policies: PoliciesListResult = { items: [], effective: effective() };
  state.context = {
    config: current,
    policies: { status: "success", data: policies },
    restartRequired: true,
    saveConfig,
    restartHarness: vi.fn().mockResolvedValue({ ok: true }),
    ...overrides,
  };
  render(<PoliciesPage />);
  return { saveConfig, current };
}

describe("M3 PoliciesPage", () => {
  it("shows Runtime effective evidence and restart mismatch without editable system controls", () => {
    setup();
    expect(screen.getByText("Allow workspace capabilities")).toBeInTheDocument();
    expect(screen.getByText("Current generation")).toBeInTheDocument();
    expect(screen.getByText("Restart Required")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
  });

  it("localizes system friendly presentation while preserving raw IDs, matchers, and user text", async () => {
    const current = effective();
    current.userRules = [{
      id: "user-review-writes",
      name: "Keep my exact user name",
      decision: "ask",
      matcher: { type: "tool", tool: "write" },
      reason: "Keep my exact user reason",
      source: "user",
      editable: true,
      enabled: true,
    }];
    setup({ policies: { status: "success", data: { items: [], effective: current } } });

    expect(screen.getByText("Allow workspace capabilities")).toBeInTheDocument();
    expect(screen.getByText("system-read-workspace")).toBeInTheDocument();
    expect(screen.getByText("Keep my exact user name")).toBeInTheDocument();
    expect(screen.getByText("Keep my exact user reason")).toBeInTheDocument();
    expect(screen.getByText("user-review-writes")).toBeInTheDocument();
    expect(screen.getByText("tool:write")).toBeInTheDocument();

    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByText("允许工作区能力")).toBeInTheDocument();
    expect(screen.getByText("工作区能力基线。")).toBeInTheDocument();
    expect(screen.getByText("system-read-workspace")).toBeInTheDocument();
    expect(screen.getByText("Keep my exact user name")).toBeInTheDocument();
    expect(screen.getByText("Keep my exact user reason")).toBeInTheDocument();
    expect(screen.getByText("user-review-writes")).toBeInTheDocument();
    expect(screen.getByText("工具：write")).toBeInTheDocument();
    expect(screen.queryByText("Allow workspace capabilities")).not.toBeInTheDocument();
  });

  it("creates and edits a User Rule while preserving its identity", async () => {
    const { saveConfig, current } = setup();
    fireEvent.change(screen.getByLabelText("User rule ID"), { target: { value: "review-writes" } });
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Review writes" } });
    fireEvent.change(screen.getByLabelText("User rule reason"), { target: { value: "Writes need review" } });
    fireEvent.change(screen.getByLabelText("User rule decision"), { target: { value: "ask" } });
    fireEvent.change(screen.getByLabelText("User rule tool matcher"), { target: { value: "write" } });
    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    await waitFor(() => expect(saveConfig).toHaveBeenCalled());
    const created = saveConfig.mock.calls[0][0] as AppConfig;
    expect(created.policies.userRules[0]).toMatchObject({ id: "review-writes", decision: "ask", matcher: { type: "tool", tool: "write" } });

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByLabelText("User rule ID")).toHaveAttribute("readonly");
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Review all writes" } });
    fireEvent.click(screen.getByRole("button", { name: "Save User Rule" }));
    await waitFor(() => expect(saveConfig).toHaveBeenCalledTimes(2));
    const edited = saveConfig.mock.calls[1][0] as AppConfig;
    expect(edited.policies.userRules[0].id).toBe("review-writes");
    expect(edited.policies.userRules[0].name).toBe("Review all writes");
    expect(current.policies.userRules).toHaveLength(0);
  });

  it("edits a Tool Default and rejects an incomplete rule without pretending to save", async () => {
    const { saveConfig } = setup();
    fireEvent.change(screen.getByLabelText("Desired default for write"), { target: { value: "deny" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Policy Changes" }));
    await waitFor(() => expect(saveConfig).toHaveBeenCalledTimes(1));
    expect((saveConfig.mock.calls[0][0] as AppConfig).policies.toolDefaults.write).toBe("deny");

    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    expect(await screen.findByText("User policy rule ID is required.")).toBeInTheDocument();
    expect(saveConfig).toHaveBeenCalledTimes(1);
  });

  it("hides Restart Required after the effective generation converges", () => {
    setup({ restartRequired: false });
    expect(screen.queryByText("Restart Required")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Desired default for write")).toHaveValue("allow");
  });

  it("rolls back desired policy state when control save fails", async () => {
    const saveConfig = vi.fn().mockResolvedValue({ ok: false, error: { message: "save rejected" } });
    setup({ saveConfig });
    fireEvent.change(screen.getByLabelText("Desired default for write"), { target: { value: "deny" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Policy Changes" }));
    expect(await screen.findByText("save rejected")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText("Desired default for write")).toHaveValue("allow"));
    expect(screen.queryByText("Unsaved policy changes")).not.toBeInTheDocument();
  });

  it("opens an approval handoff as an unsaved broader tool-level draft", () => {
    setup();
    cleanup();
    render(<PoliciesPage initialDraft={{ tool: "write", scope: "notes.txt", name: "Review write approvals", reason: "Review requests for write before allowing execution." }} />);
    expect(screen.getByText("Approval handoff")).toBeInTheDocument();
    expect(screen.getByText(/persistent M3 tool rule below is broader/i)).toBeInTheDocument();
    expect(screen.getByLabelText("User rule tool matcher")).toHaveValue("write");
    expect(screen.getByLabelText("User rule name")).toHaveValue("Review write approvals");
    expect(state.context.saveConfig).not.toHaveBeenCalled();
  });

  it("creates a typed Bash Command matcher with an exact argv prefix", async () => {
    const { saveConfig } = setup();
    fireEvent.change(screen.getByLabelText("User rule matcher type"), { target: { value: "bash_command" } });
    fireEvent.change(screen.getByLabelText("User rule ID"), { target: { value: "review-git-status" } });
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Review git status" } });
    fireEvent.change(screen.getByLabelText("Bash executable"), { target: { value: "git" } });
    fireEvent.change(screen.getByLabelText("Bash argv prefix tokens"), { target: { value: '["status", "--short"]' } });
    fireEvent.change(screen.getByLabelText("User rule reason"), { target: { value: "Inspect status before execution" } });
    fireEvent.change(screen.getByLabelText("User rule decision"), { target: { value: "ask" } });
    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    await waitFor(() => expect(saveConfig).toHaveBeenCalled());
    expect((saveConfig.mock.calls[0][0] as AppConfig).policies.userRules[0]).toMatchObject({
      id: "review-git-status",
      decision: "ask",
      matcher: { type: "bash_command", executable: "git", argvPrefix: ["status", "--short"] },
    });
  });

  it("rejects a Bash matcher that tries to use a shell operator", async () => {
    const { saveConfig } = setup();
    fireEvent.change(screen.getByLabelText("User rule matcher type"), { target: { value: "bash_command" } });
    fireEvent.change(screen.getByLabelText("User rule ID"), { target: { value: "unsafe" } });
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Unsafe" } });
    fireEvent.change(screen.getByLabelText("Bash executable"), { target: { value: "git && push" } });
    fireEvent.change(screen.getByLabelText("User rule reason"), { target: { value: "Must be reviewed" } });
    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    expect(await screen.findByText(/normalized command token/)).toBeInTheDocument();
    expect(saveConfig).not.toHaveBeenCalled();
  });

  it("preserves an empty logical argv token in a Bash Command matcher", async () => {
    const { saveConfig } = setup();
    fireEvent.change(screen.getByLabelText("User rule matcher type"), { target: { value: "bash_command" } });
    fireEvent.change(screen.getByLabelText("User rule ID"), { target: { value: "review-empty-arg" } });
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Review empty arg" } });
    fireEvent.change(screen.getByLabelText("Bash executable"), { target: { value: "printf" } });
    fireEvent.change(screen.getByLabelText("Bash argv prefix tokens"), { target: { value: '["%s", ""]' } });
    fireEvent.change(screen.getByLabelText("User rule reason"), { target: { value: "Preserve logical argv shape" } });
    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    await waitFor(() => expect(saveConfig).toHaveBeenCalled());
    expect((saveConfig.mock.calls[0][0] as AppConfig).policies.userRules[0].matcher).toEqual({
      type: "bash_command",
      executable: "printf",
      argvPrefix: ["%s", ""],
    });
  });

  it("rejects malformed multiline JSON instead of treating lines as separate tokens", async () => {
    const { saveConfig } = setup();
    fireEvent.change(screen.getByLabelText("User rule matcher type"), { target: { value: "bash_command" } });
    fireEvent.change(screen.getByLabelText("User rule ID"), { target: { value: "malformed-prefix" } });
    fireEvent.change(screen.getByLabelText("User rule name"), { target: { value: "Malformed prefix" } });
    fireEvent.change(screen.getByLabelText("Bash executable"), { target: { value: "git" } });
    fireEvent.change(screen.getByLabelText("Bash argv prefix tokens"), { target: { value: '[\n  "status",\n  "--short"\n' } });
    fireEvent.change(screen.getByLabelText("User rule reason"), { target: { value: "Must remain typed JSON" } });
    fireEvent.click(screen.getByRole("button", { name: "Create User Rule" }));
    expect(await screen.findByText(/Bash argv prefix must be a JSON array/)).toBeInTheDocument();
    expect(saveConfig).not.toHaveBeenCalled();
  });

  it("opens an unclassified Bash handoff at an ASK-safe starting decision", () => {
    setup();
    cleanup();
    render(<PoliciesPage initialDraft={{ tool: "bash", scope: "dynamic command", name: "Review Bash approvals", reason: "Review dynamic Bash commands.", matcher: { type: "tool", tool: "bash" } }} />);
    expect(screen.getByLabelText("User rule tool matcher")).toHaveValue("bash");
    expect(screen.getByLabelText("User rule decision")).toHaveValue("ask");
  });
});

it("M7 invalid snapshot presents fail-closed remediation without offering compiled ALLOW as executable", async () => {
  await i18n.changeLanguage("en");
  const snapshot = effective(); snapshot.valid = false; snapshot.status = "invalid"; snapshot.statusReason = "invalid_policy_snapshot";
  snapshot.perTool = [{ tool: "write", decision: "deny", matchedRuleIds: ["bad-rule"], reason: "invalid_policy_snapshot", availability: "unavailable_by_policy" }];
  setup({ policies: { status: "success", data: { items: [], effective: snapshot } } });
  expect(screen.getAllByText("Repair invalid Policy configuration and restart. Execution remains denied until valid.").length).toBeGreaterThan(0);
  expect(screen.getByText("bad-rule")).toBeInTheDocument();
  expect(screen.queryByText("Allow workspace capabilities")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /ignore/i })).not.toBeInTheDocument();
});
