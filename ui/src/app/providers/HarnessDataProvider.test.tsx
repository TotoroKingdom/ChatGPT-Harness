import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ActivityEvent, ApprovalsListResult, DiagnosticRun, RuntimeState, StartupAttempt } from "@protocol/v1";
import type { ActivityEventListener, ComponentStatesListener } from "@/services/harness/client";
import { HarnessClientError } from "@/services/harness/errors";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

let client: MockHarnessControlClient;
let activityListener: ActivityEventListener | undefined;
let unsubscribe: ReturnType<typeof vi.fn>;
let observedActivityCursor: string | undefined;
let componentStatesListener: ComponentStatesListener | undefined;

vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => client,
}));

import { HarnessDataProvider, useHarnessData } from "./HarnessDataProvider";
import { useActivitySubscription } from "./useActivitySubscription";

function activityEvent(): ActivityEvent {
  return {
    eventId: "event-1",
    runtimeId: "runtime-1",
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

function ApprovalsProbe() {
  const { approvals, refreshApprovals, runtime } = useHarnessData();
  const count = approvals.status === "success" || approvals.status === "empty"
    ? approvals.data.items.filter((item) => item.status === "pending").length
    : -1;
  return <>
    <output data-testid="approval-status">{approvals.status}</output>
    <output data-testid="approval-count">{count}</output>
    <output data-testid="approval-runtime">{runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : runtime.status}</output>
    <button type="button" onClick={() => void refreshApprovals()}>Refresh approvals</button>
  </>;
}

function DiagnosticsProbe() {
  const { lastDiagnostics, latestStartupAttempt, runDiagnostics } = useHarnessData();
  const [result, setResult] = useState("");
  return <>
    <output data-testid="diagnostics-present">{lastDiagnostics ? "present" : "none"}</output>
    <output data-testid="startup-present">{latestStartupAttempt ? "present" : "none"}</output>
    <output data-testid="diagnostics-result">{result}</output>
    <button type="button" onClick={() => {
      void runDiagnostics()
        .then(() => setResult("ok"))
        .catch((error) => setResult(String((error as { code?: string })?.code ?? "failed")));
    }}>Run diagnostics</button>
  </>;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function approvalEvent(overrides: Partial<ActivityEvent> = {}): ActivityEvent {
  return {
    eventId: "approval-event-1",
    runtimeId: "runtime_mock_01",
    sequence: "4",
    eventType: "approval_requested",
    timestamp: "2026-09-02T00:00:00Z",
    summary: "Approval requested.",
    metadata: {},
    ...overrides,
  };
}

function approvalItem(requestId: string) {
  return {
    requestId,
    runtimeId: "runtime_mock_01",
    generationId: "generation-mock",
    tool: "write" as const,
    sanitizedArguments: { tool: "write" as const, logicalPath: `${requestId}.txt` },
    scopeSummary: `${requestId}.txt`,
    matchedRuleIds: [],
    policyReason: "tool_default" as const,
    riskLevel: "low" as const,
    createdAt: "2026-09-02T00:00:00Z",
    expiresAt: "2026-09-02T01:00:00Z",
    status: "pending" as const,
  };
}

function LanguageProbe() {
  const { setUiLanguage } = useHarnessData();
  const [result, setResult] = useState("");

  return (
    <>
      <output data-testid="language-save-result">{result}</output>
      <button
        type="button"
        onClick={() => {
          void setUiLanguage("zh-CN").then((next) => setResult(String(next.ok)));
        }}
      >
        Save language
      </button>
    </>
  );
}

function ThemeProbe() {
  const { themePreference, effectiveTheme, restartRequired } = useHarnessData();
  return <><output data-testid="theme-preference">{themePreference}</output><output data-testid="effective-theme">{effectiveTheme}</output><output data-testid="theme-restart-required">{String(restartRequired)}</output></>;
}

function RuntimeProbe() {
  const { components, restartRequired, runtime } = useHarnessData();
  const runtimeId =
    runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : "";
  const runtimeErrorCode = runtime.status === "error" ? runtime.error.code ?? "" : "";
  const runtimeErrorMessage = runtime.status === "error" ? runtime.error.message : "";

  return (
    <>
      <output data-testid="runtime-component">{components.runtime}</output>
      <output data-testid="mcp-component">{components.mcp}</output>
      <output data-testid="runtime-resource">{runtime.status}</output>
      <output data-testid="runtime-id">{runtimeId}</output>
      <output data-testid="runtime-error-code">{runtimeErrorCode}</output>
      <output data-testid="runtime-error-message">{runtimeErrorMessage}</output>
      <output data-testid="restart-required">{String(restartRequired)}</output>
    </>
  );
}

function RestartProbe() {
  const { restartHarness, restartRequired } = useHarnessData();
  const [result, setResult] = useState("");
  return <>
    <output data-testid="restart-result">{result}</output>
    <output data-testid="restart-required">{String(restartRequired)}</output>
    <button onClick={() => void restartHarness().then((next) => setResult(next.ok ? "ok" : next.error?.code ?? "error"))}>Restart</button>
  </>;
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

  it("persists language through the isolated client mutation", async () => {
    const languageSet = vi.spyOn(client, "uiLanguageSet");
    const fullConfigSave = vi.spyOn(client, "configSave");

    render(
      <HarnessDataProvider>
        <LanguageProbe />
      </HarnessDataProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Save language" }));
    await waitFor(() => expect(screen.getByTestId("language-save-result")).toHaveTextContent("true"));
    expect(languageSet).toHaveBeenCalledWith("zh-CN");
    expect(fullConfigSave).not.toHaveBeenCalled();
    await expect(client.configGet()).resolves.toEqual(expect.objectContaining({ ui: { language: "zh-CN" } }));
  });

  it("applies persisted System theme and follows OS changes without Runtime restart state", async () => {
    const persisted = await client.configGet();
    if (!persisted) throw new Error("mock config missing");
    await client.configSave({ ...persisted, ui: { ...persisted.ui, theme: "system" } });

    const originalMatchMedia = window.matchMedia;
    let listener: ((event: MediaQueryListEvent) => void) | undefined;
    const media = {
      matches: false,
      addEventListener: vi.fn((_type: string, next: (event: MediaQueryListEvent) => void) => { listener = next; }),
      removeEventListener: vi.fn(),
    } as unknown as MediaQueryList;
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: vi.fn().mockReturnValue(media) });

    render(<HarnessDataProvider><ThemeProbe /></HarnessDataProvider>);
    await waitFor(() => {
      expect(screen.getByTestId("theme-preference")).toHaveTextContent("system");
      expect(screen.getByTestId("effective-theme")).toHaveTextContent("light");
      expect(document.documentElement.dataset.theme).toBe("light");
    });

    act(() => listener?.({ matches: true } as MediaQueryListEvent));
    await waitFor(() => {
      expect(screen.getByTestId("effective-theme")).toHaveTextContent("dark");
      expect(document.documentElement.dataset.theme).toBe("dark");
    });
    expect(screen.getByTestId("theme-restart-required")).toHaveTextContent("false");

    if (originalMatchMedia) Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: originalMatchMedia });
    else Reflect.deleteProperty(window, "matchMedia");
  });

  it("reconciles the pending count from Runtime after an approval activity event", async () => {
    render(<HarnessDataProvider><Probe /><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => {
      expect(screen.getByTestId("subscription-state")).toHaveTextContent("subscribed");
      expect(screen.getByTestId("approval-runtime")).toHaveTextContent("runtime_mock_01");
      expect(screen.getByTestId("approval-count")).toHaveTextContent(/^0$/);
    });
    const refresh = vi.spyOn(client, "approvalsList").mockResolvedValue({ items: [approvalItem("approval-new")] });
    act(() => activityListener?.(approvalEvent()));
    await waitFor(() => expect(refresh).toHaveBeenCalled(), { timeout: 1500 });
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^1$/));
  });

  it("ignores approval signals from a previous Runtime identity", async () => {
    render(<HarnessDataProvider><Probe /><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => {
      expect(screen.getByTestId("approval-runtime")).toHaveTextContent("runtime_mock_01");
      expect(screen.getByTestId("approval-count")).toHaveTextContent(/^0$/);
    });
    const refresh = vi.spyOn(client, "approvalsList");
    act(() => activityListener?.(approvalEvent({ eventId: "old-runtime", runtimeId: "runtime_old", sequence: "9" })));
    await new Promise((resolve) => window.setTimeout(resolve, 500));
    expect(refresh).not.toHaveBeenCalled();
  });

  it("keeps the newest authoritative approval read when responses arrive out of order", async () => {
    render(<HarnessDataProvider><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^0$/));
    const first = deferred<ApprovalsListResult>();
    const second = deferred<ApprovalsListResult>();
    const refresh = vi.spyOn(client, "approvalsList")
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    fireEvent.click(screen.getByRole("button", { name: "Refresh approvals" }));
    fireEvent.click(screen.getByRole("button", { name: "Refresh approvals" }));
    expect(refresh).toHaveBeenCalledTimes(2);
    await act(async () => second.resolve({
      items: [
        approvalItem("approval-a"),
        approvalItem("approval-b"),
      ],
    }));
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^2$/));
    await act(async () => first.resolve({ items: Array.from({ length: 5 }, (_, index) => approvalItem(`stale-${index}`)) }));
    expect(screen.getByTestId("approval-count")).toHaveTextContent(/^2$/);
  });

  it("removes the pending count when an authoritative approval read fails", async () => {
    render(<HarnessDataProvider><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^0$/));
    vi.spyOn(client, "approvalsList").mockRejectedValueOnce({ code: "RUNTIME_UNAVAILABLE" });
    fireEvent.click(screen.getByRole("button", { name: "Refresh approvals" }));
    await waitFor(() => expect(screen.getByTestId("approval-status")).toHaveTextContent("error"));
    expect(screen.getByTestId("approval-count")).toHaveTextContent(/^-1$/);
  });

  it("reconciles approvals on the low-frequency fallback even without an activity event", async () => {
    const setInterval = vi.spyOn(window, "setInterval");
    render(<HarnessDataProvider><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => expect(screen.getByTestId("approval-runtime")).toHaveTextContent("runtime_mock_01"));
    const call = setInterval.mock.calls.find(([, delay]) => delay === 30_000);
    expect(call).toBeDefined();
    const refresh = vi.spyOn(client, "approvalsList").mockResolvedValue({ items: [approvalItem("fallback")] });
    act(() => (call?.[0] as () => void)());
    await waitFor(() => expect(refresh).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^1$/));
  });

  it("invalidates on disconnect and reconciles immediately after reconnect", async () => {
    let listener: ComponentStatesListener | undefined;
    vi.spyOn(client, "componentStatesSubscribe").mockImplementation(async (next) => {
      listener = next;
      return () => undefined;
    });
    render(<HarnessDataProvider><ApprovalsProbe /></HarnessDataProvider>);
    await waitFor(() => {
      expect(screen.getByTestId("approval-runtime")).toHaveTextContent("runtime_mock_01");
      expect(screen.getByTestId("approval-count")).toHaveTextContent(/^0$/);
    });

    act(() => listener?.(
      { runtime: "error", tunnel: "stopped", mcp: "error" },
      undefined,
      { phase: "crashed", error: { code: "RUNTIME_CRASHED", message: "Runtime crashed." } },
    ));
    await waitFor(() => expect(screen.getByTestId("approval-status")).toHaveTextContent("idle"));

    const refresh = vi.spyOn(client, "approvalsList").mockResolvedValue({ items: [approvalItem("after-reconnect")] });
    act(() => listener?.(
      { runtime: "ready", tunnel: "ready", mcp: "ready" },
      runtimeState({ runtimeId: "runtime-next" }),
      { phase: "running" },
    ));
    await waitFor(() => expect(refresh).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByTestId("approval-count")).toHaveTextContent(/^1$/));
    expect(screen.getByTestId("approval-runtime")).toHaveTextContent("runtime-next");
  });

  it("keeps a successful diagnostics result when the startup evidence read fails", async () => {
    const run: DiagnosticRun = { runId: "run-1", startedAt: "2026-09-02T00:00:00Z", finishedAt: "2026-09-02T00:00:01Z", overallStatus: "pass", checks: [] };
    vi.spyOn(client, "connectionDiagnosticsRun").mockResolvedValue(run);
    vi.spyOn(client, "startupLatest").mockRejectedValue({ code: "RUNTIME_UNAVAILABLE" });
    render(<HarnessDataProvider><DiagnosticsProbe /></HarnessDataProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Run diagnostics" }));
    await waitFor(() => expect(screen.getByTestId("diagnostics-result")).toHaveTextContent("RUNTIME_UNAVAILABLE"));
    expect(screen.getByTestId("diagnostics-present")).toHaveTextContent("present");
    expect(screen.getByTestId("startup-present")).toHaveTextContent("none");
  });

  it("keeps successful startup evidence when diagnostics fails", async () => {
    const startup: StartupAttempt = { attemptId: "attempt-1", harnessSessionId: "session-1", cohort: "cold", startedAt: "2026-09-02T00:00:00Z", outcome: "ready", phases: [], appVersion: "0.1.3" };
    vi.spyOn(client, "connectionDiagnosticsRun").mockRejectedValue({ code: "RUNTIME_UNAVAILABLE" });
    vi.spyOn(client, "startupLatest").mockResolvedValue(startup);
    render(<HarnessDataProvider><DiagnosticsProbe /></HarnessDataProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Run diagnostics" }));
    await waitFor(() => expect(screen.getByTestId("diagnostics-result")).toHaveTextContent("RUNTIME_UNAVAILABLE"));
    expect(screen.getByTestId("diagnostics-present")).toHaveTextContent("none");
    expect(screen.getByTestId("startup-present")).toHaveTextContent("present");
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
    componentStatesListener = undefined;
    client = new MockHarnessControlClient("empty");
    vi.spyOn(client, "activityList").mockResolvedValue({ items: [], latestSequence: "0" });
    vi.spyOn(client, "componentStatesSubscribe").mockImplementation(async (listener) => {
      componentStatesListener = listener;
      return () => {};
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("uses the serialized Host restart and permits retry after a restart error", async () => {
    const config = await client.configGet();
    vi.spyOn(client, "configGet").mockResolvedValue(config ? { ...config, configRevision: "8" } : config);
    vi.spyOn(client, "runtimeStatus").mockResolvedValueOnce(runtimeState()).mockResolvedValue(runtimeState({ configRevision: "8" }));
    vi.spyOn(client, "runtimeProcessStatus").mockResolvedValue({ phase: "running", last_error: null });
    const ready = { ...client.getHarnessStatus(), lifecycle: "ready" as const, configRevision: "8" };
    const restart = vi.spyOn(client, "harnessRestart")
      .mockRejectedValueOnce(new HarnessClientError("error", "Shutdown failed; retry Stop or Restart.", undefined, "RUNTIME_TIMEOUT"))
      .mockResolvedValue(ready);
    const stop = vi.spyOn(client, "harnessStop");
    const start = vi.spyOn(client, "harnessStart");
    render(<HarnessDataProvider><RestartProbe /></HarnessDataProvider>);
    await waitFor(() => expect(screen.getByTestId("restart-required")).toHaveTextContent("true"));
    fireEvent.click(screen.getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(screen.getByTestId("restart-result")).toHaveTextContent("RUNTIME_TIMEOUT"));
    fireEvent.click(screen.getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(screen.getByTestId("restart-result")).toHaveTextContent("ok"));
    expect(screen.getByTestId("restart-required")).toHaveTextContent("false");
    expect(restart).toHaveBeenCalledTimes(2);
    expect(stop).not.toHaveBeenCalled();
    expect(start).not.toHaveBeenCalled();
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
      expect(screen.getByTestId("mcp-component")).toHaveTextContent("ready");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("success");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("runtime-1");
    });
    expect(status).toHaveBeenCalledOnce();
    expect(screen.getByTestId("runtime-component")).not.toHaveTextContent("stopped");
    expect(screen.getByTestId("restart-required")).toHaveTextContent("true");
  });

  it("clears the active RuntimeState and preserves the crash error", async () => {
    vi.spyOn(client, "runtimeStatus").mockResolvedValue(
      runtimeState({ runtimeId: "runtime-A", configRevision: "7" }),
    );
    vi.spyOn(client, "runtimeProcessStatus").mockResolvedValue({
      phase: "running",
      last_error: null,
    });

    render(
      <HarnessDataProvider>
        <RuntimeProbe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("ready");
      expect(screen.getByTestId("mcp-component")).toHaveTextContent("ready");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("success");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("runtime-A");
    });

    act(() =>
      componentStatesListener?.(
        { runtime: "error", tunnel: "stopped", mcp: "stopped" },
        undefined,
        {
          phase: "crashed",
          error: {
            code: "RUNTIME_CRASHED",
            message: "Runtime process exited unexpectedly.",
          },
        },
      ),
    );

    await waitFor(() => {
      expect(screen.getByTestId("runtime-component")).toHaveTextContent("error");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("error");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("");
      expect(screen.getByTestId("runtime-error-code")).toHaveTextContent("RUNTIME_CRASHED");
      expect(screen.getByTestId("runtime-error-message")).toHaveTextContent(
        "Runtime process exited unexpectedly.",
      );
      expect(screen.getByTestId("restart-required")).toHaveTextContent("false");
    });
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
      expect(screen.getByTestId("mcp-component")).toHaveTextContent("stopped");
      expect(screen.getByTestId("runtime-resource")).toHaveTextContent("idle");
      expect(screen.getByTestId("runtime-id")).toHaveTextContent("");
      expect(screen.getByTestId("restart-required")).toHaveTextContent("false");
    });
    expect(status).not.toHaveBeenCalled();
  });
});
