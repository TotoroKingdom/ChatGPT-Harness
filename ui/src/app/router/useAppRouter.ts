import { useState } from "react";
import type { PageKey, PolicyDraftHandoff } from "./routes";

export function useAppRouter() {
  const [page, setPage] = useState<PageKey>("overview");
  const [policyDraft, setPolicyDraft] = useState<PolicyDraftHandoff | null>(null);
  const navigate = (nextPage: PageKey, options?: { policyDraft?: PolicyDraftHandoff }) => {
    setPolicyDraft(options?.policyDraft ?? null);
    setPage(nextPage);
  };
  return { page, navigate, policyDraft };
}
