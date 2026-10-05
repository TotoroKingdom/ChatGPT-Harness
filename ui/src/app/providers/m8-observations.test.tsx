import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HarnessStatus, RuntimeProcessEvent } from "@/services/harness/client";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import type { RuntimeState } from "@protocol/v1";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));
const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: tauri.listen }));
vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => state.client,
}));

import { TauriHarnessControlClient } from "@/services/harness/local-client";
import { HarnessDataProvider, useHarnessData } from "./HarnessDataProvider";

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

function harnessState(overrides: Partial<HarnessStatus> = {}): HarnessStatus {
  return {
    lifecycle: "ready",
    runtime: "ready",
    tunnel: "ready",
    mcp: "ready",
    activeSessionId: "session-1",
    configRevision: "7",
    lastError: null,
    ...overrides,
  };
}

describe("M8 observation ordering", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    tauri.listen.mockReset();
    state.client = new MockHarnessControlClient("ready");
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("keeps a confirmed Tunnel-ready state when runtimeStatus refreshes Runtime", async () => {
    const harness = harnessState();
    const runtime = runtimeState();
    tauri.listen.mockResolvedValue(vi.fn());
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === "harness_status") return harness;
      if (command === "runtime_status") return runtime;
      throw new Error(`Unexpected command: ${command}`);
    });
    const client = new TauriHarnessControlClient();

    await expect(client.harnessStatus()).resolves.toEqual(harness);
    expect(client.getComponentStates()).toEqual({ runtime: "ready", tunnel: "ready", mcp: "ready" });

    await expect(client.runtimeStatus()).resolves.toEqual(runtime);
    expect(client.getComponentStates()).toEqual({ runtime: "ready", tunnel: "ready", mcp: "ready" });
  });

  it("does not let a delayed harness_status reply revive a newer stopped event", async () => {
    let resolveHarness!: (value: HarnessStatus) => void;
    let harnessListener: ((event: { payload: HarnessStatus }) => void) | undefined;
    const ready = harnessState();
    const stopped = harnessState({
      lifecycle: "stopped",
      runtime: "stopped",
      tunnel: "stopped",
      mcp: "stopped",
      activeSessionId: null,
      configRevision: null,
    });
    tauri.listen.mockImplementation(async (name, listener) => {
      expect(name).toBe("harness:status");
      harnessListener = listener;
      return vi.fn();
    });
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "harness_status") return new Promise<HarnessStatus>((resolve) => { resolveHarness = resolve; });
      throw new Error(`Unexpected command: ${command}`);
    });
    const client = new TauriHarnessControlClient();
    const pending = client.harnessStatus();

    await waitFor(() => expect(harnessListener).toBeDefined());
    act(() => harnessListener?.({ payload: stopped }));
    expect(client.getHarnessStatus()).toEqual(stopped);
    expect(client.getComponentStates()).toEqual({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });

    resolveHarness(ready);
    await expect(pending).resolves.toEqual(stopped);
    expect(client.getHarnessStatus()).toEqual(stopped);
    expect(client.getComponentStates()).toEqual({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });
  });

  function ProviderProbe() {
    const data = useHarnessData();
    return (
      <>
        <output data-testid="harness-lifecycle">{data.harness.lifecycle}</output>
        <output data-testid="harness-session">{data.harness.activeSessionId ?? ""}</output>
        <output data-testid="runtime-component">{data.components.runtime}</output>
        <output data-testid="runtime-resource">{data.runtime.status}</output>
        <output data-testid="tools-resource">{data.tools.status}</output>
      </>
    );
  }

  it("does not apply delayed initial resources after a newer stopped observation", async () => {
    const initialHarness = state.client.getHarnessStatus();
    vi.spyOn(state.client, "harnessStatus").mockResolvedValue(initialHarness);
    vi.spyOn(state.client, "runtimeProcessStatus").mockResolvedValue({ phase: "running", last_error: null });
    const runtime = runtimeState();
    let resolveResources!: () => void;
    const resources = new Promise<void>((resolve) => { resolveResources = resolve; });
    vi.spyOn(state.client, "runtimeStatus").mockImplementation(async () => { await resources; return runtime; });
    vi.spyOn(state.client, "toolsList").mockImplementation(async () => { await resources; return { items: [] }; });
    vi.spyOn(state.client, "policiesList").mockImplementation(async () => { await resources; return { items: [] }; });
    vi.spyOn(state.client, "activityList").mockImplementation(async () => { await resources; return { items: [], latestSequence: "0" }; });
    vi.spyOn(state.client, "approvalsList").mockImplementation(async () => { await resources; return { items: [] }; });
    let componentListener: ((states: ReturnType<MockHarnessControlClient["getComponentStates"]>, runtime?: RuntimeState, process?: RuntimeProcessEvent) => void) | undefined;
    vi.spyOn(state.client, "componentStatesSubscribe").mockImplementation(async (listener) => {
      componentListener = listener;
      return () => undefined;
    });

    render(
      <HarnessDataProvider>
        <ProviderProbe />
      </HarnessDataProvider>,
    );
    await waitFor(() => expect(componentListener).toBeDefined());
    await waitFor(() => expect(screen.getByTestId("harness-lifecycle")).toHaveTextContent("ready"));

    state.client.setComponentStates({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });
    act(() => componentListener?.(
      state.client.getComponentStates(),
      undefined,
      { phase: "stopped" },
    ));
    await waitFor(() => {
      expect(screen.getByTestId("harness-lifecycle")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("idle");
    });

    resolveResources();
    await waitFor(() => {
      expect(screen.getByTestId("harness-lifecycle")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("idle");
      expect(screen.getByTestId("tools-resource")).toHaveTextContent("loading");
    });
  });
});
