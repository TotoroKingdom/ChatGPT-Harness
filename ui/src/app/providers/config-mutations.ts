import type { AppConfig } from "@protocol/v1";
import { HarnessClientError } from "@/services/harness/errors";

/** UI-local serialization only; this is not a cross-process transaction. */
export class ConfigMutationQueue {
  private tail: Promise<unknown> = Promise.resolve();
  run<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.tail.then(operation);
    this.tail = result.catch(() => undefined);
    return result;
  }
}

export type ConfigIntent = (latest: AppConfig) => AppConfig;

/** Explicit editors compare only their own object, including the queue-time read. */
export function assertUnchanged(observed: unknown, latest: unknown) {
  if (JSON.stringify(observed) !== JSON.stringify(latest)) {
    throw new HarnessClientError("error", "This object changed. Reload before saving.", undefined, "CONFIG_CONFLICT");
  }
}

/** Transitional caller adapter. Capture changed leaves, never replay a snapshot. */
export function configIntent(base: AppConfig | null, next: AppConfig): ConfigIntent {
  function merge(before: unknown, wanted: unknown, latest: unknown): unknown {
    if (JSON.stringify(before) === JSON.stringify(wanted)) return latest;
    if (wanted && before && typeof wanted === "object" && typeof before === "object" && !Array.isArray(wanted)) {
      const result = { ...(latest as Record<string, unknown>) };
      for (const key of new Set([...Object.keys(before), ...Object.keys(wanted)])) {
        result[key] = merge((before as Record<string, unknown>)[key], (wanted as Record<string, unknown>)[key], result[key]);
      }
      return result;
    }
    return wanted;
  }
  return (latest) => merge(base, next, latest) as AppConfig;
}
