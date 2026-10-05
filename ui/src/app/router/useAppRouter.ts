import { useState } from "react";
import type { PageKey, PolicyDraftHandoff, NavigationOptions } from "./routes";
import { useDraftNavigation } from "./DraftNavigation";

export function useAppRouter() {
  const { requestLeave } = useDraftNavigation();
  const [page, setPage] = useState<PageKey>("overview");
  const [policyDraft, setPolicyDraft] = useState<PolicyDraftHandoff | null>(null);
  const [context, setContext] = useState<NavigationOptions>({});
  const navigate = (nextPage: PageKey, options?: NavigationOptions) => {
    requestLeave(() => { setPolicyDraft(options?.policyDraft ?? null); setContext(options ?? {}); setPage(nextPage); });
  };
  return { page, navigate, policyDraft, context };
}
