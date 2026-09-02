import type { ReactNode } from "react";
import type { AsyncState } from "@/app/providers/HarnessDataProvider";
import { Card } from "@/components/common/primitives";

export function LoadingState({ label = "Loading data" }: { label?: string }) {
  return <Card className="state-card"><div className="loader" aria-hidden="true" /><strong>{label}</strong><span className="muted">Fetching the latest Runtime view.</span></Card>;
}
export function EmptyState({ title = "Nothing here yet", description = "No records were returned by the Runtime." }: { title?: string; description?: string }) {
  return <Card className="state-card"><div className="state-icon">—</div><strong>{title}</strong><span className="muted">{description}</span></Card>;
}
export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return <Card className="state-card state-danger"><div className="state-icon">!</div><strong>Couldn’t load this view</strong><span className="muted">{message}</span>{onRetry && <button className="button button-secondary" onClick={onRetry}>Try again</button>}</Card>;
}
export function DisconnectedState() {
  return <Card className="state-card state-disconnected"><div className="state-icon">○</div><strong>Runtime not connected</strong><span className="muted">The local Runtime is not running. Runtime controls will be available in the next milestone.</span></Card>;
}
export function AsyncStateView<T>({ state, children, empty }: { state: AsyncState<T>; children: (data: T) => ReactNode; empty?: (data: T) => ReactNode }) {
  switch (state.status) {
    case "idle":
    case "loading":
      return <LoadingState />;
    case "disconnected":
      return <DisconnectedState />;
    case "error":
      return <ErrorState message={state.error.message} />;
    case "empty":
      return empty ? empty(state.data) : <EmptyState />;
    case "success":
      return <>{children(state.data)}</>;
  }
}
