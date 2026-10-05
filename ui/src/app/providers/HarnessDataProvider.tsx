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
import { ConfigMutationQueue, configIntent, type ConfigIntent } from "./config-mutations";

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
export type SaveResult = { ok: boolean; error?: HarnessClientError; refreshUnconfirmed?: boolean };

type ContextValue = ResourceState & {
  client: HarnessControlClient;
  config: AppConfig | null;
  configMetadata: ConfigMetadata | null;
  configError: HarnessClientError | null;
  eligibility: StartEligibility | null;
  setupRequired: boolean;
  apiKeyConfigured: boolean;
  apiKeyObserved: boolean;
  components: ComponentStates;
  harness: HarnessStatus;
  harnessObserved: boolean;
  overall: OverallStatus;
  restartRequired: boolean;
  activeRevision: string | null;
  lastDiagnostics: DiagnosticRun | null;
  refreshConfig: () => Promise<void>;
  refreshHarness: () => Promise<void>;
  resetConfig: () => Promise<SaveResult>;
  saveConfig: (config: AppConfig, apiKey?: string) => Promise<SaveResult>;
  mutateConfig: (intent: ConfigIntent, apiKey?: string, initial?: AppConfig) => Promise<SaveResult>;
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
  const [apiKeyObserved, setApiKeyObserved] = useState(false);
  const [components, setComponents] = useState<ComponentStates>(client.getComponentStates());
  const [harness, setHarness] = useState<HarnessStatus>(client.getHarnessStatus());
  const [harnessObserved, setHarnessObserved] = useState(false);
  const [activeRevision, setActiveRevision] = useState<string | null>(null);
  const [lastDiagnostics, setLastDiagnostics] = useState<DiagnosticRun | null>(null);
  const activityCursorRef = useRef<string | undefined>(undefined);
  const observedSession = useRef<string | null | undefined>(undefined);
  const resourceEpoch = useRef(0);
  const observedComponents = useRef<string | undefined>(undefined);
  const mutations = useRef(new ConfigMutationQueue());
  const persistedVersion = useRef(0);

  const refreshConfig = useCallback(async () => {
    const version = persistedVersion.current;
    const [content, location, key, start] = await Promise.allSettled([
      client.configGet(), client.configMetadata(), client.apiKeyStatus(), client.harnessStartEligibility(),
    ]);
    // Metadata is independent from content validation / migration.
    if (location.status === "fulfilled") setMetadata(location.value);
    if (version !== persistedVersion.current) return;
    if (key.status === "fulfilled") { setApiKeyConfigured(key.value); setApiKeyObserved(true); }
    else setApiKeyObserved(false);
    if (content.status === "fulfilled") {
      setConfig(content.value); setConfigLoaded(true); setConfigError(null);
      setEligibility(start.status === "fulfilled" ? start.value : null);
    } else {
      setConfig(null); setConfigLoaded(true); setConfigError(toClientError(content.reason)); setEligibility(null);
    }
  }, [client]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: Unsubscribe | undefined;
    void client.componentStatesSubscribe((nextComponents, runtime, process) => {
      if (!mounted) return;
      setComponents(nextComponents);
      const harnessState = client.getHarnessStatus();
      const session = harnessState.activeSessionId ?? null;
      const signature = `${nextComponents.runtime}:${nextComponents.mcp}:${nextComponents.tunnel}:${session}`;
      if (observedComponents.current !== undefined && observedComponents.current !== signature) resourceEpoch.current += 1;
      observedComponents.current = signature;
      if (observedSession.current !== undefined && observedSession.current !== session) {
        // Old policy/approval snapshots must not describe a new managed session.
        setResources((current) => ({ ...current, policies: { status: "idle" }, approvals: { status: "idle" } }));
      }
      observedSession.current = session;
      if (nextComponents.runtime !== "ready" && nextComponents.runtime !== "degraded") {
        setResources((current) => ({ ...current, policies: { status: "idle" }, approvals: { status: "idle" } }));
      }
      setHarness(harnessState);
      setHarnessObserved(true);
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
        setResources((current) => ({ ...current, runtime: { status: "idle" }, policies: { status: "idle" }, approvals: { status: "idle" } }));
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
    void (async () => {
      // Establish the managed session before reading its dependent resources.
      const harnessResource = await load(() => client.harnessStatus(), () => false);
      if (!mounted) return;
      const confirmed = harnessResource.status === "success" || harnessResource.status === "empty";
      if (confirmed) {
        setHarness(harnessResource.data); setHarnessObserved(true);
        const state = harnessResource.data;
        observedComponents.current = `${state.runtime}:${state.mcp}:${state.tunnel}:${state.activeSessionId ?? null}`;
        observedSession.current = state.activeSessionId ?? null;
        client.setComponentStates({ runtime: state.runtime, mcp: state.mcp, tunnel: state.tunnel });
        setComponents(client.getComponentStates());
      } else setHarnessObserved(false);
      const epoch = resourceEpoch.current;
      const [runtime, tools, policies, activity, approvals] = await Promise.all([
        rehydrateRuntime(client),
        load(() => client.toolsList(), (data) => data.items.length === 0),
        load(() => client.policiesList(), (data) => data.items.length === 0),
        load(() => client.activityList(), (data) => data.items.length === 0),
        load(() => client.approvalsList(), (data) => data.items.length === 0),
      ]);
      if (!mounted || epoch !== resourceEpoch.current) return;
      const nextComponents = { ...runtime.components, tunnel: confirmed ? harnessResource.data.tunnel : runtime.components.tunnel };
      client.setComponentStates(nextComponents); setComponents(nextComponents);
      setResources({ runtime: runtime.resource, tools, policies, activity, approvals });
      setActiveRevision(runtime.activeRevision);
      if (activity.status === "success" || activity.status === "empty") activityCursorRef.current = activity.data.latestSequence;
    })();
    return () => {
      mounted = false;
    };
  }, [client, refreshConfig]);

  const mutateConfig = useCallback(
    (intent: ConfigIntent, apiKey?: string, initial?: AppConfig): Promise<SaveResult> => mutations.current.run(async () => {
      let saved: AppConfig;
      try {
        const latest = (await client.configGet()) ?? initial;
        if (!latest) throw new HarnessClientError("error", "Save configuration first.", undefined, "CONFIG_REQUIRED");
        saved = await client.configSave(intent(latest), apiKey);
      } catch (error) {
        // Config/SecretStore may have partially changed. Read back, never promise rollback.
        persistedVersion.current += 1;
        await refreshConfig();
        return { ok: false, error: toClientError(error) };
      }
      persistedVersion.current += 1;
      setConfig(saved); setConfigError(null);
      if (apiKey?.trim()) { setApiKeyConfigured(true); setApiKeyObserved(true); }
      let refreshUnconfirmed = false;
      try {
        const observed = await client.configGet();
        if (observed) setConfig(observed);
        else refreshUnconfirmed = true;
        const start = await client.harnessStartEligibility();
        setEligibility(start);
      } catch { refreshUnconfirmed = true; }
      return { ok: true, refreshUnconfirmed };
    }), [client, refreshConfig],
  );

  const saveConfig = useCallback(
    (next: AppConfig, apiKey?: string) => {
      // First Run can persist the first complete object; subsequent saves merge intent.
      if (!config) return mutations.current.run(async () => {
        try {
          if (await client.configGet()) throw new HarnessClientError("error", "Configuration changed. Reload before saving.", undefined, "CONFIG_CONFLICT");
          const saved = await client.configSave(next, apiKey);
          persistedVersion.current += 1;
          setConfig(saved); setConfigError(null);
          if (apiKey?.trim()) { setApiKeyConfigured(true); setApiKeyObserved(true); }
          await refreshConfig();
          return { ok: true };
        } catch (error) { return { ok: false, error: toClientError(error) }; }
      });
      return mutateConfig(configIntent(config, next), apiKey);
    }, [client, config, mutateConfig, refreshConfig],
  );

  const setUiLanguage = useCallback(
    (language: UiLanguage | null): Promise<SaveResult> => mutations.current.run(async () => {
      try {
        const saved = await client.uiLanguageSet(language);
        persistedVersion.current += 1;
        setConfig(saved); setConfigError(null);
        return { ok: true };
      } catch (error) { return { ok: false, error: toClientError(error) }; }
    }), [client],
  );

  const resetConfig = useCallback((): Promise<SaveResult> => mutations.current.run(async () => {
    try {
      await client.configReset();
      persistedVersion.current += 1;
      setConfig(null); setApiKeyConfigured(false); setApiKeyObserved(true); setConfigError(null); setLastDiagnostics(null);
      await refreshConfig();
      return { ok: true };
    } catch (error) { persistedVersion.current += 1; await refreshConfig(); return { ok: false, error: toClientError(error) }; }
  }), [client, refreshConfig]);

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
    const epoch = resourceEpoch.current;
    const [runtime, tools, policies, activity, approvals] = await Promise.all([
      load(() => client.runtimeStatus(), () => false),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
      load(() => client.approvalsList(), (data) => data.items.length === 0),
    ]);

    if (epoch !== resourceEpoch.current) return;
    if (activity.status === "success" || activity.status === "empty") {
      activityCursorRef.current = activity.data.latestSequence;
    }
    setResources({ runtime, tools, policies, activity, approvals });
  }, [client]);

  const refreshApprovals = useCallback(async () => {
    const epoch = resourceEpoch.current;
    const approvals = await load(() => client.approvalsList(), (data) => data.items.length === 0);
    if (epoch === resourceEpoch.current) setResources((current) => ({ ...current, approvals }));
  }, [client]);

  const refreshHarness = useCallback(async () => {
    try {
      const state = await client.harnessStatus();
      setHarness(state); setHarnessObserved(true);
      client.setComponentStates({ runtime: state.runtime, mcp: state.mcp, tunnel: state.tunnel });
      setComponents(client.getComponentStates());
      const runtime = await rehydrateRuntime(client);
      setActiveRevision(runtime.activeRevision);
      setResources((current) => ({ ...current, runtime: runtime.resource }));
    } catch { setHarnessObserved(false); }
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
      setHarnessObserved(true);
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
      setHarnessObserved(true);
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
      apiKeyObserved,
      components,
      harness,
      harnessObserved,
      overall: harness.lifecycle,
      activeRevision,
      restartRequired: activeRevision !== null && config !== null && activeRevision !== config.configRevision,
      lastDiagnostics,
      refreshConfig,
      refreshHarness,
      resetConfig,
      saveConfig,
      mutateConfig,
      setUiLanguage,
      deleteApiKey: () => mutations.current.run(async () => {
        await client.apiKeyDelete();
        persistedVersion.current += 1;
        setApiKeyConfigured(false);
        setApiKeyObserved(true);
        // The command deletes SecretStore content, not the persisted reference.
        await refreshConfig();
      }),
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
      apiKeyObserved,
      appendActivity,
      client,
      components,
      harness,
      harnessObserved,
      config,
      configError,
      configLoaded,
      eligibility,
      lastDiagnostics,
      metadata,
      refreshConfig,
      refreshHarness,
      resetConfig,
      resources,
      refreshApprovals,
      restartHarness,
      runDiagnostics,
      saveConfig,
      mutateConfig,
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

