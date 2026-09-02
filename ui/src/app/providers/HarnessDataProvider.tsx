import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { ActivityEvent, ActivityListResult, PoliciesListResult, RuntimeState, ToolsListResult } from "@protocol/v1";
import { toClientError, type HarnessClientError } from "@/services/harness/errors";
import { createHarnessControlClient } from "@/services/harness/factory";
import type { ActivityEventListener, HarnessControlClient, Unsubscribe } from "@/services/harness/client";

export type AsyncState<T> =
  | { status: "idle" | "loading" }
  | { status: "success"; data: T }
  | { status: "empty"; data: T }
  | { status: "error"; error: HarnessClientError }
  | { status: "disconnected"; error: HarnessClientError };

type ResourceState = {
  runtime: AsyncState<RuntimeState>;
  tools: AsyncState<ToolsListResult>;
  policies: AsyncState<PoliciesListResult>;
  activity: AsyncState<ActivityListResult>;
};

type HarnessDataContextValue = ResourceState & {
  client: HarnessControlClient;
  appendActivity: (event: ActivityEvent) => void;
  subscribeToActivity: (listener: ActivityEventListener) => Promise<Unsubscribe>;
};

const initialState: ResourceState = {
  runtime: { status: "idle" }, tools: { status: "idle" }, policies: { status: "idle" }, activity: { status: "idle" },
};
const HarnessDataContext = createContext<HarnessDataContextValue | null>(null);

function loaded<T>(data: T, isEmpty: boolean): AsyncState<T> { return isEmpty ? { status: "empty", data } : { status: "success", data }; }
async function load<T>(request: () => Promise<T>, isEmpty: (data: T) => boolean): Promise<AsyncState<T>> {
  try { const data = await request(); return loaded(data, isEmpty(data)); }
  catch (error) { const clientError = toClientError(error); return { status: clientError.kind, error: clientError }; }
}

export function HarnessDataProvider({ children }: { children: ReactNode }) {
  const client = useMemo(() => createHarnessControlClient(), []);
  const [state, setState] = useState<ResourceState>(initialState);

  useEffect(() => {
    let mounted = true;
    setState({ runtime: { status: "loading" }, tools: { status: "loading" }, policies: { status: "loading" }, activity: { status: "loading" } });
    void Promise.all([
      load(() => client.runtimeStatus(), () => false),
      load(() => client.toolsList(), (data) => data.items.length === 0),
      load(() => client.policiesList(), (data) => data.items.length === 0),
      load(() => client.activityList(), (data) => data.items.length === 0),
    ]).then(([runtime, tools, policies, activity]) => { if (mounted) setState({ runtime, tools, policies, activity }); });
    return () => { mounted = false; };
  }, [client]);

  const appendActivity = useCallback((newEvent: ActivityEvent) => {
    setState((current) => {
      if (current.activity.status !== "success" && current.activity.status !== "empty") return current;
      const currentData = current.activity.data;
      if (currentData.items.some((item) => item.eventId === newEvent.eventId)) return current;
      return { ...current, activity: loaded({ items: [...currentData.items, newEvent], latestSequence: newEvent.sequence }, false) };
    });
  }, []);
  const subscribeToActivity = useCallback((listener: ActivityEventListener) => client.activitySubscribe(listener), [client]);
  const value = useMemo(() => ({ ...state, client, appendActivity, subscribeToActivity }), [appendActivity, client, state, subscribeToActivity]);

  return <HarnessDataContext.Provider value={value}>{children}</HarnessDataContext.Provider>;
}

export function useHarnessData() {
  const context = useContext(HarnessDataContext);
  if (!context) throw new Error("useHarnessData must be used inside HarnessDataProvider");
  return context;
}
