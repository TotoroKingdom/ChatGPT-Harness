import type { ReactNode } from "react";
import type { ReadState } from "@/app/providers/useControlRead";
import { Card } from "@/components/common/primitives";
import { LoadingState } from "./AsyncStates";
import { useTranslation } from "react-i18next";
import { errorCodeKey } from "@/i18n/presentation";

export function ControlReadView<T>({ state, children }: { state: ReadState<T>; children: (data: T) => ReactNode }) {
  const { t } = useTranslation();
  if (state.status === "loading") return <LoadingState label={t("common.readingSnapshot")} />;
  if (state.status === "error") return <Card className="state-card"><strong role="alert">{t(errorCodeKey(state.error.code))}</strong><span className="muted">{t("common.noSnapshot")}</span></Card>;
  return <>{children(state.data)}</>;
}
