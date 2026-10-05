import { expect, it } from "vitest";
import { currentToolState, enabledMismatch, hasPolicyCritical } from "./state-presentation";
import type { ToolDiagnostic } from "@/services/harness/control-read";

const available: ToolDiagnostic = { name: "bash", availability: "available", decision: "allow" };
it("never uses desired config or stale observations to declare availability", () => {
  expect(currentToolState("stopped", { status: "success" }, available)).toBe("tools.notRunning");
  expect(currentToolState("ready", { status: "error" }, available)).toBe("m8.unknown");
  expect(currentToolState("error", { status: "loading" })).toBe("m8.unknown");
  expect(currentToolState("ready", { status: "loading" }, available)).toBe("common.loading");
  expect(currentToolState("ready", { status: "success" })).toBe("tools.notReported");
});
it("projects fail-closed attention only from the current Runtime validity flag", () => {
  expect(hasPolicyCritical("ready", { status: "success", data: { effective: { valid: false } } })).toBe(true);
  expect(hasPolicyCritical("stopped", { status: "success", data: { effective: { valid: false } } })).toBe(false);
  expect(hasPolicyCritical("ready", { status: "error" })).toBe(false);
});
it("limits concrete mismatch explanations to facts reported by Runtime", () => {
  expect(enabledMismatch(false, available)).toBe("offStillAvailable");
  expect(enabledMismatch(true, { ...available, availability: "disabled" })).toBe("onStillDisabled");
  expect(enabledMismatch(true, { ...available, availability: "unavailable_by_mode" })).toBeNull();
  expect(enabledMismatch(false)).toBeNull();
});
