import { useCallback, useState } from "react";
import type { PageKey, PolicyDraftHandoff, NavigationOptions } from "./routes";
import { useDraftNavigation } from "./DraftNavigation";

export function useAppRouter() {
  const { requestLeave } = useDraftNavigation();
  const [page, setPage] = useState<PageKey>("overview");
  const [policyDraft, setPolicyDraft] = useState<PolicyDraftHandoff | null>(null);
  const [context, setContext] = useState<NavigationOptions>({});
  const navigate = useCallback((nextPage: PageKey, options?: NavigationOptions) => {
    requestLeave(() => { setPolicyDraft(options?.policyDraft ?? null); setContext(options ?? {}); setPage(nextPage); });
  }, [requestLeave]);
  return { page, navigate, policyDraft, context };
}
