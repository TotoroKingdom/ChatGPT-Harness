import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SearchConfig } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { Button, InlineFeedback, Section, TextField } from "@/components/common/primitives";

const MIN_SECONDS = 5;
const MAX_SECONDS = 120;

function secondsValid(value: number | undefined): value is number {
  return typeof value === "number"
    && Number.isInteger(value)
    && value >= MIN_SECONDS
    && value <= MAX_SECONDS;
}

function valid(config: SearchConfig): boolean {
  return secondsValid(config.globTimeoutSeconds) && secondsValid(config.grepTimeoutSeconds);
}

export function SearchSettingsSection() {
  const { t } = useTranslation();
  const data = useHarnessData();
  const saved = data.config?.search;
  const [draft, setDraft] = useState<SearchConfig | null>(saved ?? null);
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [feedbackError, setFeedbackError] = useState(false);

  useEffect(() => setDraft(saved ?? null), [saved]);

  const save = async () => {
    if (!draft || !valid(draft) || saving) return;
    setSaving(true);
    setFeedback(null);
    setFeedbackError(false);
    try {
      const result = await data.mutateConfig((latest) => ({ ...latest, search: draft }));
      setFeedback(result.ok ? t("settings.search.saved") : t("settings.search.saveFailed"));
      setFeedbackError(!result.ok);
    } catch {
      setFeedback(t("settings.search.saveFailed"));
      setFeedbackError(true);
    } finally {
      setSaving(false);
    }
  };

  const update = (key: keyof SearchConfig, value: string) => {
    if (!draft) return;
    setDraft({ ...draft, [key]: Number(value) });
  };

  return <Section title={t("settings.search.title")}>
    <div className="settings-grid">
      <TextField
        label={t("settings.search.globTimeout")}
        type="number"
        min={MIN_SECONDS}
        max={MAX_SECONDS}
        value={draft?.globTimeoutSeconds ?? 30}
        disabled={!draft || saving}
        helper={t("settings.search.range")}
        onChange={(event) => update("globTimeoutSeconds", event.target.value)}
        error={draft && !secondsValid(draft.globTimeoutSeconds) ? t("settings.search.rangeError") : undefined}
      />
      <TextField
        label={t("settings.search.grepTimeout")}
        type="number"
        min={MIN_SECONDS}
        max={MAX_SECONDS}
        value={draft?.grepTimeoutSeconds ?? 30}
        disabled={!draft || saving}
        helper={t("settings.search.range")}
        onChange={(event) => update("grepTimeoutSeconds", event.target.value)}
        error={draft && !secondsValid(draft.grepTimeoutSeconds) ? t("settings.search.rangeError") : undefined}
      />
    </div>
    <InlineFeedback>{t("settings.search.restartHint")}</InlineFeedback>
    <div className="settings-actions">
      <Button onClick={() => void save()} disabled={!draft || !valid(draft) || saving}>
        {saving ? t("common.saving") : t("settings.search.save")}
      </Button>
    </div>
    {feedback && <InlineFeedback error={feedbackError}>{feedback}</InlineFeedback>}
  </Section>;
}
