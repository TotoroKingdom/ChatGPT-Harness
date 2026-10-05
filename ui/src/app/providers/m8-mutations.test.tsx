import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "@protocol/v1";
import { HarnessClientError } from "@/services/harness/errors";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));

vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => state.client,
}));

import { HarnessDataProvider, useHarnessData } from "./HarnessDataProvider";

function configRequired(config: AppConfig | null): AppConfig {
  if (!config) throw new Error("The mutation probe requires a loaded configuration.");
  return config;
}

function MutationProbe() {
  const data = useHarnessData();
  const [result, setResult] = useState("");
  const [refreshState, setRefreshState] = useState("");

  const saveQuickFields = () => {
    const base = configRequired(data.config);
    void Promise.all([
      data.saveConfig({ ...base, tunnel: { ...base.tunnel, tunnelId: "quick-tunnel" } }),
      data.saveConfig({ ...base, workspace: { rootPath: "D:\\quick-workspace" } }),
    ]).then(([first, second]) => {
      setResult(`quick:${first.ok ? "ok" : "failed"},${second.ok ? "ok" : "failed"}`);
    });
  };

  const saveAfterFailure = () => {
    void Promise.all([
      data.mutateConfig((latest) => ({ ...latest, workspace: { rootPath: "D:\\failed-workspace" } })),
      data.mutateConfig((latest) => ({ ...latest, tunnel: { ...latest.tunnel, tunnelId: "after-failure" } })),
    ]).then(([first, second]) => {
      setResult(`failure:${first.ok ? "ok" : "failed"},${second.ok ? "ok" : "failed"}`);
    });
  };

  const saveRuleConnectionLanguage = () => {
    const rule: AppConfig["policies"]["userRules"][number] = {
      id: "m8-rule",
      name: "M8 rule",
      enabled: true,
      decision: "ask",
      matcher: { type: "tool", tool: "write" },
      reason: "M8 mutation coordination",
    };
    void Promise.all([
      data.mutateConfig((latest) => ({ ...latest, policies: { ...latest.policies, userRules: [rule] } })),
      data.setUiLanguage("zh-CN"),
      data.mutateConfig((latest) => ({ ...latest, tunnel: { ...latest.tunnel, tunnelId: "interleaved-connection" } })),
    ]).then(([ruleResult, languageResult, connectionResult]) => {
      setResult(`interleave:${ruleResult.ok ? "ok" : "failed"},${languageResult.ok ? "ok" : "failed"},${connectionResult.ok ? "ok" : "failed"}`);
    });
  };

  const saveAndReset = () => {
    void Promise.all([
      data.mutateConfig((latest) => ({ ...latest, tunnel: { ...latest.tunnel, tunnelId: "before-reset" } })),
      data.resetConfig(),
    ]).then(([save, reset]) => {
      setResult(`reset:${save.ok ? "save-ok" : "save-failed"},${reset.ok ? "reset-ok" : "reset-failed"}`);
    });
  };

  const saveWithRefreshFailure = () => {
    void data.mutateConfig((latest) => ({ ...latest, tunnel: { ...latest.tunnel, tunnelId: "refresh-unconfirmed" } })).then((next) => {
      setResult(`refresh-check:${next.ok ? "ok" : "failed"}${next.refreshUnconfirmed ? ":refresh-unconfirmed" : ""}`);
    });
  };

  const refreshThenSave = () => {
    setRefreshState("pending");
    void data.refreshConfig().then(() => setRefreshState("done"));
    void data.mutateConfig((latest) => ({ ...latest, tunnel: { ...latest.tunnel, tunnelId: "stale-guard" } })).then((next) => {
      setResult(`stale:${next.ok ? "ok" : "failed"}`);
    });
  };

  return (
    <>
      <output data-testid="config-state">{data.config ? "ready" : "empty"}</output>
      <output data-testid="config-tunnel">{data.config?.tunnel.tunnelId ?? ""}</output>
      <output data-testid="api-key-configured">{String(data.apiKeyConfigured)}</output>
      <output data-testid="api-key-ref">{data.config?.tunnel.apiKeyRef?.key ?? ""}</output>
      <output data-testid="harness-lifecycle">{data.harness.lifecycle}</output>
      <output data-testid="harness-session">{data.harness.activeSessionId ?? ""}</output>
      <output data-testid="mutation-result">{result}</output>
      <output data-testid="refresh-state">{refreshState}</output>
      <button type="button" onClick={saveQuickFields}>Save quick fields</button>
      <button type="button" onClick={saveAfterFailure}>Save after failure</button>
      <button type="button" onClick={saveRuleConnectionLanguage}>Save interleaved rule connection language</button>
      <button type="button" onClick={saveAndReset}>Save then reset</button>
      <button type="button" onClick={saveWithRefreshFailure}>Save with refresh failure</button>
      <button type="button" onClick={refreshThenSave}>Refresh then save</button>
      <button type="button" onClick={() => void data.deleteApiKey().then(() => setResult("delete:done"))}>Delete API Key</button>
    </>
  );
}

function renderMutationProbe() {
  render(
    <HarnessDataProvider>
      <MutationProbe />
    </HarnessDataProvider>,
  );
  return waitFor(() => expect(screen.getByTestId("config-state")).toHaveTextContent("ready"), { timeout: 5000 });
}

function ColdStartProbe({ initial }: { initial: AppConfig }) {
  const data = useHarnessData();
  const [result, setResult] = useState("");

  const runSeedAwareMutation = () => {
    void Promise.all([
      data.setUiLanguage("zh-CN"),
      data.mutateConfig(
        (latest) => ({ ...latest, workspace: { rootPath: "D:\\cold-workspace" } }),
        undefined,
        initial,
      ),
    ]).then(([language, workspace]) => {
      setResult(`cold:${language.ok ? "language-ok" : "language-failed"},${workspace.ok ? "workspace-ok" : "workspace-failed"}`);
    });
  };

  const runStaleFullSave = () => {
    void Promise.all([
      data.setUiLanguage("zh-CN"),
      data.saveConfig(initial),
    ]).then(([language, save]) => {
      setResult(`conflict:${language.ok ? "language-ok" : "language-failed"},${save.ok ? "save-ok" : `save-failed:${save.error?.code ?? "unknown"}`}`);
    });
  };

  return (
    <>
      <output data-testid="cold-result">{result}</output>
      <output data-testid="cold-config-state">{data.config ? "ready" : "empty"}</output>
      <button type="button" onClick={runSeedAwareMutation}>Run cold seed mutation</button>
      <button type="button" onClick={runStaleFullSave}>Run stale full save</button>
    </>
  );
}

async function prepareColdStartClient() {
  const initial = await state.client.configGet();
  if (!initial) throw new Error("Expected the mock client to start with a config.");
  await state.client.configReset();
  return initial;
}

describe("M8 config mutation coordination", () => {
  beforeEach(() => {
    state.client = new MockHarnessControlClient("ready");
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("preserves two independent quick saves made from the same UI snapshot", async () => {
    await renderMutationProbe();

    fireEvent.click(screen.getByRole("button", { name: "Save quick fields" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("quick:ok,ok"), { timeout: 5000 });

    const persisted = await state.client.configGet();
    expect(persisted?.tunnel.tunnelId).toBe("quick-tunnel");
    expect(persisted?.workspace.rootPath).toBe("D:\\quick-workspace");
  });

  it("continues with the next intent after the first queued save fails", async () => {
    const save = state.client.configSave.bind(state.client);
    const configSave = vi.spyOn(state.client, "configSave");
    configSave
      .mockRejectedValueOnce(new HarnessClientError("error", "first write failed", undefined, "CONFIG_WRITE_FAILED"))
      .mockImplementation((config, apiKey) => save(config, apiKey));
    await renderMutationProbe();

    fireEvent.click(screen.getByRole("button", { name: "Save after failure" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("failure:failed,ok"), { timeout: 5000 });

    const persisted = await state.client.configGet();
    expect(configSave).toHaveBeenCalledTimes(2);
    expect(persisted?.workspace.rootPath).toBe("D:\\example-project");
    expect(persisted?.tunnel.tunnelId).toBe("after-failure");
  });

  it("keeps Rule, Connection, and Language intents when they interleave", async () => {
    await renderMutationProbe();

    fireEvent.click(screen.getByRole("button", { name: "Save interleaved rule connection language" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("interleave:ok,ok,ok"), { timeout: 6000 });

    const persisted = await state.client.configGet();
    expect(persisted?.ui.language).toBe("zh-CN");
    expect(persisted?.tunnel.tunnelId).toBe("interleaved-connection");
    expect(persisted?.policies.userRules).toEqual([
      expect.objectContaining({ id: "m8-rule", name: "M8 rule", decision: "ask" }),
    ]);
  });

  it("reports a saved mutation when the follow-up refresh is unavailable without retrying", async () => {
    const read = state.client.configGet.bind(state.client);
    let persisted = false;
    const configGet = vi.spyOn(state.client, "configGet").mockImplementation(async () => {
      if (persisted) throw new HarnessClientError("disconnected", "refresh unavailable");
      return read();
    });
    const save = state.client.configSave.bind(state.client);
    const configSave = vi.spyOn(state.client, "configSave").mockImplementation(async (config, apiKey) => {
      const result = await save(config, apiKey);
      persisted = true;
      return result;
    });
    await renderMutationProbe();

    fireEvent.click(screen.getByRole("button", { name: "Save with refresh failure" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("refresh-check:ok:refresh-unconfirmed"), { timeout: 6000 });

    // The refresh failure is injected only after the config write; the saved
    // value remains authoritative and no extra configSave is attempted.
    expect(configSave).toHaveBeenCalledOnce();
    configGet.mockRestore();
    const confirmed = await state.client.configGet();
    expect(confirmed?.tunnel.tunnelId).toBe("refresh-unconfirmed");
    expect(confirmed?.workspace.rootPath).toBe("D:\\example-project");
  });

  it("does not let an older refreshConfig response overwrite a later confirmed mutation", async () => {
    await renderMutationProbe();

    const oldConfig = await state.client.configGet();
    const read = state.client.configGet.bind(state.client);
    const metadata = state.client.configMetadata.bind(state.client);
    const apiKeyStatus = state.client.apiKeyStatus.bind(state.client);
    const eligibility = state.client.harnessStartEligibility.bind(state.client);
    let configReads = 0;
    let eligibilityReads = 0;
    let releaseRefreshReads!: () => void;
    const refreshReads = new Promise<void>((resolve) => { releaseRefreshReads = resolve; });
    const configGet = vi.spyOn(state.client, "configGet").mockImplementation(async () => {
      configReads += 1;
      // The first read belongs to the explicitly triggered refresh. It is an
      // older persisted snapshot; the mutation's reads use the real client.
      return configReads === 1 ? oldConfig : read();
    });
    vi.spyOn(state.client, "configMetadata").mockImplementation(async () => {
      await refreshReads;
      return metadata();
    });
    vi.spyOn(state.client, "apiKeyStatus").mockImplementation(async () => {
      await refreshReads;
      return apiKeyStatus();
    });
    vi.spyOn(state.client, "harnessStartEligibility").mockImplementation(async () => {
      eligibilityReads += 1;
      if (eligibilityReads === 1) await refreshReads;
      return eligibility();
    });

    fireEvent.click(screen.getByRole("button", { name: "Refresh then save" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("stale:ok"), { timeout: 6000 });
    expect(screen.getByTestId("refresh-state")).toHaveTextContent("pending");
    expect(screen.getByTestId("config-state")).toHaveTextContent("ready");
    expect(screen.getByTestId("config-tunnel")).toHaveTextContent("stale-guard");
    expect(screen.getByTestId("mutation-result")).toHaveTextContent("stale:ok");

    // The later confirmed mutation is visible while the older refresh is
    // still waiting on independent reads.
    releaseRefreshReads();
    await waitFor(() => expect(screen.getByTestId("refresh-state")).toHaveTextContent("done"), { timeout: 6000 });
    expect(screen.getByTestId("config-tunnel")).toHaveTextContent("stale-guard");
    expect(screen.getByTestId("mutation-result")).toHaveTextContent("stale:ok");
    configGet.mockRestore();
    await expect(state.client.configGet()).resolves.toEqual(expect.objectContaining({
      tunnel: expect.objectContaining({ tunnelId: "stale-guard" }),
    }));
  });

  it("keeps the persisted credential reference after SecretStore deletion and preserves the managed session", async () => {
    await renderMutationProbe();
    await waitFor(() => expect(screen.getByTestId("api-key-configured")).toHaveTextContent("true"));
    const before = await state.client.configGet();
    const session = state.client.getHarnessStatus().activeSessionId;
    const apiKeyDelete = vi.spyOn(state.client, "apiKeyDelete").mockResolvedValue(undefined);
    vi.spyOn(state.client, "apiKeyStatus").mockResolvedValue(false);
    const harnessStop = vi.spyOn(state.client, "harnessStop");

    fireEvent.click(screen.getByRole("button", { name: "Delete API Key" }));
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("delete:done"), { timeout: 5000 });
    await waitFor(() => expect(screen.getByTestId("api-key-configured")).toHaveTextContent("false"));

    expect(apiKeyDelete).toHaveBeenCalledOnce();
    expect(screen.getByTestId("api-key-ref")).toHaveTextContent(before?.tunnel.apiKeyRef?.key ?? "");
    expect(screen.getByTestId("harness-lifecycle")).toHaveTextContent("ready");
    expect(screen.getByTestId("harness-session")).toHaveTextContent(session ?? "");
    expect(harnessStop).not.toHaveBeenCalled();
    await expect(state.client.configGet()).resolves.toEqual(expect.objectContaining({
      tunnel: expect.objectContaining({ apiKeyRef: before?.tunnel.apiKeyRef }),
    }));
  });

  it("uses a cold-start seed only when configGet is still null and preserves queued Language intent", async () => {
    const initial = await prepareColdStartClient();
    render(
      <HarnessDataProvider>
        <ColdStartProbe initial={initial} />
      </HarnessDataProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Run cold seed mutation" }));
    await waitFor(() => expect(screen.getByTestId("cold-result")).toHaveTextContent("cold:language-ok,workspace-ok"), { timeout: 6000 });

    const persisted = await state.client.configGet();
    expect(persisted?.ui.language).toBe("zh-CN");
    expect(persisted?.workspace.rootPath).toBe("D:\\cold-workspace");
    // The old full snapshot was only a fallback seed. It must not be replayed
    // after the queued Language mutation has created a real persisted config.
    expect(persisted?.tunnel.tunnelId).toBe("");
    expect(persisted?.tunnel.apiKeyRef).toBeUndefined();
  });

  it("rejects a queued stale full save with CONFIG_CONFLICT after Language creates the config", async () => {
    const initial = await prepareColdStartClient();
    render(
      <HarnessDataProvider>
        <ColdStartProbe initial={initial} />
      </HarnessDataProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Run stale full save" }));
    await waitFor(() => expect(screen.getByTestId("cold-result")).toHaveTextContent("conflict:language-ok,save-failed:CONFIG_CONFLICT"), { timeout: 6000 });

    const persisted = await state.client.configGet();
    expect(persisted?.ui.language).toBe("zh-CN");
    expect(persisted?.tunnel.tunnelId).toBe("");
    expect(persisted?.workspace.rootPath).toBe("");
  });

  it("queues Reset behind an in-flight config mutation", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const save = state.client.configSave.bind(state.client);
    let started = false;
    vi.spyOn(state.client, "configSave").mockImplementation(async (config, apiKey) => {
      started = true;
      await gate;
      return save(config, apiKey);
    });
    const reset = vi.spyOn(state.client, "configReset");
    await renderMutationProbe();

    fireEvent.click(screen.getByRole("button", { name: "Save then reset" }));
    await waitFor(() => expect(started).toBe(true));
    expect(reset).not.toHaveBeenCalled();

    release();
    await waitFor(() => expect(screen.getByTestId("mutation-result")).toHaveTextContent("reset:save-ok,reset-ok"), { timeout: 6000 });
    expect(reset).toHaveBeenCalledOnce();
    await expect(state.client.configGet()).resolves.toBeNull();
  });
});
