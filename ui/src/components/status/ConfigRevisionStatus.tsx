import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useTranslation } from "react-i18next";

export function ConfigRevisionStatus({ descriptionKey = "overview.configChanged", titleKey = "overview.restartRequired", onRestart }: { descriptionKey?: string; titleKey?: string; onRestart?: () => void }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  if (!data.config) return null;
  const running = data.components ? data.components.runtime === "ready" || data.components.runtime === "degraded" : data.restartRequired;
  const active = running ? data.activeRevision ?? ((data.runtime?.status === "success" || data.runtime?.status === "empty") ? data.runtime.data.configRevision : null) : null;
  return <section aria-label={t("m7.revision.title")}>
    <p className="muted">{t("m7.revision.saved")}: <code>{data.config.configRevision}</code> · {t("m7.revision.active")}: <code>{active ?? t(running ? "activity.notRecorded" : "m7.revision.stopped")}</code></p>
    {running && data.restartRequired && <div className="notice notice-warning" role="status">
      <strong>{t(titleKey)}</strong><span>{t(descriptionKey)}</span>
      {onRestart && <button className="button button-warning" onClick={onRestart}>{t("overview.restart")}</button>}
    </div>}
  </section>;
}
