import type { HarnessControlClient } from "./client";
import { createLocalClient } from "./local-client";
import { MockHarnessControlClient, type MockScenario } from "./mock-client";
function scenarioFromLocation(): MockScenario { if (typeof window === "undefined") return "ready"; const value = new URLSearchParams(window.location.search).get("mock"); return value === "disconnected" || value === "empty" || value === "error" || value === "ready" ? value : "ready"; }
export function createHarnessControlClient(): HarnessControlClient { const scenario = scenarioFromLocation(); if (scenario !== "ready") return new MockHarnessControlClient(scenario); return createLocalClient(); }