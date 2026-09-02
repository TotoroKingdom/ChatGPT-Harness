import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
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
async function load<T>(fn: () => Promise<T>, empty: (data: T) => boolean): Promise<AsyncState<T>> {
  try {
    const data = await fn();
    return loaded(data, empty(data));
  } catch (error) {
    const clientError = toClientError(error);
    return { status: clientError.kind, error: clientError };
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
  const [activeRevision, setActiveRevision] = useState<bigint | null>(null);
  const [lastDiagnostics, setLastDiagnostics] = useState<DiagnosticRun | null>(null);

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
    void refreshConfig();
    setResources({
      runtime: { status: "loading" },
      tools: { status: "loading" },
      policies: { status: "loading" },
      activity: { status: "loading" },
    });
    void Promise.all([
      load(() => client.runtimeStatus(), () => false),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
    ]).then(([runtime, tools, policies, activity]) => {
      if (!mounted) return;
      setResources({ runtime, tools, policies, activity });
      if (runtime.status === "success" || runtime.status === "empty") {
        setActiveRevision(runtime.data.readiness === "not_ready" ? null : runtime.data.configRevision);
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
      .then((activity) =>
        setResources((current) => ({
          ...current,
          activity: loaded(activity, activity.items.length === 0),
        })),
      )
      .catch(() => undefined);
  }, [client]);

  const startHarness = useCallback(async (): Promise<SaveResult> => {
    if (!config || !apiKeyConfigured) {
      return {
        ok: false,
        error: new HarnessClientError("error", "Setup is incomplete.", undefined, "SETUP_INCOMPLETE"),
      };
    }
    if (overallStatus(client.getComponentStates()) !== "stopped") {
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
      const state = await client.runtimeStatus();
      setResources((current) => ({ ...current, runtime: { status: "success", data: state } }));
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
      appendActivity: (event: ActivityEvent) =>
        setResources((current) =>
          current.activity.status === "success" || current.activity.status === "empty"
            ? {
                ...current,
                activity: loaded(
                  { items: [...current.activity.data.items, event], latestSequence: event.sequence },
                  false,
                ),
              }
            : current,
        ),
      subscribeToActivity: (listener: ActivityEventListener) => client.activitySubscribe(listener),
    }),
    [
      activeRevision,
      apiKeyConfigured,
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
    ],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useHarnessData() {
  const value = useContext(Context);
  if (!value) throw new Error("useHarnessData must be used inside HarnessDataProvider");
  return value;
}
