import type { ReactNode } from "react";
import type { AsyncState } from "@/app/providers/HarnessDataProvider";
import { Card } from "@/components/common/primitives";
import { useTranslation } from "react-i18next";
import { presentError } from "@/i18n/presentation";

export function LoadingState({ label }: { label?: string }) {
  const { t } = useTranslation();
  return <Card className="state-card"><div className="loader" aria-hidden="true" /><strong>{label ?? t("common.loading")}</strong><span className="muted">{t("common.fetching")}</span></Card>;
}
export function EmptyState({ title, description }: { title?: string; description?: string }) {
  const { t } = useTranslation();
  return <Card className="state-card"><div className="state-icon">—</div><strong>{title ?? t("common.emptyTitle")}</strong><span className="muted">{description ?? t("common.emptyDescription")}</span></Card>;
}
export function ErrorState({ code, onRetry }: { message: string; code?: string; onRetry?: () => void }) {
  const { t } = useTranslation();
  const error = presentError({ code });
  return <Card className="state-card state-danger"><div className="state-icon">!</div><strong>{t("common.loadFailed")}</strong><span className="muted">{t(error.summaryKey)}</span>{error.code && <span className="muted mono">{error.code}</span>}{onRetry && <button className="button button-secondary" onClick={onRetry}>{t("common.retry")}</button>}</Card>;
}
export function DisconnectedState() {
  const { t } = useTranslation();
  return <Card className="state-card state-disconnected"><div className="state-icon">○</div><strong>{t("common.disconnectedTitle")}</strong><span className="muted">{t("common.disconnectedDescription")}</span></Card>;
}
export function AsyncStateView<T>({ state, children, empty }: { state: AsyncState<T>; children: (data: T) => ReactNode; empty?: (data: T) => ReactNode }) {
  switch (state.status) {
    case "idle":
    case "loading":
      return <LoadingState />;
    case "disconnected":
      return <DisconnectedState />;
    case "error":
      return <ErrorState message={state.error.message} code={state.error.code} />;
    case "empty":
      return empty ? empty(state.data) : <EmptyState />;
    case "success":
      return <>{children(state.data)}</>;
  }
}
