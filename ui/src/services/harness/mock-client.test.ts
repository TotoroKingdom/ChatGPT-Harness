import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

describe("MockHarnessControlClient", () => {
  it("returns M0-shaped normal-mode tools", async () => {
    const client = new MockHarnessControlClient();
    const result = await client.toolsList();
    expect(result.items.map((tool) => tool.name)).toEqual(["read_file", "glob", "bash"]);
    expect(result.items.filter((tool) => tool.exposed).map((tool) => tool.name)).toEqual(["read_file", "glob"]);
  });

  it("cleans up activity subscriptions", async () => {
    const client = new MockHarnessControlClient();
    const listener = vi.fn();
    const unsubscribe = await client.activitySubscribe(listener);
    unsubscribe();
    await new Promise((resolve) => window.setTimeout(resolve, 260));
    expect(listener).not.toHaveBeenCalled();
  });
});


const testConfig = (): AppConfig => ({
  schemaVersion: 1,
  configRevision: 0n,
  workspace: { rootPath: "D:\\workspace" },
  tunnel: { tunnelId: "tunnel", tunnelClientPath: "D:\\tunnel-client.exe", autoDetect: true, apiKeyRef: { provider: "memory-development", key: "tunnel-api-key" } },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
});

afterEach(() => {
  window.history.replaceState({}, "", "/");
});

describe("M2 mock configuration and lifecycle", () => {
  it("increments revision only when saved semantics change", async () => {
    const client = new MockHarnessControlClient("ready");
    const base = await client.configGet();
    const first = await client.configSave(testConfig());
    const same = await client.configSave({ ...first });
    const changed = await client.configSave({ ...same, tunnel: { ...same.tunnel, tunnelId: "next" } });
    expect(first.configRevision).toBe((base?.configRevision ?? 0n) + 1n);
    expect(same.configRevision).toBe(first.configRevision);
    expect(changed.configRevision).toBe(first.configRevision + 1n);
  });

  it("captures the saved revision on start and clears the active snapshot on stop", async () => {
    const client = new MockHarnessControlClient("ready");
    const config = await client.configSave(testConfig());
    const started = await client.runtimeStart();
    expect(started.configRevision).toBe(config.configRevision);
    expect(client.getComponentStates()).toEqual({ runtime: "ready", tunnel: "ready", mcp: "ready" });
    await client.runtimeStop();
    expect(client.getComponentStates()).toEqual({ runtime: "stopped", tunnel: "stopped", mcp: "stopped" });
    expect((await client.runtimeStatus()).readiness).toBe("not_ready");
  });
});
