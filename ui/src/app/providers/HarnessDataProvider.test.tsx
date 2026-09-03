import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ActivityEvent } from "@protocol/v1";
import type { ActivityEventListener } from "@/services/harness/client";
import { MockHarnessControlClient } from "@/services/harness/mock-client";

let client: MockHarnessControlClient;
let activityListener: ActivityEventListener | undefined;
let unsubscribe: ReturnType<typeof vi.fn>;

vi.mock("@/services/harness/factory", () => ({
  createHarnessControlClient: () => client,
}));

import { HarnessDataProvider, useHarnessData } from "./HarnessDataProvider";
import { useActivitySubscription } from "./useActivitySubscription";

function activityEvent(): ActivityEvent {
  return {
    eventId: "event-1",
    sequence: "1",
    eventType: "runtime_started",
    timestamp: "2026-09-02T00:00:00Z",
    summary: "Runtime started.",
    metadata: {},
  };
}

function Probe() {
  const { activity } = useHarnessData();
  const subscriptionState = useActivitySubscription();
  const itemCount =
    activity.status === "success" || activity.status === "empty" ? activity.data.items.length : -1;

  return (
    <>
      <output data-testid="activity-count">{itemCount}</output>
      <output data-testid="subscription-state">{subscriptionState}</output>
    </>
  );
}

describe("HarnessDataProvider activity subscription", () => {
  beforeEach(() => {
    activityListener = undefined;
    unsubscribe = vi.fn();
    client = new MockHarnessControlClient("empty");
    vi.spyOn(client, "activityList").mockResolvedValue({ items: [], latestSequence: "0" });
    vi.spyOn(client, "activitySubscribe").mockImplementation(async (listener) => {
      activityListener = listener;
      return unsubscribe;
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  async function renderSubscribedProvider() {
    render(
      <HarnessDataProvider>
        <Probe />
      </HarnessDataProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("activity-count")).toHaveTextContent("0");
      expect(screen.getByTestId("subscription-state")).toHaveTextContent("subscribed");
    });
    expect(activityListener).toBeDefined();
  }

  it("activity_listener_does_not_resubscribe_on_append", async () => {
    await renderSubscribedProvider();

    act(() => activityListener?.(activityEvent()));

    await waitFor(() => expect(screen.getByTestId("activity-count")).toHaveTextContent("1"));
    expect(client.activitySubscribe).toHaveBeenCalledOnce();
  });

  it("duplicate_event_id_is_not_appended_twice", async () => {
    await renderSubscribedProvider();
    const event = activityEvent();

    act(() => activityListener?.(event));
    act(() => activityListener?.(event));

    await waitFor(() => expect(screen.getByTestId("activity-count")).toHaveTextContent("1"));
    expect(client.activitySubscribe).toHaveBeenCalledOnce();
  });
});
