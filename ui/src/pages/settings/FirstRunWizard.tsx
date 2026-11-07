import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppConfig } from "@protocol/v1";
import { useHarnessData, type SaveResult } from "@/app/providers/HarnessDataProvider";
import { assertUnchanged } from "@/app/providers/config-mutations";
import { useDraftGuard, useDraftNavigation } from "@/app/router/DraftNavigation";
import { Button, Dialog, Disclosure, InlineFeedback, Section, TextField } from "@/components/common/primitives";
import { ConfigRecovery } from "@/components/status/ConfigRecovery";
import { presentError, startEligibilityKey } from "@/i18n/presentation";
import { initialConfig } from "./configuration";

type Step = "workspace" | "connection" | "readiness" | "start";
type Feedback = { key: string; error?: boolean; detail?: string };
export function FirstRunWizard({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [step, setStep] = useState<Step>(data.config?.workspace.rootPath ? "connection" : "workspace");
  const [draft, setDraft] = useState<AppConfig["tunnel"]>(data.config?.tunnel ?? initialConfig.tunnel);
  const [baseline, setBaseline] = useState(draft);
  const [key, setKey] = useState("");
  const [replacementFailed, setReplacementFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [resetOpen, setResetOpen] = useState(false);
  const [startRequested, setStartRequested] = useState(false);
  const dirty = step === "connection" && (JSON.stringify(draft) !== JSON.stringify(baseline) || key.length > 0);
  const conflict = step === "connection" && data.config && JSON.stringify(data.config.tunnel) !== JSON.stringify(baseline);
  const discard = () => { setKey(""); setReplacementFailed(false); const fresh = data.config?.tunnel ?? initialConfig.tunnel; setDraft(fresh); setBaseline(fresh); };
  const leave = useDraftGuard({ dirty, saving: busy, onDiscard: discard });
  const { finishDraft } = useDraftNavigation();

  useEffect(() => {
    if (step === "connection" && !dirty && !busy && data.config) { setDraft(data.config.tunnel); setBaseline(data.config.tunnel); }
  }, [data.config, step, dirty, busy]);
  useEffect(() => { if (data.configError) setKey(""); }, [data.configError]);
  useEffect(() => {
    if (startRequested && data.harnessObserved && data.overall === "ready") onDone();
  }, [data.harnessObserved, data.overall, onDone, startRequested]);

  const report = (result: SaveResult) => {
    if (result.ok) setFeedback({ key: result.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved" });
    else { const error = presentError(result.error); setFeedback({ key: error.summaryKey, error: true }); }
  };
  const chooseWorkspace = async () => {
    if (lock.current || data.configError) return;
    lock.current = true; setBusy(true); setFeedback(null);
    try {
      const path = await data.chooseWorkspace();
      if (!path) return;
      const result = await data.mutateConfig((latest) => ({ ...latest, workspace: { rootPath: path } }), undefined, initialConfig);
      report(result);
      if (result.ok) { finishDraft(); setStep("connection"); }
    } catch (error) { setFeedback({ key: presentError(error).summaryKey, error: true }); }
    finally { lock.current = false; setBusy(false); }
  };
  const applyConnection = async () => {
    if (lock.current || conflict || !data.config) return;
    if (!draft.tunnelId.trim()) { setFeedback({ key: "m8FirstRun.tunnelRequired", error: true }); return; }
    if ((!data.apiKeyObserved || !data.apiKeyConfigured || replacementFailed) && !key.trim()) { setFeedback({ key: "m8FirstRun.keyRequired", error: true }); return; }
    lock.current = true; setBusy(true); setFeedback(null);
    const secret = key.trim() || undefined;
    setKey("");
    try {
      const section = { ...draft, tunnelId: draft.tunnelId.trim() };
      const result = await data.mutateConfig((latest) => { assertUnchanged(baseline, latest.tunnel); return { ...latest, tunnel: section }; }, secret);
      report(result);
      if (result.ok) { finishDraft(); setReplacementFailed(false); setBaseline(section); setDraft(section); setStep("readiness"); }
      else { if (secret) setReplacementFailed(true); setFeedback({ key: "m8FirstRun.connectionUnconfirmed", error: true }); }
    } catch (error) { if (secret) setReplacementFailed(true); setFeedback({ key: "m8FirstRun.connectionUnconfirmed", error: true }); }
    finally { lock.current = false; setBusy(false); setKey(""); }
  };
  const start = async () => {
    if (lock.current || !data.eligibility?.canStart) return;
    lock.current = true; setBusy(true); setFeedback(null);
    try {
      const result = await data.startHarness();
      report(result);
      if (result.ok) setStartRequested(true);
    } catch (error) { setFeedback({ key: presentError(error).summaryKey, error: true }); }
    finally { lock.current = false; setBusy(false); }
  };
  const reset = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try {
      const result = await data.resetConfig(); report(result);
      if (result.ok) { finishDraft(); discard(); setStep("workspace"); setResetOpen(false); setStartRequested(false); }
    } finally { lock.current = false; setBusy(false); }
  };
  const stop = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try { report(await data.stopHarness()); } finally { lock.current = false; setBusy(false); }
  };
  const go = (next: Step) => leave(() => { setStep(next); setFeedback(null); });
  const hasWorkspace = Boolean(data.config?.workspace.rootPath);
  const prepared = hasWorkspace && Boolean(data.config?.tunnel.tunnelId) && data.apiKeyObserved && data.apiKeyConfigured;
  const steps: Step[] = ["workspace", "connection", "readiness", "start"];
  return <div className="onboarding-shell">
    <div className="wizard-content">
      <aside className="onboarding-aside">
        <div className="onboarding-brand"><div className="brand-mark" aria-hidden="true"><span>H</span></div><span>{t("app.desktopWorkspace")}</span></div>
        <div><h1>ChatGPT Harness</h1><p>{t("m8FirstRun.title")}</p></div>
        <nav className="setup-progress" aria-label={t("m8FirstRun.steps")}>{steps.map((item, index) => <span key={item} aria-current={step === item ? "step" : undefined}>{index + 1}. {t(`m8FirstRun.${item}`)}</span>)}</nav>
      </aside>
      <main className="onboarding-main">
        {data.configError ? <ConfigRecovery onReset={() => leave(() => setResetOpen(true))} disabled={busy} /> : <div className="wizard-step">
          {step === "workspace" && <Section title={t("m8FirstRun.workspace")}><p>{t("m8FirstRun.workspaceHelp")}</p><TextField label={t("settings.workspace.root")} value={data.config?.workspace.rootPath ?? ""} readOnly />
            <div className="wizard-actions"><Button variant={hasWorkspace ? "secondary" : "primary"} onClick={() => void chooseWorkspace()} disabled={busy}>{t("firstRun.chooseFolder")}</Button><Button variant="primary" disabled={busy || !hasWorkspace} onClick={() => go("connection")}>{t("m8FirstRun.continue")}</Button></div>
          </Section>}
          {step === "connection" && <Section title={t("m8FirstRun.connection")}><p>{t("m8FirstRun.connectionHelp")}</p>
            <TextField label={t("firstRun.tunnelId")} value={draft.tunnelId} disabled={busy} onChange={(event) => setDraft((current) => ({ ...current, tunnelId: event.target.value }))} />
            <TextField label={t("firstRun.apiKey")} type="password" value={key} autoComplete="off" disabled={busy} onChange={(event) => setKey(event.target.value)} helper={t(!data.apiKeyObserved ? "m8.unknown" : data.apiKeyConfigured ? "m8FirstRun.keyConfigured" : "m8FirstRun.keyRequired")} />
            {conflict && <InlineFeedback error>{t("m8.conflict")} <Button disabled={busy} variant="quiet" onClick={() => leave(discard)}>{t("m8.reload")}</Button></InlineFeedback>}
            <div className="wizard-actions"><Button disabled={busy} onClick={() => go("workspace")}>{t("m8FirstRun.back")}</Button><Button variant="primary" disabled={busy || Boolean(conflict) || !hasWorkspace} onClick={() => void applyConnection()}>{busy ? t("common.saving") : t("m8FirstRun.applyContinue")}</Button></div>
          </Section>}
          {step === "readiness" && <Section title={t("m8FirstRun.readiness")}><p>{t(prepared ? "m8FirstRun.prepared" : "m8FirstRun.notPrepared")}</p><p>{data.eligibility ? t(startEligibilityKey(data.eligibility.status)) : t("m8.unknown")}</p>
            <div className="wizard-actions"><Button disabled={busy} onClick={() => go("connection")}>{t("m8FirstRun.reviewConnection")}</Button><Button disabled={busy} onClick={() => void data.refreshConfig()}>{t("common.refresh")}</Button><Button variant="primary" disabled={busy || !prepared} onClick={() => go("start")}>{t("m8FirstRun.continue")}</Button></div>
          </Section>}
          {step === "start" && <Section title={t("m8FirstRun.start")}><p>{t("m8FirstRun.startHelp")}</p><p>{data.harnessObserved ? t(`status.${data.overall}`) : t("m8.unknown")}</p>
            {data.eligibility && !data.eligibility.canStart && <InlineFeedback error>{t(startEligibilityKey(data.eligibility.status))}</InlineFeedback>}
            <div className="wizard-actions"><Button disabled={busy} onClick={() => go("connection")}>{t("m8FirstRun.reviewConnection")}</Button>{data.harness.activeSessionId && <Button disabled={busy} onClick={() => void stop()}>{t("overview.stop")}</Button>}<Button variant="primary" disabled={busy || !data.eligibility?.canStart} onClick={() => void start()}>{busy ? t("status.starting") : t("firstRun.start")}</Button></div>
          </Section>}
          <Disclosure title={t("m8.advanced")}><p>{t("m8FirstRun.existingSession")}</p><code>{data.harness.activeSessionId ?? t("activity.notRecorded")}</code></Disclosure>
        </div>}
        {feedback && <InlineFeedback error={feedback.error}>{t(feedback.key)}{feedback.detail}</InlineFeedback>}
      </main>
    </div>
    {resetOpen && <Dialog title={t("m8FirstRun.resetTitle")} onClose={() => { if (!busy) setResetOpen(false); }}><p>{t("m8FirstRun.resetLoss")}</p>{feedback?.error && <InlineFeedback error>{t(feedback.key)}</InlineFeedback>}<div className="settings-actions"><Button disabled={busy} onClick={() => setResetOpen(false)}>{t("common.cancel")}</Button><Button variant="danger" disabled={busy} onClick={() => void reset()}>{busy ? t("common.saving") : t("firstRun.reset")}</Button></div></Dialog>}
  </div>;
}
