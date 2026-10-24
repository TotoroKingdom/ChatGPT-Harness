import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

describe("MockHarnessControlClient", () => {
  it("identifies the test adapter", () => {
    expect(new MockHarnessControlClient().adapterKind).toBe("mock");
  });
  it("returns the stable M5 catalog, including the mode-gated Bash row", async () => {
    const client = new MockHarnessControlClient();
    const result = await client.toolsList();
    expect(result.items.map((tool) => tool.name)).toEqual(["read_file", "glob", "write", "edit", "bash"]);
    expect(result.items.filter((tool) => tool.exposed).map((tool) => tool.name)).toEqual(["read_file", "glob", "write", "edit"]);
  });

  it("reports Bash mode, disabled, and Runtime observations separately", async () => {
    const client = new MockHarnessControlClient();
    await expect(client.runtimeGetEffectiveTools()).resolves.toEqual(expect.arrayContaining([
      expect.objectContaining({ name: "bash", availability: "unavailable_by_mode", readiness: "unavailable" }),
    ]));
    const config = await client.configGet();
    await client.configSave({ ...config!, security: { mode: "developer_test", developerTestMode: true }, tools: { ...config!.tools, bash: { enabled: true } } });
    await client.runtimeStop();
    await client.runtimeStart();
    await expect(client.runtimeGetEffectiveTools()).resolves.toEqual(expect.arrayContaining([
      expect.objectContaining({ name: "bash", availability: "unavailable_by_runtime", readiness: "unavailable" }),
    ]));
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
  schemaVersion: 3,
  configRevision: "0",
  workspace: { rootPath: "D:\\workspace" },
  tunnel: { tunnelId: "tunnel", tunnelClientPath: "D:\\tunnel-client-v0.0.13-windows-amd64", autoDetect: true, apiKeyRef: { provider: "memory-development", key: "tunnel-api-key" } },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
  tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
  bash: { autoDetect: true },
  policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
  desktop: { approvalNotifications: true, connectionNotifications: true, backgroundNoticeAcknowledged: false, autoStartHarnessOnAppLaunch: false },
  history: { enabled: true, retentionDays: 30, maxStorageMiB: 256 },
});

afterEach(() => {
  window.history.replaceState({}, "", "/");
});

describe("M2 mock configuration and lifecycle", () => {
  it("increments revision only when saved semantics change", async () => {
    const client = new MockHarnessControlClient("ready");
    await client.configReset();
    const base = await client.configGet();
    const first = await client.configSave(testConfig());
    const same = await client.configSave({ ...first });
    const changed = await client.configSave({ ...same, tunnel: { ...same.tunnel, tunnelId: "next" } });
    expect(first.configRevision).toBe("1");
    expect(same.configRevision).toBe(first.configRevision);
    expect(changed.configRevision).toBe("2");
  });

  it("keeps the revision stable for language-only saves and preserves unrelated fields", async () => {
    const client = new MockHarnessControlClient("ready");
    const first = await client.configSave(testConfig());
    const saved = await client.uiLanguageSet("zh-CN");

    expect(saved.ui.language).toBe("zh-CN");
    expect(saved.configRevision).toBe(first.configRevision);
    expect(saved.workspace).toEqual(first.workspace);
    expect(saved.tunnel).toEqual(first.tunnel);
    expect(saved.tools).toEqual(first.tools);

    const cleared = await client.uiLanguageSet(null);
    expect(cleared.ui.language).toBeUndefined();
    expect(cleared.configRevision).toBe(first.configRevision);
  });

  it("returns the tunnel-client package directory from auto detect", async () => {
    const client = new MockHarnessControlClient("ready");
    await expect(client.detectTunnelClient()).resolves.toBe(
      "D:\\example-project\\tunnel-client-v0.0.13-windows-amd64",
    );
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


it("serializes config, runtime, and activity wire models without bigint", async () => {
  const client = new MockHarnessControlClient("ready");
  const config = await client.configSave(testConfig());
  expect(() => JSON.stringify(config)).not.toThrow();
  const loaded = await client.configGet();
  expect(() => JSON.stringify(loaded)).not.toThrow();

  const runtime = await client.runtimeStart();
  expect(() => JSON.stringify(runtime)).not.toThrow();
  expect(runtime.configRevision).toBe(config.configRevision);

  const activity = await client.activityList();
  expect(() => JSON.stringify(activity)).not.toThrow();
  expect(activity.latestSequence).toBeTypeOf("string");
  const listener = vi.fn();
  await client.activitySubscribe(listener);
  await new Promise((resolve) => window.setTimeout(resolve, 260));
  expect(listener).toHaveBeenCalled();
  expect(() => JSON.stringify(listener.mock.calls[0][0])).not.toThrow();
});
