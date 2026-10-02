import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, PolicyEffectiveSnapshot, PoliciesListResult } from "@protocol/v1";
import { PoliciesPage } from "./PoliciesPage";

const state = vi.hoisted(() => ({ context: undefined as any }));
vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => state.context }));
afterEach(() => cleanup());

function config(): AppConfig {
  return {
    schemaVersion: 1,
    configRevision: "7",
    workspace: { rootPath: "D:\\workspace" },
    tunnel: { tunnelId: "tunnel", autoDetect: true },
    security: { mode: "normal", developerTestMode: false },
    ui: {},
    tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
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
    expect(screen.getByText("Allow workspace reads")).toBeInTheDocument();
    expect(screen.getByText("Current generation")).toBeInTheDocument();
    expect(screen.getByText("Restart Required")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
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
});
