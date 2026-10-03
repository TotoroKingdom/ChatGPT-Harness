import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  ActivityEvent,
  ActivityListResult,
  AppConfig,
  ApprovalsListResult,
  DiagnosticRun,
  PoliciesListResult,
  RuntimeState,
  StartEligibility,
  ToolsListResult,
  UiLanguage,
} from "@protocol/v1";
import { HarnessClientError, toClientError } from "@/services/harness/errors";
import { createHarnessControlClient } from "@/services/harness/factory";
import type {
  ActivityEventListener,
  ComponentStates,
  ConfigMetadata,
  HarnessControlClient,
  HarnessStatus,
  OverallStatus,
  Unsubscribe,
} from "@/services/harness/client";
import { overallStatus } from "@/services/harness/client";

export type AsyncState<T> =
  | { status: "idle" | "loading" }
  | { status: "success" | "empty"; data: T }
  | { status: "error" | "disconnected"; error: HarnessClientError };
type ResourceState = {
  runtime: AsyncState<RuntimeState>;
  tools: AsyncState<ToolsListResult>;
  policies: AsyncState<PoliciesListResult>;
  activity: AsyncState<ActivityListResult>;
  approvals: AsyncState<ApprovalsListResult>;
};
export type SaveResult = { ok: boolean; error?: HarnessClientError };

type ContextValue = ResourceState & {
  client: HarnessControlClient;
  config: AppConfig | null;
  configMetadata: ConfigMetadata | null;
  configError: HarnessClientError | null;
  eligibility: StartEligibility | null;
  setupRequired: boolean;
  apiKeyConfigured: boolean;
  components: ComponentStates;
  harness: HarnessStatus;
  overall: OverallStatus;
  restartRequired: boolean;
  lastDiagnostics: DiagnosticRun | null;
  refreshConfig: () => Promise<void>;
  resetConfig: () => Promise<SaveResult>;
  saveConfig: (config: AppConfig, apiKey?: string) => Promise<SaveResult>;
  setUiLanguage: (language: UiLanguage | null) => Promise<SaveResult>;
  deleteApiKey: () => Promise<void>;
  chooseWorkspace: () => Promise<string | null>;
  chooseTunnelClient: () => Promise<string | null>;
  detectTunnelClient: () => Promise<string | null>;
  runDiagnostics: () => Promise<DiagnosticRun>;
  startHarness: () => Promise<SaveResult>;
  stopHarness: () => Promise<SaveResult>;
  restartHarness: () => Promise<SaveResult>;
  appendActivity: (event: ActivityEvent) => void;
  subscribeToActivity: (listener: ActivityEventListener) => Promise<Unsubscribe>;
  refreshApprovals: () => Promise<void>;
};

const initial: ResourceState = {
  runtime: { status: "idle" },
  tools: { status: "idle" },
  policies: { status: "idle" },
  activity: { status: "idle" },
  approvals: { status: "idle" },
};
const Context = createContext<ContextValue | null>(null);

function loaded<T>(data: T, empty: boolean): AsyncState<T> {
  return empty ? { status: "empty", data } : { status: "success", data };
}
function isNewerSequence(current: string | undefined, candidate: string): boolean {
  if (current === undefined || candidate.length !== current.length) {
    return current === undefined || candidate.length > current.length;
  }
  return candidate > current;
}
async function load<T>(fn: () => Promise<T>, empty: (data: T) => boolean): Promise<AsyncState<T>> {
  try {
    const data = await fn();
    return loaded(data, empty(data));
  } catch (error) {
    const clientError = toClientError(error);
    return { status: clientError.kind, error: clientError };
  }
}

function componentStatesFromRuntime(
  state: RuntimeState,
  current: ComponentStates,
): ComponentStates {
  let runtime: ComponentStates["runtime"];
  if (state.lifecycle === "error") runtime = "error";
  else if (!state.runtimeId) runtime = "stopped";
  else if (state.lifecycle === "stopping") runtime = "stopping";
  else if (state.lifecycle === "ready" && state.readiness === "ready") runtime = "ready";
  else if (state.readiness === "degraded") runtime = "degraded";
  else runtime = "starting";

  // M5 MCP is an endpoint inside Runtime; RuntimeState is authoritative for it.
  // Keep Mock-only Tunnel lifecycle behavior intact.
  return { ...current, runtime, mcp: runtime };
}

function componentStatesWithRuntimeStatus(
  current: ComponentStates,
  runtime: ComponentStates["runtime"],
): ComponentStates {
  return { ...current, runtime, mcp: runtime };
}

type RuntimeRehydration = {
  resource: AsyncState<RuntimeState>;
  components: ComponentStates;
  activeRevision: string | null;
};

async function rehydrateRuntime(client: HarnessControlClient): Promise<RuntimeRehydration> {
  const current = client.getComponentStates();
  try {
    const process = await client.runtimeProcessStatus();
    switch (process.phase) {
      case "running": {
        const resource = await load(() => client.runtimeStatus(), () => false);
        if (resource.status === "success" || resource.status === "empty") {
          return {
            resource,
            components: client.getComponentStates(),
            activeRevision: resource.data.configRevision,
          };
        }
        return { resource, components: componentStatesWithRuntimeStatus(current, "error"), activeRevision: null };
      }
      case "stopped":
        return {
          resource: { status: "idle" },
          components: componentStatesWithRuntimeStatus(current, "stopped"),
          activeRevision: null,
        };
      case "crashed":
        return {
          resource: {
            status: "error",
            error: new HarnessClientError(
              "error",
              process.last_error?.message ?? "Runtime crashed.",
              undefined,
              process.last_error?.code,
            ),
          },
          components: componentStatesWithRuntimeStatus(current, "error"),
          activeRevision: null,
        };
      case "starting":
        return {
          resource: { status: "idle" },
          components: componentStatesWithRuntimeStatus(current, "starting"),
          activeRevision: null,
        };
      case "stopping":
        return {
          resource: { status: "idle" },
          components: componentStatesWithRuntimeStatus(current, "stopping"),
          activeRevision: null,
        };
    }
  } catch (error) {
    const clientError = toClientError(error);
    return {
      resource: { status: clientError.kind, error: clientError },
      components: componentStatesWithRuntimeStatus(current, "error"),
      activeRevision: null,
    };
  }
}

export function HarnessDataProvider({ children }: { children: ReactNode }) {
  const client = useMemo(() => createHarnessControlClient(), []);
  const [resources, setResources] = useState<ResourceState>(initial);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [configLoaded, setConfigLoaded] = useState(false);
  const [metadata, setMetadata] = useState<ConfigMetadata | null>(null);
  const [configError, setConfigError] = useState<HarnessClientError | null>(null);
  const [eligibility, setEligibility] = useState<StartEligibility | null>(null);
  const [apiKeyConfigured, setApiKeyConfigured] = useState(false);
  const [components, setComponents] = useState<ComponentStates>(client.getComponentStates());
  const [harness, setHarness] = useState<HarnessStatus>(client.getHarnessStatus());
  const [activeRevision, setActiveRevision] = useState<string | null>(null);
  const [lastDiagnostics, setLastDiagnostics] = useState<DiagnosticRun | null>(null);
  const activityCursorRef = useRef<string | undefined>(undefined);

  const refreshConfig = useCallback(async () => {
    try {
      const [nextConfig, nextMetadata, keyConfigured, nextEligibility] = await Promise.all([
        client.configGet(),
        client.configMetadata(),
        client.apiKeyStatus(),
        client.harnessStartEligibility(),
      ]);
      setConfig(nextConfig);
      setConfigLoaded(true);
      setMetadata(nextMetadata);
      setApiKeyConfigured(keyConfigured);
      setEligibility(nextEligibility);
      setConfigError(null);
    } catch (error) {
      const clientError = toClientError(error);
      if (clientError.code === "CONFIG_INVALID") setConfigLoaded(true);
      setConfigError(clientError);
      setEligibility(null);
    }
  }, [client]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: Unsubscribe | undefined;
    void client.componentStatesSubscribe((nextComponents, runtime, process) => {
      if (!mounted) return;
      setComponents(nextComponents);
      const harnessState = client.getHarnessStatus();
      setHarness(harnessState);
      if (harnessState.lastError && nextComponents.runtime === "error") {
        setResources((current) => ({
          ...current,
          runtime: {
            status: "error",
            error: new HarnessClientError(
              "error",
              harnessState.lastError?.message ?? "Harness Runtime failed.",
              undefined,
              harnessState.lastError?.code,
            ),
          },
        }));
      }
      if (process?.phase === "crashed") {
        setActiveRevision(null);
        setResources((current) => ({
          ...current,
          runtime: {
            status: "error",
            error: new HarnessClientError(
              "error",
              process.error?.message ?? "Runtime crashed.",
              undefined,
              process.error?.code,
            ),
          },
        }));
      } else if (process?.phase === "stopped") {
        setActiveRevision(null);
        setResources((current) => ({ ...current, runtime: { status: "idle" } }));
      } else if (runtime) {
        setActiveRevision(runtime.runtimeId ? runtime.configRevision : null);
        setResources((current) => ({ ...current, runtime: { status: "success", data: runtime } }));
      } else if (nextComponents.runtime === "stopped" || nextComponents.runtime === "error") {
        setActiveRevision(null);
      }
    }).then((cleanup) => {
      if (mounted) unsubscribe = cleanup;
      else cleanup();
    });
    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, [client]);

  useEffect(() => {
    let mounted = true;
    void refreshConfig();
    setResources({
      runtime: { status: "loading" },
      tools: { status: "loading" },
      policies: { status: "loading" },
      activity: { status: "loading" },
      approvals: { status: "loading" },
    });
    void Promise.all([
      load(() => client.harnessStatus(), () => false),
      rehydrateRuntime(client),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
      load(() => client.approvalsList(), (data) => data.items.length === 0),
    ]).then(([harnessResource, runtime, tools, policies, activity, approvals]) => {
      if (!mounted) return;
      if (harnessResource.status === "success" || harnessResource.status === "empty") {
        setHarness(harnessResource.data);
        client.setComponentStates({ runtime: harnessResource.data.runtime, mcp: harnessResource.data.mcp, tunnel: harnessResource.data.tunnel });
      }
      client.setComponentStates(runtime.components);
      setComponents(runtime.components);
      setResources({ runtime: runtime.resource, tools, policies, activity, approvals });
      setActiveRevision(runtime.activeRevision);
      if (activity.status === "success" || activity.status === "empty") {
        activityCursorRef.current = activity.data.latestSequence;
      }
    });
    return () => {
      mounted = false;
    };
  }, [client, refreshConfig]);

  const saveConfig = useCallback(
    async (next: AppConfig, apiKey?: string): Promise<SaveResult> => {
      try {
        const saved = await client.configSave(next, apiKey);
        setConfig(saved);
        if (apiKey?.trim()) setApiKeyConfigured(true);
        setConfigError(null);
        await refreshConfig();
        return { ok: true };
      } catch (error) {
        const clientError = toClientError(error);
        setConfigError(clientError);
        return { ok: false, error: clientError };
      }
    },
    [client, refreshConfig],
  );

  const setUiLanguage = useCallback(
    async (language: UiLanguage | null): Promise<SaveResult> => {
      try {
        const saved = await client.uiLanguageSet(language);
        setConfig(saved);
        setConfigError(null);
        await refreshConfig();
        return { ok: true };
      } catch (error) {
        const clientError = toClientError(error);
        setConfigError(clientError);
        return { ok: false, error: clientError };
      }
    },
    [client, refreshConfig],
  );

  const resetConfig = useCallback(async (): Promise<SaveResult> => {
    try {
      await client.configReset();
      setConfig(null);
      setApiKeyConfigured(false);
      setConfigError(null);
      setLastDiagnostics(null);
      await refreshConfig();
      return { ok: true };
    } catch (error) {
      const clientError = toClientError(error);
      setConfigError(clientError);
      return { ok: false, error: clientError };
    }
  }, [client, refreshConfig]);

  const refreshLifecycleActivity = useCallback(() => {
    void client
      .activityList()
      .then((activity) => {
        activityCursorRef.current = activity.latestSequence;
        setResources((current) => ({
          ...current,
          activity: loaded(activity, activity.items.length === 0),
        }));
      })
      .catch(() => undefined);
  }, [client]);

  const refreshRuntimeResources = useCallback(async () => {
    const [runtime, tools, policies, activity, approvals] = await Promise.all([
      load(() => client.runtimeStatus(), () => false),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
      load(() => client.approvalsList(), (data) => data.items.length === 0),
    ]);

    if (activity.status === "success" || activity.status === "empty") {
      activityCursorRef.current = activity.data.latestSequence;
    }
    setResources({ runtime, tools, policies, activity, approvals });
  }, [client]);

  const refreshApprovals = useCallback(async () => {
    const approvals = await load(() => client.approvalsList(), (data) => data.items.length === 0);
    setResources((current) => ({ ...current, approvals }));
  }, [client]);

  const startHarness = useCallback(async (): Promise<SaveResult> => {
    try {
      const nextEligibility = await client.harnessStartEligibility();
      setEligibility(nextEligibility);
      if (!nextEligibility.canStart) {
        return {
          ok: false,
          error: new HarnessClientError("error", nextEligibility.message, undefined, nextEligibility.status.toUpperCase()),
        };
      }
      const start = client.harnessStart();
      setComponents(client.getComponentStates());
      const state = await start;
      setComponents(client.getComponentStates());
      setHarness(state);
      setActiveRevision(state.configRevision ?? null);
      await refreshRuntimeResources();
      return { ok: true };
    } catch (error) {
      return { ok: false, error: toClientError(error) };
    }
  }, [client, refreshRuntimeResources]);

  const stopHarness = useCallback(async (): Promise<SaveResult> => {
    if (client.getHarnessStatus().lifecycle === "stopped") {
      return {
        ok: false,
        error: new HarnessClientError("error", "Harness is already stopped.", undefined, "LIFECYCLE_CONFLICT"),
      };
    }
    try {
      const stop = client.harnessStop();
      setComponents(client.getComponentStates());
      const stopped = await stop;
      setComponents(client.getComponentStates());
      setHarness(stopped);
      setActiveRevision(null);
      setResources((current) => ({ ...current, runtime: { status: "idle" } }));
      refreshLifecycleActivity();
      await refreshConfig();
      return { ok: true };
    } catch (error) {
      return { ok: false, error: toClientError(error) };
    }
  }, [client, refreshConfig, refreshLifecycleActivity]);

  const restartHarness = useCallback(async () => {
    const stopped = await stopHarness();
    return stopped.ok ? startHarness() : stopped;
  }, [startHarness, stopHarness]);

  const runDiagnostics = useCallback(async () => {
    const result = await client.localDiagnosticsRun();
    setLastDiagnostics(result);
    return result;
  }, [client]);

  const appendActivity = useCallback((event: ActivityEvent) => {
    if (isNewerSequence(activityCursorRef.current, event.sequence)) {
      activityCursorRef.current = event.sequence;
    }
    setResources((current) => {
      if (current.activity.status !== "success" && current.activity.status !== "empty") {
        return current;
      }
      if (current.activity.data.items.some((item) => item.eventId === event.eventId)) {
        return current;
      }
      return {
        ...current,
        activity: loaded(
          {
            items: [...current.activity.data.items, event],
            latestSequence: activityCursorRef.current ?? event.sequence,
          },
          false,
        ),
      };
    });
  }, []);

  const subscribeToActivity = useCallback(
    (listener: ActivityEventListener) =>
      client.activitySubscribe(listener, activityCursorRef.current),
    [client],
  );

  const value = useMemo<ContextValue>(
    () => ({
      ...resources,
      client,
      config,
      configMetadata: metadata,
      configError,
      eligibility,
      setupRequired: configLoaded && (eligibility?.status === "setup_required" || !config?.workspace.rootPath),
      apiKeyConfigured,
      components,
      harness,
      overall: harness.lifecycle,
      restartRequired: activeRevision !== null && config !== null && activeRevision !== config.configRevision,
      lastDiagnostics,
      refreshConfig,
      resetConfig,
      saveConfig,
      setUiLanguage,
      deleteApiKey: async () => {
        await client.apiKeyDelete();
        setApiKeyConfigured(false);
        setConfig((current) =>
          current ? { ...current, tunnel: { ...current.tunnel, apiKeyRef: undefined } } : current,
        );
      },
      chooseWorkspace: () => client.chooseWorkspace(),
      chooseTunnelClient: () => client.chooseTunnelClient(),
      detectTunnelClient: () => client.detectTunnelClient(),
      runDiagnostics,
      startHarness,
      stopHarness,
      restartHarness,
      appendActivity,
      subscribeToActivity,
      refreshApprovals,
    }),
    [
      activeRevision,
      apiKeyConfigured,
      appendActivity,
      client,
      components,
      harness,
      config,
      configError,
      configLoaded,
      eligibility,
      lastDiagnostics,
      metadata,
      refreshConfig,
      resetConfig,
      resources,
      refreshApprovals,
      restartHarness,
      runDiagnostics,
      saveConfig,
      setUiLanguage,
      startHarness,
      stopHarness,
      subscribeToActivity,
    ],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useHarnessData() {
  const value = useContext(Context);
  if (!value) throw new Error("useHarnessData must be used inside HarnessDataProvider");
  return value;
}

