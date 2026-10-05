import { expect, it } from "vitest";
import { ConfigMutationQueue, configIntent, assertUnchanged } from "./config-mutations";
import type { AppConfig } from "@protocol/v1";

it("serializes operations, survives failure and preserves independent intents", async () => {
  const queue = new ConfigMutationQueue();
  const order: number[] = [];
  const first = queue.run(async () => { order.push(1); throw Error("write failed"); });
  const second = queue.run(async () => { order.push(2); return 2; });
  await expect(first).rejects.toThrow("write failed");
  await expect(second).resolves.toBe(2);
  expect(order).toEqual([1, 2]);
});

it("merges only changed leaves over the latest confirmed config", () => {
  const base = { ui: { language: "en" }, tools: { bash: { enabled: false } }, tunnel: { tunnelId: "old" } } as AppConfig;
  const intent = configIntent(base, { ...base, tunnel: { ...base.tunnel, tunnelId: "new" } });
  const latest = { ...base, ui: { language: "zh-CN" as const }, tools: { ...base.tools, bash: { enabled: true } } };
  expect(intent(latest)).toEqual({ ...latest, tunnel: { tunnelId: "new" } });
});
it("blocks explicit editors when the observed object has changed", () => {
  expect(() => assertUnchanged({ name: "draft base" }, { name: "external edit" })).toThrow();
  expect(() => assertUnchanged({ name: "same" }, { name: "same" })).not.toThrow();
});
