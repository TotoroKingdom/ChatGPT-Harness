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
  StartupAttempt,
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
  DesktopPreferencesPatch,
  Unsubscribe,
} from "@/services/harness/client";
import { overallStatus } from "@/services/harness/client";
import { ConfigMutationQueue, configIntent, type ConfigIntent } from "./config-mutations";
import {
  applyTheme,
  getBootstrappedThemePreference,
  getSystemThemeDark,
  normalizeThemePreference,
  observeSystemTheme,
  resolveEffectiveTheme,
  type EffectiveTheme,
  type ThemePreference,
} from "@/theme/theme";

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
  latestStartupAttempt?: StartupAttempt | null;
  refreshConfig: () => Promise<void>;
  refreshHarness: () => Promise<void>;
  resetConfig: () => Promise<SaveResult>;
  saveConfig: (config: AppConfig, apiKey?: string) => Promise<SaveResult>;
  mutateConfig: (intent: ConfigIntent, apiKey?: string, initial?: AppConfig) => Promise<SaveResult>;
  setUiLanguage: (language: UiLanguage | null) => Promise<SaveResult>;
  setDesktopPreferences: (patch: DesktopPreferencesPatch) => Promise<SaveResult>;
  continueInBackground: () => Promise<SaveResult>;
  quitDesktop: () => Promise<SaveResult>;
  themePreference: ThemePreference;
  effectiveTheme: EffectiveTheme;
  setThemePreference: (preference: ThemePreference) => void;
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

// Activity notifications only tell the UI that the authoritative approval
// queue may have changed. The pending count is always read from Runtime.
const approvalSignalEvents = new Set<ActivityEvent["eventType"]>([
  "approval_requested",
  "approval_resolved",
  "approval_denied",
  "approval_expired",
  "approval_cancelled",
]);

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
  const [themePreference, setThemePreferenceState] = useState<ThemePreference>(() => getBootstrappedThemePreference());
  const [effectiveTheme, setEffectiveTheme] = useState<EffectiveTheme>(() => (
    typeof document !== "undefined" && document.documentElement.dataset.theme === "light" ? "light" : "dark"
  ));
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
  const [latestStartupAttempt, setLatestStartupAttempt] = useState<StartupAttempt | null>(null);
  const activityCursorRef = useRef<string | undefined>(undefined);
  const observedSession = useRef<string | null | undefined>(undefined);
  const observedRuntimeId = useRef<string | null>(null);
  const observedRuntimeStatus = useRef<ComponentStates["runtime"] | undefined>(undefined);
  const resourceEpoch = useRef(0);
  const observedComponents = useRef<string | undefined>(undefined);
  const approvalReadSequence = useRef(0);
  const approvalRefreshTimer = useRef<number | null>(null);
  const mutations = useRef(new ConfigMutationQueue());
  const persistedVersion = useRef(0);

  const setThemePreference = useCallback((preference: ThemePreference) => {
    const normalized = normalizeThemePreference(preference);
    const effective = resolveEffectiveTheme(normalized, getSystemThemeDark());
    setThemePreferenceState(normalized);
    setEffectiveTheme(effective);
    applyTheme(effective);
  }, []);

  const refreshApprovals = useCallback(async () => {
    const sequence = ++approvalReadSequence.current;
    const epoch = resourceEpoch.current;
    const runtimeId = observedRuntimeId.current;
    const session = client.getHarnessStatus().activeSessionId ?? null;
    const approvals = await load(() => client.approvalsList(), (data) => data.items.length === 0);
    if (sequence !== approvalReadSequence.current || epoch !== resourceEpoch.current) return;
    if (runtimeId !== null && observedRuntimeId.current !== runtimeId) return;
    if ((client.getHarnessStatus().activeSessionId ?? null) !== session) return;
    setResources((current) => ({ ...current, approvals }));
  }, [client]);

  const scheduleApprovalRefresh = useCallback((delayMs = 350) => {
    if (typeof window === "undefined" || approvalRefreshTimer.current !== null) return;
    approvalRefreshTimer.current = window.setTimeout(() => {
      approvalRefreshTimer.current = null;
      void refreshApprovals();
    }, delayMs);
  }, [refreshApprovals]);

  useEffect(() => () => {
    if (approvalRefreshTimer.current !== null) {
      window.clearTimeout(approvalRefreshTimer.current);
      approvalRefreshTimer.current = null;
    }
  }, []);

  // Low-frequency reconciliation is the fallback when activity delivery is
  // interrupted or an approval change produces no event we can observe.
  useEffect(() => {
    if (components.runtime !== "ready" && components.runtime !== "degraded") return;
    const timer = window.setInterval(() => { void refreshApprovals(); }, 30_000);
    return () => window.clearInterval(timer);
  }, [components.runtime, refreshApprovals]);

  const previousRuntimeStatus = useRef(components.runtime);
  useEffect(() => {
    const previous = previousRuntimeStatus.current;
    previousRuntimeStatus.current = components.runtime;
    if ((components.runtime === "ready" || components.runtime === "degraded") && previous !== components.runtime) {
      void refreshApprovals();
    }
  }, [components.runtime, refreshApprovals]);

  useEffect(() => {
    observedRuntimeId.current = resources.runtime.status === "success" || resources.runtime.status === "empty"
      ? resources.runtime.data.runtimeId
      : null;
  }, [resources.runtime]);

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
    if (!configLoaded) return;
    setThemePreference(normalizeThemePreference(config?.ui.theme));
  }, [configLoaded, config?.ui.theme, setThemePreference]);

  useEffect(() => {
    if (themePreference !== "system") return;
    const update = (systemDark: boolean) => {
      const effective = resolveEffectiveTheme("system", systemDark);
      setEffectiveTheme(effective);
      applyTheme(effective);
    };
    update(getSystemThemeDark());
    return observeSystemTheme(update);
  }, [themePreference]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: Unsubscribe | undefined;
    void client.componentStatesSubscribe((nextComponents, runtime, process) => {
      if (!mounted) return;
      setComponents(nextComponents);
      const harnessState = client.getHarnessStatus();
      const session = harnessState.activeSessionId ?? null;
      const previousRuntime = observedRuntimeStatus.current;
      observedRuntimeStatus.current = nextComponents.runtime;
      const signature = `${nextComponents.runtime}:${nextComponents.mcp}:${nextComponents.tunnel}:${session}`;
      if (observedComponents.current !== undefined && observedComponents.current !== signature) resourceEpoch.current += 1;
      observedComponents.current = signature;
      const sessionChanged = observedSession.current !== undefined && observedSession.current !== session;
      if (sessionChanged) {
        // Old policy/approval snapshots must not describe a new managed session.
        approvalReadSequence.current += 1;
        setResources((current) => ({ ...current, policies: { status: "idle" }, approvals: { status: "idle" } }));
      }
      observedSession.current = session;
      if (nextComponents.runtime !== "ready" && nextComponents.runtime !== "degraded") {
        approvalReadSequence.current += 1;
        setResources((current) => ({ ...current, policies: { status: "idle" }, approvals: { status: "idle" } }));
      } else if (sessionChanged || (previousRuntime !== undefined && previousRuntime !== nextComponents.runtime)) {
        // Reconnect or a new managed session: reconcile the queue immediately.
        scheduleApprovalRefresh(0);
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
        observedRuntimeId.current = null;
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
        observedRuntimeId.current = null;
        setActiveRevision(null);
        setResources((current) => ({ ...current, runtime: { status: "idle" }, policies: { status: "idle" }, approvals: { status: "idle" } }));
      } else if (runtime) {
        observedRuntimeId.current = runtime.runtimeId || null;
        setActiveRevision(runtime.runtimeId ? runtime.configRevision : null);
        setResources((current) => ({ ...current, runtime: { status: "success", data: runtime } }));
      } else if (nextComponents.runtime === "stopped" || nextComponents.runtime === "error") {
        observedRuntimeId.current = null;
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
  }, [client, scheduleApprovalRefresh]);

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
      observedRuntimeStatus.current = nextComponents.runtime;
      observedRuntimeId.current = runtime.resource.status === "success" || runtime.resource.status === "empty"
        ? runtime.resource.data.runtimeId
        : null;
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

  const setDesktopPreferences = useCallback(
    (patch: DesktopPreferencesPatch): Promise<SaveResult> => mutations.current.run(async () => {
      try {
        const saved = await client.desktopPreferencesSet(patch);
        persistedVersion.current += 1;
        setConfig(saved); setConfigError(null);
        return { ok: true };
      } catch (error) { return { ok: false, error: toClientError(error) }; }
    }), [client],
  );

  const continueInBackground = useCallback((): Promise<SaveResult> => mutations.current.run(async () => {
    try {
      await client.desktopContinueInBackground();
      return { ok: true };
    } catch (error) { return { ok: false, error: toClientError(error) }; }
  }), [client]);

  const quitDesktop = useCallback((): Promise<SaveResult> => mutations.current.run(async () => {
    try {
      await client.desktopQuit();
      return { ok: true };
    } catch (error) { return { ok: false, error: toClientError(error) }; }
  }), [client]);

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
    const approvalSequence = approvalReadSequence.current;
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
    observedRuntimeId.current = runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : null;
    setResources((current) => ({
      runtime, tools, policies, activity,
      // A dedicated approval refresh that started later is newer evidence.
      approvals: approvalSequence === approvalReadSequence.current ? approvals : current.approvals,
    }));
  }, [client]);

  const refreshHarness = useCallback(async () => {
    try {
      const state = await client.harnessStatus();
      setHarness(state); setHarnessObserved(true);
      client.setComponentStates({ runtime: state.runtime, mcp: state.mcp, tunnel: state.tunnel });
      setComponents(client.getComponentStates());
      const runtime = await rehydrateRuntime(client);
      observedRuntimeId.current = runtime.resource.status === "success" || runtime.resource.status === "empty"
        ? runtime.resource.data.runtimeId
        : null;
      observedRuntimeStatus.current = state.runtime;
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
      observedRuntimeId.current = null;
      setActiveRevision(null);
      setResources((current) => ({ ...current, runtime: { status: "idle" } }));
      refreshLifecycleActivity();
      await refreshConfig();
      return { ok: true };
    } catch (error) {
      return { ok: false, error: toClientError(error) };
    }
  }, [client, refreshConfig, refreshLifecycleActivity]);

  const restartHarness = useCallback(async (): Promise<SaveResult> => {
    try {
      // Keep Stop + Start inside the Host's existing lifecycle operation lock.
      // Separate UI commands allow Tray actions to interleave between them.
      const state = await client.harnessRestart();
      setComponents(client.getComponentStates());
      setHarness(state);
      setHarnessObserved(true);
      setActiveRevision(state.configRevision ?? null);
      await refreshRuntimeResources();
      return { ok: true };
    } catch (error) {
      // Report the failed operation immediately; reconcile the actual Host
      // state separately so the user can retry a bounded shutdown/start.
      void refreshHarness();
      return { ok: false, error: toClientError(error) };
    }
  }, [client, refreshHarness, refreshRuntimeResources]);

  const runDiagnostics = useCallback(async () => {
    const [diagnostics, startup] = await Promise.allSettled([
      client.connectionDiagnosticsRun(),
      client.startupLatest(),
    ]);
    // Diagnostics and startup evidence are independent observations. A
    // failure in one must not discard a successful result from the other.
    if (diagnostics.status === "fulfilled") setLastDiagnostics(diagnostics.value);
    if (startup.status === "fulfilled") setLatestStartupAttempt(startup.value);
    if (diagnostics.status === "rejected") throw diagnostics.reason;
    if (startup.status === "rejected") throw startup.reason;
    return diagnostics.value;
  }, [client]);

  const appendActivity = useCallback((event: ActivityEvent) => {
    const cursor = activityCursorRef.current;
    // Out-of-order or duplicate notifications must not reopen an older
    // sequence or append an event the UI has already observed.
    if (cursor !== undefined && !isNewerSequence(cursor, event.sequence)) return;
    activityCursorRef.current = event.sequence;
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
    // Events from a previous Runtime identity cannot describe the current
    // session's approval queue.
    if (
      approvalSignalEvents.has(event.eventType)
      && (observedRuntimeId.current === null || observedRuntimeId.current === event.runtimeId)
    ) {
      scheduleApprovalRefresh();
    }
  }, [scheduleApprovalRefresh]);

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
      latestStartupAttempt,
      themePreference,
      effectiveTheme,
      setThemePreference,
      refreshConfig,
      refreshHarness,
      resetConfig,
      saveConfig,
      mutateConfig,
      setUiLanguage,
      setDesktopPreferences,
      continueInBackground,
      quitDesktop,
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
      latestStartupAttempt,
      themePreference,
      effectiveTheme,
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
      setDesktopPreferences,
      continueInBackground,
      quitDesktop,
      setThemePreference,
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

