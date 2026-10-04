import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PolicyEffectiveSnapshot, ToolStatusReason } from "@protocol/v1";
import en from "@/i18n/en.json";
import zhCN from "@/i18n/zh-CN.json";
import { initializeLanguage, i18n } from "@/i18n";
import { HarnessDataProvider } from "@/app/providers/HarnessDataProvider";
import { ActivityPage } from "@/pages/activity/ActivityPage";
import { PoliciesPage } from "@/pages/policies/PoliciesPage";
import { SettingsPage } from "@/pages/settings/SettingsPage";
import { ToolsPage } from "@/pages/tools/ToolsPage";
import { parseActivity } from "@/services/harness/control-read";
import { TauriHarnessControlClient } from "@/services/harness/local-client";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import { toolRemediationKey, toolStatusReasonKey } from "@/i18n/presentation";

/*
 * M7 independent UI Integration gate.  The pages are mounted with the real
 * HarnessDataProvider and production page/status components.  The host
 * boundary is deterministic only so this suite can assert the UI contract;
 * raw control-read values are passed through the production Tauri parser
 * before the provider receives them.
 */
const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
const tauri = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => state.client,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: tauri.listen }));

function renderWithProvider(element: React.ReactElement) {
  return render(<HarnessDataProvider>{element}</HarnessDataProvider>);
}

function flatten(value: Record<string, unknown>, prefix = ""): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof child === "string") result[path] = child;
    else Object.assign(result, flatten(child as Record<string, unknown>, path));
  }
  return result;
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{\{\s*([^},\s]+)(?:,[^}]*)?\s*\}\}/g)].map((match) => match[1]).sort();
}

beforeEach(async () => {
  await initializeLanguage("en");
  document.documentElement.lang = "en";
  state.client = new MockHarnessControlClient("ready");
  tauri.invoke.mockReset();
  tauri.listen.mockReset();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("M7 Integration — typed remediation and invalid Policy UX", () => {
  it("INT-33: preserves machine reason through the local parser and renders deterministic production remediation", async () => {
    const rawTools = [
      { name: "read_file", availability: "available", decision: "allow", statusReason: "available" },
      { name: "glob", availability: "available_with_approval", decision: "ask", statusReason: "approval_required" },
      { name: "write", availability: "disabled", decision: "deny", statusReason: "tool_disabled" },
      { name: "edit", availability: "unavailable_by_policy", decision: "deny", statusReason: "policy_denied" },
      { name: "bash", availability: "unavailable_by_runtime", decision: "deny", statusReason: "bash_executable_unavailable", readiness: "unavailable", reason: "Git Bash was not ready." },
    ];
    tauri.invoke.mockResolvedValue(rawTools);
    const parsed = await new TauriHarnessControlClient().runtimeGetEffectiveTools();
    expect(parsed.map((tool) => tool.statusReason)).toEqual([
      "available",
      "approval_required",
      "tool_disabled",
      "policy_denied",
      "bash_executable_unavailable",
    ]);

    const allReasons: ToolStatusReason[] = [
      "available",
      "approval_required",
      "tool_disabled",
      "developer_mode_required",
      "runtime_not_ready",
      "capability_unavailable",
      "bash_executable_unavailable",
      "policy_denied",
      "invalid_policy_snapshot",
      "invalid_policy_context",
      "unsupported_capability",
      "metadata_invalid",
    ];
    for (const reason of allReasons) {
      const reasonKey = toolStatusReasonKey(reason);
      const remediationKey = toolRemediationKey(reason);
      expect(reasonKey).toMatch(/^m7\.reasons\./);
      expect(remediationKey).toMatch(/^m7\.remediation\./);
      expect(i18n.t(reasonKey)).not.toBe(reasonKey);
      expect(i18n.t(remediationKey)).not.toBe(remediationKey);
    }

    vi.spyOn(state.client, "runtimeGetEffectiveTools").mockResolvedValue(parsed);
    renderWithProvider(<ToolsPage />);
    expect(await screen.findByText("Tool disabled")).toBeInTheDocument();
    expect(screen.getByText("Enable the tool, save, and restart Harness.")).toBeInTheDocument();
    expect(screen.getByText("Approval required")).toBeInTheDocument();
    expect(screen.getByText("Invoke the tool and resolve the request in Approval Center.")).toBeInTheDocument();
    expect(screen.getByText("Denied by Policy")).toBeInTheDocument();
    expect(screen.getByText("Review the matched User Policy rules, save changes, and restart Harness.")).toBeInTheDocument();
    expect(screen.getByText("Git Bash unavailable")).toBeInTheDocument();
    expect(screen.getByText("Configure or auto-detect a valid Git Bash executable, then restart Harness.")).toBeInTheDocument();
  });

  it("INT-34: invalid effective Policy stays fail closed and points to Policy repair", async () => {
    const current = await state.client.policiesList();
    const effective = current.effective as PolicyEffectiveSnapshot;
    const invalid: PolicyEffectiveSnapshot = {
      ...effective,
      valid: false,
      status: "invalid",
      statusReason: "invalid_policy_snapshot",
      systemRules: [],
      userRules: [],
      perTool: effective.perTool.map((tool) => ({
        ...tool,
        decision: "deny",
        availability: "unavailable_by_policy",
        reason: "invalid_policy_snapshot",
        matchedRuleIds: [],
      })),
    };
    vi.spyOn(state.client, "policiesList").mockResolvedValue({ ...current, effective: invalid });

    renderWithProvider(<PoliciesPage />);
    expect(await screen.findByText("Invalid policy snapshot — fail closed")).toBeInTheDocument();
    expect(screen.getByText("Policy-controlled tools are denied until a valid snapshot is compiled by the next Runtime generation.")).toBeInTheDocument();
    expect(screen.getAllByText("Repair invalid Policy configuration and restart. Execution remains denied until valid.").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Denied").length).toBeGreaterThan(0);
    expect(screen.queryByText("Runtime snapshot valid")).not.toBeInTheDocument();
  });
});

describe("M7 Integration — revision truth and Activity evidence", () => {
  it("INT-35: a Runtime-affecting save separates desired config from the active snapshot", async () => {
    renderWithProvider(<SettingsPage />);
    await screen.findByDisplayValue("D:\\example-project");
    const tunnel = screen.getByRole("textbox", { name: "Tunnel ID (required)" });
    fireEvent.change(tunnel, { target: { value: "saved-but-not-active" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));

    expect(await screen.findByText("Changes saved.")).toBeInTheDocument();
    const revision = screen.getByRole("region", { name: "Configuration revisions" });
    await waitFor(() => expect(revision).toHaveTextContent("Saved revision: 8"));
    expect(revision).toHaveTextContent("Active revision: 7");
    expect(revision).toHaveTextContent("Restart Required");
  });

  it("INT-36: a UI-only language save does not manufacture Runtime restart debt", async () => {
    const before = await state.client.configGet();
    const lifecycle = ["runtimeStart", "runtimeStop", "harnessStart", "harnessStop", "harnessRestart"]
      .map((method) => vi.spyOn(state.client, method as "runtimeStart"));
    renderWithProvider(<SettingsPage />);
    fireEvent.change(await screen.findByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    expect(await screen.findByText("语言偏好已保存。")).toBeInTheDocument();
    expect((await state.client.configGet())?.configRevision).toBe(before?.configRevision);
    expect(screen.queryByText("需要重启")).not.toBeInTheDocument();
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  });

  it("INT-37: Activity detail renders Runtime-supplied hardened evidence and drops raw payload sentinels", async () => {
    const raw = {
      eventId: "event-m7-evidence",
      requestId: "request-m7-evidence",
      timestamp: "2026-10-04T01:02:03Z",
      category: "execution",
      lifecycleState: "failed",
      eventType: "execution_failed",
      toolName: "write",
      decisionSummary: "ask",
      matchedRuleIds: ["rule-m7-review"],
      policyReason: "matched_user_rules",
      outcomeSummary: "failed",
      failureCategory: "execution_failed",
      toolEnabled: true,
      capability: "workspace_write",
      sanitizedArguments: { tool: "write", logicalPath: "notes.txt", content: "CONTENT_SENTINEL" },
      approvalId: "approval-m7-evidence",
      approvalDecision: "allow_once",
      durationMs: 42,
      resultEvidence: { tool: "write", bytesWritten: 7, disposition: "created", content: "RESULT_SENTINEL" },
    };
    const entries = parseActivity([raw]);
    expect(JSON.stringify(entries)).not.toContain("SENTINEL");
    vi.spyOn(state.client, "runtimeGetActivitySnapshot").mockResolvedValue(entries);

    const { container } = renderWithProvider(<ActivityPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Diagnostic detail" }));
    expect(screen.getByText("request-m7-evidence")).toBeInTheDocument();
    expect(screen.getByText(/logicalPath.*notes\.txt/)).toBeInTheDocument();
    expect(screen.getByText("rule-m7-review")).toBeInTheDocument();
    expect(screen.getByText("approval-m7-evidence")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText(/bytesWritten/)).toBeInTheDocument();
    expect(screen.getAllByText("Execution failed").length).toBeGreaterThan(0);
    expect(container).not.toHaveTextContent("CONTENT_SENTINEL");
    expect(container).not.toHaveTextContent("RESULT_SENTINEL");

    await act(async () => { await i18n.changeLanguage("zh-CN"); });
    expect(screen.getByRole("button", { name: "诊断详情" })).toBeInTheDocument();
    expect(screen.getByText("request-m7-evidence")).toBeInTheDocument();
    expect(screen.getByText(/logicalPath.*notes\.txt/)).toBeInTheDocument();
  });
});

describe("M7 Integration — bilingual presentation and M6 regression", () => {
  it("INT-38: M7 resources have exact key and placeholder parity in both supported locales", () => {
    const english = flatten(en.m7 as Record<string, unknown>);
    const chinese = flatten(zhCN.m7 as Record<string, unknown>);
    expect(Object.keys(chinese).sort()).toEqual(Object.keys(english).sort());
    for (const [key, value] of Object.entries(english)) {
      expect(value.trim(), key).not.toBe("");
      expect(chinese[key].trim(), key).not.toBe("");
      expect(placeholders(chinese[key]), key).toEqual(placeholders(value));
      expect(i18n.t(`m7.${key}`)).not.toBe(`m7.${key}`);
    }
  });

  it("INT-40 M6 language: instant switch preserves revision and does not call lifecycle mutation", async () => {
    const before = await state.client.configGet();
    const lifecycle = ["runtimeStart", "runtimeStop", "harnessStart", "harnessStop", "harnessRestart"]
      .map((method) => vi.spyOn(state.client, method as "runtimeStart"));
    renderWithProvider(<SettingsPage />);
    fireEvent.change(await screen.findByRole("combobox", { name: "Language" }), { target: { value: "zh-CN" } });
    expect(await screen.findByText("语言偏好已保存。")).toBeInTheDocument();
    expect((await state.client.configGet())?.configRevision).toBe(before?.configRevision);
    expect(document.documentElement.lang).toBe("zh-CN");
    lifecycle.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  });
});
