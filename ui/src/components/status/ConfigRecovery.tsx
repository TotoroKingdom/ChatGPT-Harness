import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useTranslation } from "react-i18next";
import { presentError } from "@/i18n/presentation";

export function ConfigRecovery({ onReset, disabled }: { onReset: () => void; disabled: boolean }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const path = data.configMetadata?.configFile;
  return <div className="notice notice-danger" role="alert">
    <strong>{t("firstRun.configErrorTitle")}</strong>
    <span>{t(presentError(data.configError).summaryKey)}</span>
    <code>{data.configError?.code}</code>
    <span>{t("m7.recovery.unchanged")}</span><span>{t("m7.recovery.repair")}</span>
    <label>{t("m7.recovery.path")}<input className="path-input" aria-label={t("m7.recovery.path")} readOnly value={path ?? t("activity.notRecorded")} onFocus={(event) => event.currentTarget.select()} /></label>
    <button className="button button-secondary" onClick={() => void data.refreshConfig()} disabled={disabled}>{t("m7.recovery.retry")}</button>
    <button className="button button-secondary" disabled={!path} onClick={() => { if (path) void navigator.clipboard?.writeText(path).catch(() => undefined); }}>{t("m7.recovery.copy")}</button>
    <button className="button button-quiet" onClick={onReset} disabled={disabled}>{t("firstRun.reset")}</button>
  </div>;
}
