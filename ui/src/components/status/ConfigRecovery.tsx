import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useTranslation } from "react-i18next";
import { presentError } from "@/i18n/presentation";
import { Button, Disclosure, Section, TextField } from "@/components/common/primitives";

export function ConfigRecovery({ onReset, disabled }: { onReset: () => void; disabled: boolean }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const path = data.configMetadata?.configFile;
  const knownUnchanged = ["CONFIG_INVALID", "CONFIG_SCHEMA_UNSUPPORTED", "CONFIG_MIGRATION_FAILED"].includes(data.configError?.code ?? "");
  return <div className="page-stack">
    <div className="notice notice-danger" role="alert"><strong>{t("firstRun.configErrorTitle")}</strong><span>{t(presentError(data.configError).summaryKey)}</span></div>
    <p>{t("m8Recovery.summary")}</p>
    <p>{t(knownUnchanged ? "m7.recovery.unchanged" : "m8Recovery.unknownSafety")}</p>
    <p>{t("m7.recovery.repair")}</p>
    <Button variant="primary" onClick={() => void data.refreshConfig()} disabled={disabled}>{t("m7.recovery.retry")}</Button>
    <Disclosure title={t("m8Recovery.technical")}><code>{data.configError?.code ?? t("activity.notRecorded")}</code>
      <TextField label={t("m7.recovery.path")} readOnly value={path ?? t("activity.notRecorded")} onFocus={(event) => event.currentTarget.select()} />
      <Button variant="quiet" disabled={!path} onClick={() => { if (path) void navigator.clipboard?.writeText(path).catch(() => undefined); }}>{t("m7.recovery.copy")}</Button>
      <p>{t("m8Recovery.secretBoundary")}</p>
    </Disclosure>
    <Section title={t("m8Recovery.danger")}><Button variant="danger" onClick={onReset} disabled={disabled}>{t("firstRun.reset")}</Button></Section>
  </div>;
}
