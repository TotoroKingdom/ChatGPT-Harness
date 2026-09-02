import type { HarnessControlClient } from "./client";
import { createLocalClient } from "./local-client";
import { MockHarnessControlClient, type MockScenario } from "./mock-client";

function mockScenarioFromLocation(): MockScenario | null {
  if (typeof window === "undefined") return null;
  const value = new URLSearchParams(window.location.search).get("mock");
  if (value === null || value === "ready") return value === "ready" ? "ready" : null;
  return value === "disconnected" || value === "empty" || value === "error" ? value : "ready";
}

export function createHarnessControlClient(): HarnessControlClient {
  const scenario = mockScenarioFromLocation();
  return scenario ? new MockHarnessControlClient(scenario) : createLocalClient();
}
