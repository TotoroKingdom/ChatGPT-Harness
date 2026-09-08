import { useEffect, useState } from "react";
import { useHarnessData } from "./HarnessDataProvider";
import { controlReadError } from "@/services/harness/control-read";

export type ReadState<T> = { status: "loading" } | { status: "success"; data: T } | { status: "error"; message: string };

// Lifecycle signals only invalidate the snapshot. They NEVER supply readiness,
// policy, workspace or availability values; those come from the read API alone.
export function useControlRead<T>(read: () => Promise<T>) {
  const { components, runtime } = useHarnessData();
  const identity = runtime.status === "success" || runtime.status === "empty" ? runtime.data.runtimeId : "";
  const epoch = components.runtime + ":" + identity;
  const [revision, setRevision] = useState(0);
  const [result, setResult] = useState<{ read: typeof read; epoch: string; revision: number; state: ReadState<T> }>();
  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const data = await read();
        if (active) setResult({ read, epoch, revision, state: { status: "success", data } });
      } catch (error) {
        if (active) setResult({ read, epoch, revision, state: { status: "error", message: controlReadError(error) } });
      }
    })();
    return () => { active = false; };
  }, [read, epoch, revision]);
  const state: ReadState<T> = result?.read === read && result.epoch === epoch && result.revision === revision ? result.state : { status: "loading" };
  return { state, refresh: () => setRevision((value) => value + 1) };
}
