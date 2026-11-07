import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { EvidenceStoreStatus, HistoryClearResult, HistoryConfig } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import {
  Button,
  Dialog,
  InlineFeedback,
  Row,
  Section,
  StatusBadge,
  Switch,
  TextField,
} from "@/components/common/primitives";

function valid(config: HistoryConfig): boolean {
  return Number.isInteger(config.retentionDays)
    && config.retentionDays >= 1
    && config.retentionDays <= 3650
    && Number.isInteger(config.maxStorageMiB)
    && config.maxStorageMiB >= 32
    && config.maxStorageMiB <= 4096;
}

export function HistorySettingsSection() {
  const { t } = useTranslation();
  const data = useHarnessData();
  const saved = data.config?.history;
  const [draft, setDraft] = useState<HistoryConfig | null>(saved ?? null);
  const [status, setStatus] = useState<EvidenceStoreStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [feedbackError, setFeedbackError] = useState(false);
  const [clearFeedback, setClearFeedback] = useState<HistoryClearResult | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  useEffect(() => setDraft(saved ?? null), [saved]);

  const refreshStatus = async () => {
    try {
      setStatus(await data.client.historyStatus());
    } catch {
      setStatus(null);
    }
  };

  useEffect(() => {
    void refreshStatus();
  }, [data.client]);

  const save = async () => {
    if (!draft || !valid(draft) || saving) return;
    setSaving(true);
    setFeedback(null);
    setFeedbackError(false);
    try {
      const result = await data.mutateConfig((latest) => ({ ...latest, history: draft }));
      setFeedback(result.ok ? t("m03History.settings.saved") : t("m03History.settings.saveFailed"));
      setFeedbackError(!result.ok);
      await refreshStatus();
    } catch {
      setFeedback(t("m03History.settings.saveFailed"));
      setFeedbackError(true);
    } finally {
      setSaving(false);
    }
  };

  const clear = async () => {
    if (clearing) return;
    setClearing(true);
    setFeedback(null);
    setFeedbackError(false);
    try {
      const result = await data.client.historyClear();
      setClearFeedback(result);
      setConfirmClear(false);
      await refreshStatus();
    } catch (error) {
      setFeedback(typeof error === "object" && error !== null && "code" in error
        ? String(error.code)
        : t("m03History.settings.clearFailed"));
      setFeedbackError(true);
    } finally {
      setClearing(false);
    }
  };

  return <Section title={t("m03History.settings.title")}>
    <Row
      title={t("m03History.settings.enabled")}
      summary={t("m03History.settings.enabledHelp")}
      controls={<Switch
        label={t("m03History.settings.enabled")}
        checked={draft?.enabled ?? true}
        disabled={!draft || saving}
        onChange={(enabled) => draft && setDraft({ ...draft, enabled })}
      />}
    />
    <div className="settings-grid">
      <TextField
        label={t("m03History.settings.retentionDays")}
        type="number"
        min={1}
        max={3650}
        value={draft?.retentionDays ?? 30}
        disabled={!draft || saving}
        onChange={(event) => draft && setDraft({ ...draft, retentionDays: Number(event.target.value) })}
        error={draft && (draft.retentionDays < 1 || draft.retentionDays > 3650) ? t("m03History.settings.retentionError") : undefined}
      />
      <TextField
        label={t("m03History.settings.maxStorageMiB")}
        type="number"
        min={32}
        max={4096}
        value={draft?.maxStorageMiB ?? 256}
        disabled={!draft || saving}
        onChange={(event) => draft && setDraft({ ...draft, maxStorageMiB: Number(event.target.value) })}
        error={draft && (draft.maxStorageMiB < 32 || draft.maxStorageMiB > 4096) ? t("m03History.settings.storageError") : undefined}
      />
    </div>
    <div className="settings-actions">
      <Button onClick={() => void save()} disabled={!draft || !valid(draft) || saving}>{saving ? t("m03History.settings.saving") : t("m03History.settings.save")}</Button>
      <Button variant="danger" onClick={() => setConfirmClear(true)} disabled={clearing}>{t("m03History.settings.clear")}</Button>
    </div>
    <Row
      title={t("m03History.settings.effective")}
      summary={<StatusBadge tone={status?.state === "healthy" ? "success" : status?.state === "disabled" ? "neutral" : "warning"}>{status ? t(`m03History.storeState.${status.state}`) : t("m8.unknown")}</StatusBadge>}
      controls={<span className="muted">{status?.footprintBytes === undefined ? t("m03History.notRecorded") : t("m03History.settings.footprint", { bytes: status.footprintBytes })}</span>}
    />
    {data.restartRequired && <InlineFeedback>{t("m03History.settings.restartRequired")}</InlineFeedback>}
    <InlineFeedback>{t("m03History.settings.privacy")}</InlineFeedback>
    {feedback && <InlineFeedback error={feedbackError}>{feedback}</InlineFeedback>}
    {clearFeedback && <div className="notice notice-success" role="status">{t("m03History.settings.cleared", {
      requests: clearFeedback.deletedRequestCount,
      events: clearFeedback.deletedEventCount,
      approvals: clearFeedback.deletedApprovalCount,
    })}</div>}
    {confirmClear && <Dialog title={t("m03History.settings.clearTitle")} onClose={() => !clearing && setConfirmClear(false)}>
      <p>{t("m03History.settings.clearBody")}</p>
      <div className="settings-actions">
        <Button variant="secondary" onClick={() => setConfirmClear(false)} disabled={clearing}>{t("common.cancel")}</Button>
        <Button variant="danger" onClick={() => void clear()} disabled={clearing}>{clearing ? t("m03History.settings.clearing") : t("m03History.settings.clearConfirm")}</Button>
      </div>
    </Dialog>}
  </Section>;
}
