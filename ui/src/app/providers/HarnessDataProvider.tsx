import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  ActivityEvent,
  ActivityListResult,
  AppConfig,
  DiagnosticRun,
  PoliciesListResult,
  RuntimeState,
  ToolsListResult,
} from "@protocol/v1";
import { HarnessClientError, toClientError } from "@/services/harness/errors";
import { createHarnessControlClient } from "@/services/harness/factory";
import type {
  ActivityEventListener,
  ComponentStates,
  ConfigMetadata,
  HarnessControlClient,
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
};
export type SaveResult = { ok: boolean; error?: HarnessClientError };

type ContextValue = ResourceState & {
  client: HarnessControlClient;
  config: AppConfig | null;
  configMetadata: ConfigMetadata | null;
  configError: HarnessClientError | null;
  setupRequired: boolean;
  apiKeyConfigured: boolean;
  components: ComponentStates;
  overall: OverallStatus;
  restartRequired: boolean;
  lastDiagnostics: DiagnosticRun | null;
  refreshConfig: () => Promise<void>;
  resetConfig: () => Promise<SaveResult>;
  saveConfig: (config: AppConfig, apiKey?: string) => Promise<SaveResult>;
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
};

const initial: ResourceState = {
  runtime: { status: "idle" },
  tools: { status: "idle" },
  policies: { status: "idle" },
  activity: { status: "idle" },
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

function runtimeComponentState(state: RuntimeState): ComponentStates["runtime"] {
  if (state.lifecycle === "error") return "error";
  if (!state.runtimeId) return "stopped";
  if (state.lifecycle === "stopping") return "stopping";
  if (state.lifecycle === "ready" && state.readiness === "ready") return "ready";
  if (state.readiness === "degraded") return "degraded";
  return "starting";
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
            components: { ...client.getComponentStates(), runtime: runtimeComponentState(resource.data) },
            activeRevision: resource.data.configRevision,
          };
        }
        return { resource, components: { ...current, runtime: "error" }, activeRevision: null };
      }
      case "stopped":
        return {
          resource: { status: "idle" },
          components: { ...current, runtime: "stopped" },
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
          components: { ...current, runtime: "error" },
          activeRevision: null,
        };
      case "starting":
        return {
          resource: { status: "idle" },
          components: { ...current, runtime: "starting" },
          activeRevision: null,
        };
      case "stopping":
        return {
          resource: { status: "idle" },
          components: { ...current, runtime: "stopping" },
          activeRevision: null,
        };
    }
  } catch (error) {
    const clientError = toClientError(error);
    return {
      resource: { status: clientError.kind, error: clientError },
      components: { ...current, runtime: "error" },
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
  const [apiKeyConfigured, setApiKeyConfigured] = useState(false);
  const [components, setComponents] = useState<ComponentStates>(client.getComponentStates());
  const [activeRevision, setActiveRevision] = useState<string | null>(null);
  const [lastDiagnostics, setLastDiagnostics] = useState<DiagnosticRun | null>(null);
  const activityCursorRef = useRef<string | undefined>(undefined);

  const refreshConfig = useCallback(async () => {
    try {
      const [nextConfig, nextMetadata, keyConfigured] = await Promise.all([
        client.configGet(),
        client.configMetadata(),
        client.apiKeyStatus(),
      ]);
      setConfig(nextConfig);
      setConfigLoaded(true);
      setMetadata(nextMetadata);
      setApiKeyConfigured(keyConfigured);
      setConfigError(null);
    } catch (error) {
      const clientError = toClientError(error);
      if (clientError.code === "CONFIG_INVALID") setConfigLoaded(true);
      setConfigError(clientError);
    }
  }, [client]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: Unsubscribe | undefined;
    void client.componentStatesSubscribe((nextComponents, runtime) => {
      if (!mounted) return;
      setComponents(nextComponents);
      if (runtime) {
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
    });
    void Promise.all([
      rehydrateRuntime(client),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
    ]).then(([runtime, tools, policies, activity]) => {
      if (!mounted) return;
      client.setComponentStates(runtime.components);
      setComponents(runtime.components);
      setResources({ runtime: runtime.resource, tools, policies, activity });
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
        return { ok: true };
      } catch (error) {
        const clientError = toClientError(error);
        setConfigError(clientError);
        return { ok: false, error: clientError };
      }
    },
    [client],
  );

  const resetConfig = useCallback(async (): Promise<SaveResult> => {
    try {
      await client.configReset();
      setConfig(null);
      setApiKeyConfigured(false);
      setConfigError(null);
      setLastDiagnostics(null);
      return { ok: true };
    } catch (error) {
      const clientError = toClientError(error);
      setConfigError(clientError);
      return { ok: false, error: clientError };
    }
  }, [client]);

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

  const startHarness = useCallback(async (): Promise<SaveResult> => {
    if (!config || !apiKeyConfigured) {
      return {
        ok: false,
        error: new HarnessClientError("error", "Setup is incomplete.", undefined, "SETUP_INCOMPLETE"),
      };
    }
    const overall = overallStatus(client.getComponentStates());
    if (overall !== "stopped" && overall !== "error") {
      return {
        ok: false,
        error: new HarnessClientError("error", "Harness is already running or changing state.", undefined, "LIFECYCLE_CONFLICT"),
      };
    }
    try {
      const diagnostics = await client.localDiagnosticsRun();
      setLastDiagnostics(diagnostics);
      if (diagnostics.overallStatus === "fail") {
        return {
          ok: false,
          error: new HarnessClientError("error", "Diagnostics found a blocking configuration issue.", undefined, "SETUP_INCOMPLETE"),
        };
      }
      const start = client.runtimeStart();
      setComponents(client.getComponentStates());
      const state = await start;
      setComponents(client.getComponentStates());
      setActiveRevision(state.configRevision);
      setResources((current) => ({ ...current, runtime: { status: "success", data: state } }));
      refreshLifecycleActivity();
      return { ok: true };
    } catch (error) {
      return { ok: false, error: toClientError(error) };
    }
  }, [apiKeyConfigured, client, config, refreshLifecycleActivity]);

  const stopHarness = useCallback(async (): Promise<SaveResult> => {
    if (overallStatus(client.getComponentStates()) === "stopped") {
      return {
        ok: false,
        error: new HarnessClientError("error", "Harness is already stopped.", undefined, "LIFECYCLE_CONFLICT"),
      };
    }
    try {
      const stop = client.runtimeStop();
      setComponents(client.getComponentStates());
      await stop;
      setComponents(client.getComponentStates());
      setActiveRevision(null);
      setResources((current) => ({ ...current, runtime: { status: "idle" } }));
      refreshLifecycleActivity();
      return { ok: true };
    } catch (error) {
      return { ok: false, error: toClientError(error) };
    }
  }, [client, refreshLifecycleActivity]);

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
      setupRequired: configLoaded && !config,
      apiKeyConfigured,
      components,
      overall: overallStatus(components),
      restartRequired: activeRevision !== null && config !== null && activeRevision !== config.configRevision,
      lastDiagnostics,
      refreshConfig,
      resetConfig,
      saveConfig,
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
    }),
    [
      activeRevision,
      apiKeyConfigured,
      appendActivity,
      client,
      components,
      config,
      configError,
      configLoaded,
      lastDiagnostics,
      metadata,
      refreshConfig,
      resetConfig,
      resources,
      restartHarness,
      runDiagnostics,
      saveConfig,
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

