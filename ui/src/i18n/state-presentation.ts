import type { ComponentStatus } from "@/services/harness/client";
import type { ToolDiagnostic } from "@/services/harness/control-read";
import { toolAvailabilityKey, type TranslationKey } from "./presentation";

/** Display-only: availability already contains Runtime's decision. */
export function currentToolState(component: ComponentStatus, observation: { status: string }, tool?: ToolDiagnostic): string {
  if (component === "stopped") return "tools.notRunning";
  if (component === "error") return "m8.unknown";
  if (component === "starting" || component === "stopping" || observation.status === "loading") return "common.loading";
  if (observation.status !== "success") return "m8.unknown";
  return tool ? toolAvailabilityKey(tool.availability) : "tools.notReported";
}
export function enabledMismatch(saved: boolean, tool?: ToolDiagnostic): "offStillAvailable" | "onStillDisabled" | null {
  if (!tool) return null;
  if (!saved && (tool.availability === "available" || tool.availability === "available_with_approval")) return "offStillAvailable";
  if (saved && tool.availability === "disabled") return "onStillDisabled";
  return null;
}
export type DisplayTranslationKey = TranslationKey | `m8.${string}`;

export function hasPolicyCritical(component: ComponentStatus, policy: { status: string; data?: { effective?: { valid: boolean } } }): boolean {
  return (component === "ready" || component === "degraded") && (policy.status === "success" || policy.status === "empty") && policy.data?.effective?.valid === false;
}
