import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "@protocol/v1";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: tauri.listen }));

import { TauriHarnessControlClient } from "@/services/harness/local-client";

function runtimeState(overrides: Partial<RuntimeState> = {}): RuntimeState {
  return {
    runtimeId: "runtime-1",
    startedAt: "2026-09-02T00:00:00Z",
    lifecycle: "ready",
    readiness: "ready",
    configRevision: "7",
    activeWorkspace: "D:\\workspace",
    securityMode: "normal",
    exposedToolCatalog: [],
    activeRequests: 0,
    pendingApprovals: 0,
    runningExecutions: 0,
    ...overrides,
  };
}

describe("TauriHarnessControlClient", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    tauri.listen.mockReset();
  });

  it("maps the real Runtime status returned by Tauri", async () => {
    tauri.invoke.mockResolvedValue(runtimeState());
    const client = new TauriHarnessControlClient();

    await expect(client.runtimeStatus()).resolves.toMatchObject({ runtimeId: "runtime-1" });
    expect(tauri.invoke).toHaveBeenCalledWith("runtime_status", undefined);
    expect(client.getComponentStates()).toEqual({ runtime: "ready", tunnel: "stopped", mcp: "stopped" });
  });

  it.each([
    ["runtime:crashed", "RUNTIME_CRASHED"],
    ["runtime:state", "RUNTIME_DISCONNECTED"],
  ])("maps %s notifications to an error Runtime even after its identity is cleared", async (eventName, code) => {
    const handlers = new Map<string, (event: { payload: RuntimeState }) => void>();
    tauri.listen.mockImplementation(async (name, listener) => {
      handlers.set(name, listener);
      return vi.fn();
    });
    const client = new TauriHarnessControlClient();
    const listener = vi.fn();

    await client.componentStatesSubscribe(listener);
    handlers.get(eventName)?.({
      payload: runtimeState({
        runtimeId: "",
        lifecycle: "error",
        readiness: "degraded",
        lastError: { code, message: "Runtime process exited unexpectedly." },
      }),
    });

    expect(client.getComponentStates()).toEqual({ runtime: "error", tunnel: "stopped", mcp: "stopped" });
    expect(listener).toHaveBeenLastCalledWith(
      { runtime: "error", tunnel: "stopped", mcp: "stopped" },
      expect.objectContaining({ lastError: { code, message: "Runtime process exited unexpectedly." } }),
    );
  });

  it("does not mix M2 mock tools or policies into real M3 results", async () => {
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "runtime_tools_list" || command === "runtime_policies_list") return Promise.resolve({ items: [] });
      throw new Error(`Unexpected command: ${command}`);
    });
    const client = new TauriHarnessControlClient();

    await expect(client.toolsList()).resolves.toEqual({ items: [] });
    await expect(client.policiesList()).resolves.toEqual({ items: [] });
  });
});
