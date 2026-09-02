import { useEffect, useState } from "react";
import { useHarnessData } from "./HarnessDataProvider";

export function useActivitySubscription() {
  const { subscribeToActivity, appendActivity } = useHarnessData();
  const [subscriptionState, setSubscriptionState] = useState<"idle" | "subscribed" | "error">("idle");

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | undefined;

    void subscribeToActivity((event) => {
      if (mounted) appendActivity(event);
    })
      .then((cleanup) => {
        if (!mounted) {
          cleanup();
          return;
        }
        unsubscribe = cleanup;
        setSubscriptionState("subscribed");
      })
      .catch(() => {
        if (mounted) setSubscriptionState("error");
      });

    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, [appendActivity, subscribeToActivity]);

  return subscriptionState;
}
