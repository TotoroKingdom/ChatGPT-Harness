import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "@protocol/v1";
import type { ComponentStates, RuntimeProcessEvent } from "@/services/harness/client";

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

const states = (runtime: ComponentStates["runtime"]): ComponentStates => ({
  runtime,
  tunnel: "stopped",
  mcp: runtime,
});

describe("TauriHarnessControlClient", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    tauri.listen.mockReset();
  });

  it.each([
    ["ready RuntimeState", runtimeState(), states("ready")],
    ["initializing RuntimeState", runtimeState({ lifecycle: "initializing", readiness: "not_ready" }), states("starting")],
    ["degraded RuntimeState", runtimeState({ lifecycle: "ready", readiness: "degraded" }), states("degraded")],
    ["stopped RuntimeState", runtimeState({ runtimeId: "", lifecycle: "initializing", readiness: "not_ready" }), states("stopped")],
    ["error RuntimeState", runtimeState({ lifecycle: "error", readiness: "not_ready" }), states("error")],
  ] as const)("maps %s to M5 component states", async (_label, status, expected) => {
    tauri.invoke.mockResolvedValue(status);
    const client = new TauriHarnessControlClient();

    await expect(client.runtimeStatus()).resolves.toEqual(status);
    expect(tauri.invoke).toHaveBeenCalledWith("runtime_status", undefined);
    expect(client.getComponentStates()).toEqual(expected);
    expect(client.getComponentStates().mcp).not.toBe(
      status.lifecycle === "ready" && status.readiness === "ready" ? "stopped" : "ready",
    );
  });

  it("keeps MCP starting and stopping with the Runtime command lifecycle", async () => {
    let resolveStart: (state: RuntimeState) => void;
    let resolveStop: (result: { accepted: boolean }) => void;
    tauri.invoke
      .mockImplementationOnce(
        () => new Promise<RuntimeState>((resolve) => { resolveStart = resolve; }),
      )
      .mockImplementationOnce(
        () => new Promise<{ accepted: boolean }>((resolve) => { resolveStop = resolve; }),
      );
    const client = new TauriHarnessControlClient();

    const start = client.runtimeStart();
    expect(client.getComponentStates()).toEqual(states("starting"));
    resolveStart!(runtimeState());
    await start;

    const stop = client.runtimeStop();
    expect(client.getComponentStates()).toEqual(states("stopping"));
    resolveStop!({ accepted: true });
    await stop;
    expect(client.getComponentStates()).toEqual(states("stopped"));
  });

  it("reads runtime process status from the Tauri-local command", async () => {
    tauri.invoke.mockResolvedValue({ phase: "running", last_error: null });
    const client = new TauriHarnessControlClient();

    await expect(client.runtimeProcessStatus()).resolves.toEqual({
      phase: "running",
      last_error: null,
    });
    expect(tauri.invoke).toHaveBeenCalledWith("runtime_process_status", undefined);
  });

  it.each([
    ["starting", states("starting")],
    ["stopping", states("stopping")],
    ["stopped", states("stopped")],
    ["crashed", states("error")],
  ] as const)("maps process phase %s without reporting MCP ready", async (phase, expected) => {
    let handler: ((event: { payload: RuntimeProcessEvent }) => void) | undefined;
    tauri.listen.mockImplementation(async (name, listener) => {
      expect(name).toBe("runtime:process");
      handler = listener;
      return vi.fn();
    });
    const client = new TauriHarnessControlClient();
    const listener = vi.fn();

    await client.componentStatesSubscribe(listener);
    const process = {
      phase,
      error:
        phase === "crashed"
          ? { code: "RUNTIME_CRASHED", message: "Runtime process exited unexpectedly." }
          : undefined,
    } as RuntimeProcessEvent;
    handler?.({ payload: process });

    expect(client.getComponentStates()).toEqual(expected);
    expect(listener).toHaveBeenLastCalledWith(expected, undefined, process);
  });

  it("refreshes RuntimeState after a running process event before reporting MCP ready", async () => {
    let handler: ((event: { payload: RuntimeProcessEvent }) => void) | undefined;
    tauri.listen.mockImplementation(async (_name, listener) => {
      handler = listener;
      return vi.fn();
    });
    tauri.invoke.mockResolvedValue(runtimeState());
    const client = new TauriHarnessControlClient();
    const listener = vi.fn();
    const process = { phase: "running" } as RuntimeProcessEvent;

    await client.componentStatesSubscribe(listener);
    handler?.({ payload: process });

    expect(listener).toHaveBeenLastCalledWith(states("starting"), undefined, process);
    expect(client.getComponentStates()).toEqual(states("starting"));

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(tauri.invoke).toHaveBeenCalledWith("runtime_status", undefined);
    expect(client.getComponentStates()).toEqual(states("ready"));
    expect(listener).toHaveBeenLastCalledWith(states("ready"), runtimeState());
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
