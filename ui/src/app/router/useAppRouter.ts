import { useState } from "react";
import type { PageKey } from "./routes";

export function useAppRouter() {
  const [page, setPage] = useState<PageKey>("overview");
  return { page, navigate: setPage };
}
