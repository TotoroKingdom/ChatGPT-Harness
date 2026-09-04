import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ActivityEvent, RuntimeState } from "@protocol/v1";
import type { ActivityEventListener } from "@/services/harness/client";
import { HarnessClientError } from "@/services/harness/errors";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

let client: MockHarnessControlClient;
let activityListener: ActivityEventListener | undefined;
let unsubscribe: ReturnType<typeof vi.fn>;
let observedActivityCursor: string | undefined;

vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => client,
}));

import { HarnessDataProvider, useHarnessData } from "./HarnessDataProvider";
import { useActivitySubscription } from "./useActivitySubscription";

function activityEvent(): ActivityEvent {
  return {
    eventId: "event-1",
    sequence: "1",
    eventType: "runtime_started",
    timestamp: "2026-09-02T00:00:00Z",
    summary: "Runtime started.",
    metadata: {},
  };
}

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

function Probe() {
  const { activity } = useHarnessData();
  const subscriptionState = useActivitySubscription();
  const itemCount =
    activity.status === "success" || activity.status === "empty" ? activity.data.items.length : -1;

  return (
    <>
      <output data-testid="activity-count">{itemCount}</output>
      <output data-testid="subscription-state">{subscriptionState}</output>
    </>
  );
}

function RuntimeProbe() {
  const { components, restartRequired, runtime } = useHarnessData();
  const runtimeId =
    runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : "";

  return (
    <>
      <output data-testid="runtime-component">{components.runtime}</output>
      <output data-testid="runtime-resource">{runtime.status}</output>
      <output data-testid="runtime-id">{runtimeId}</output>
      <output data-testid="restart-required">{String(restartRequired)}</output>
    </>
  );
}

describe("HarnessDataProvider activity subscription", () => {
  beforeEach(() => {
    activityListener = undefined;
    unsubscribe = vi.fn();
    client = new MockHarnessControlClient("empty");
    vi.spyOn(client, "activityList").mockResolvedValue({ items: [], latestSequence: "0" });
    vi.spyOn(client, "activitySubscribe").mockImplementation(async (listener) => {
      activityListener = listener;
      return unsubscribe;
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  async function renderSubscribedProvider() {
    render(
      <HarnessDataProvider>
        <Probe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("activity-count")).toHaveTextContent("0");
      expect(screen.getByTestId("subscription-state")).toHaveTextContent("subscribed");
    });
    expect(activityListener).toBeDefined();
  }

  it("activity_listener_does_not_resubscribe_on_append", async () => {
    await renderSubscribedProvider();

    act(() => activityListener?.(activityEvent()));

    await waitFor(() => expect(screen.getByTestId("activity-count")).toHaveTextContent("1"));
    expect(client.activitySubscribe).toHaveBeenCalledOnce();
  });

  it("duplicate_event_id_is_not_appended_twice", async () => {
    await renderSubscribedProvider();
    const event = activityEvent();

    act(() => activityListener?.(event));
    act(() => activityListener?.(event));

    await waitFor(() => expect(screen.getByTestId("activity-count")).toHaveTextContent("1"));
    expect(client.activitySubscribe).toHaveBeenCalledOnce();
  });
});

function StartProbe() {
  const { activity, policies, startHarness, subscribeToActivity, tools } = useHarnessData();
  const [startResult, setStartResult] = useState("");

  return (
    <>
      <output data-testid="tools-status">{tools.status}</output>
      <output data-testid="policies-status">{policies.status}</output>
      <output data-testid="activity-status">{activity.status}</output>
      <output data-testid="activity-sequence">
        {activity.status === "success" || activity.status === "empty" ? activity.data.latestSequence : ""}
      </output>
      <output data-testid="start-result">{startResult}</output>
      <button
        type="button"
        onClick={() => {
          void startHarness().then((result) => setStartResult(String(result.ok)));
        }}
      >
        Start
      </button>
      <button type="button" onClick={() => void subscribeToActivity(() => {})}>
        Subscribe
      </button>
    </>
  );
}

describe("HarnessDataProvider runtime rehydration", () => {
  beforeEach(() => {
    client = new MockHarnessControlClient("empty");
    vi.spyOn(client, "activityList").mockResolvedValue({ items: [], latestSequence: "0" });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("refreshes Runtime-owned resources after Start succeeds", async () => {
    observedActivityCursor = undefined;
    client.setComponentStates({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });
    vi.spyOn(client, "toolsList")
      .mockRejectedValueOnce(new HarnessClientError("disconnected", "Runtime disconnected"))
      .mockResolvedValue({ items: [] });
    vi.spyOn(client, "policiesList")
      .mockRejectedValueOnce(new HarnessClientError("disconnected", "Runtime disconnected"))
      .mockResolvedValue({ items: [] });
    vi.spyOn(client, "activityList")
      .mockResolvedValueOnce({ items: [], latestSequence: "0" })
      .mockResolvedValue({ items: [activityEvent()], latestSequence: "42" });
    vi.spyOn(client, "runtimeStatus").mockResolvedValue(runtimeState());
    vi.spyOn(client, "activitySubscribe").mockImplementation(async (_listener, afterSequence) => {
      observedActivityCursor = afterSequence;
      return () => {};
    });

    render(
      <HarnessDataProvider>
        <StartProbe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("tools-status")).toHaveTextContent("disconnected");
      expect(screen.getByTestId("policies-status")).toHaveTextContent("disconnected");
    });

    fireEvent.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => {
      expect(screen.getByTestId("start-result")).toHaveTextContent("true");
      expect(screen.getByTestId("tools-status")).toHaveTextContent("empty");
      expect(screen.getByTestId("policies-status")).toHaveTextContent("empty");
      expect(screen.getByTestId("activity-status")).toHaveTextContent("success");
      expect(screen.getByTestId("activity-sequence")).toHaveTextContent("42");
    });

    fireEvent.click(screen.getByRole("button", { name: "Subscribe" }));
    await waitFor(() => expect(observedActivityCursor).toBe("42"));
  });

  it("restores a ready RuntimeState when the process is already running", async () => {
    const status = vi.spyOn(client, "runtimeStatus").mockResolvedValue(runtimeState());
    vi.spyOn(client, "runtimeProcessStatus").mockResolvedValue({
      phase: "running",
      last_error: null,
    });
    const config = await client.configGet();
    vi.spyOn(client, "configGet").mockResolvedValue(
      config ? { ...config, configRevision: "8" } : config,
    );

    render(
      <HarnessDataProvider>
        <RuntimeProbe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("ready");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("success");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("runtime-1");
    });
    expect(status).toHaveBeenCalledOnce();
    expect(screen.getByTestId("runtime-component")).not.toHaveTextContent("stopped");
    expect(screen.getByTestId("restart-required")).toHaveTextContent("true");
  });

  it("keeps RuntimeState empty when the process is stopped", async () => {
    const status = vi.spyOn(client, "runtimeStatus");
    vi.spyOn(client, "runtimeProcessStatus").mockResolvedValue({
      phase: "stopped",
      last_error: null,
    });
    const config = await client.configGet();
    vi.spyOn(client, "configGet").mockResolvedValue(
      config ? { ...config, configRevision: "8" } : config,
    );

    render(
      <HarnessDataProvider>
        <RuntimeProbe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("idle");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("");
      expect(screen.getByTestId("restart-required")).toHaveTextContent("false");
    });
    expect(status).not.toHaveBeenCalled();
  });
});