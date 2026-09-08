import type { ReactNode } from "react";
import type { ReadState } from "@/app/providers/useControlRead";
import { Card } from "@/components/common/primitives";
import { LoadingState } from "./AsyncStates";

export function ControlReadView<T>({ state, children }: { state: ReadState<T>; children: (data: T) => ReactNode }) {
  if (state.status === "loading") return <LoadingState label="Reading Runtime snapshot" />;
  if (state.status === "error") return <Card className="state-card"><strong role="alert">{state.message}</strong><span className="muted">No current snapshot is available. This is not an empty result.</span></Card>;
  return <>{children(state.data)}</>;
}
