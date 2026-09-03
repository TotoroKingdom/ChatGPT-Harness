import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "@protocol/v1";
import type { RuntimeProcessEvent } from "@/services/harness/client";

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
    expect(client.getComponentStates()).toEqual({
      runtime: "ready",
      tunnel: "stopped",
      mcp: "stopped",
    });
  });

  it.each([
    ["starting", "starting"],
    ["running", "ready"],
    ["stopping", "stopping"],
    ["stopped", "stopped"],
    ["crashed", "error"],
  ] as const)("maps process phase %s without fabricating RuntimeState", async (phase, expected) => {
    let handler: ((event: { payload: RuntimeProcessEvent }) => void) | undefined;
    tauri.listen.mockImplementation(async (name, listener) => {
      expect(name).toBe("runtime:process");
      handler = listener;
      return vi.fn();
    });
    const client = new TauriHarnessControlClient();
    const listener = vi.fn();

    await client.componentStatesSubscribe(listener);
    handler?.({
      payload: {
        phase,
        error:
          phase === "crashed"
            ? { code: "RUNTIME_CRASHED", message: "Runtime process exited unexpectedly." }
            : undefined,
      },
    });

    const states = { runtime: expected, tunnel: "stopped", mcp: "stopped" };
    expect(client.getComponentStates()).toEqual(states);
    expect(listener).toHaveBeenLastCalledWith(states);
  });

  it("registers the activity listener before subscribing from the supplied cursor", async () => {
    const order: string[] = [];
    const unlisten = vi.fn();
    tauri.listen.mockImplementation(async () => {
      order.push("listen");
      return unlisten;
    });
    tauri.invoke.mockImplementation(async () => {
      order.push("invoke");
      return { latestSequence: "12" };
    });
    const client = new TauriHarnessControlClient();

    const unsubscribe = await client.activitySubscribe(vi.fn(), "9");

    expect(order).toEqual(["listen", "invoke"]);
    expect(tauri.invoke).toHaveBeenCalledWith("runtime_activity_subscribe", {
      params: { afterSequence: "9" },
    });
    unsubscribe();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("removes the activity listener when subscribe RPC fails", async () => {
    const unlisten = vi.fn();
    tauri.listen.mockResolvedValue(unlisten);
    tauri.invoke.mockRejectedValue(new Error("disconnected"));
    const client = new TauriHarnessControlClient();

    await expect(client.activitySubscribe(vi.fn(), "3")).rejects.toThrow("disconnected");
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("does not mix M2 mock tools or policies into real M3 results", async () => {
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "runtime_tools_list" || command === "runtime_policies_list") {
        return Promise.resolve({ items: [] });
      }
      throw new Error(`Unexpected command: ${command}`);
    });
    const client = new TauriHarnessControlClient();

    await expect(client.toolsList()).resolves.toEqual({ items: [] });
    await expect(client.policiesList()).resolves.toEqual({ items: [] });
  });
});
