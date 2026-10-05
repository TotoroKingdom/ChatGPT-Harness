import { useEffect, useState } from "react";
import { useHarnessData } from "./HarnessDataProvider";
import { controlReadError, type ControlReadError } from "@/services/harness/control-read";

export type ReadState<T> = { status: "loading" } | { status: "success"; data: T } | { status: "error"; error: ControlReadError };

// Lifecycle signals only invalidate the snapshot. They NEVER supply readiness,
// policy, workspace or availability values; those come from the read API alone.
export function useControlRead<T>(read: () => Promise<T>, enabled = true) {
  const { components, runtime, harness } = useHarnessData();
  const identity = runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : "";
  const epoch = components.runtime + ":" + identity + ":" + (harness?.activeSessionId ?? "");
  const [revision, setRevision] = useState(0);
  const [result, setResult] = useState<{ read: typeof read; epoch: string; revision: number; state: ReadState<T> }>();
  useEffect(() => {
    if (!enabled) {
      setResult(undefined);
      return;
    }
    let active = true;
    void (async () => {
      try {
        const data = await read();
        if (active) setResult({ read, epoch, revision, state: { status: "success", data } });
      } catch (error) {
        if (active) setResult({ read, epoch, revision, state: { status: "error", error: controlReadError(error) } });
      }
    })();
    return () => { active = false; };
  }, [enabled, read, epoch, revision]);
  const state: ReadState<T> = !enabled
    ? { status: "loading" }
    : result?.read === read && result.epoch === epoch && result.revision === revision
      ? result.state
      : { status: "loading" };
  return { state, refresh: () => setRevision((value) => value + 1) };
}
