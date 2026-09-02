import { describe, expect, it, vi } from "vitest";
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
