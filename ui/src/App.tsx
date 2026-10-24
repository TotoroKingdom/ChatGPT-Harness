import { useEffect, useState } from "react";
import { HarnessDataProvider, useHarnessData } from "./app/providers/HarnessDataProvider";
import { useAppRouter } from "./app/router/useAppRouter";
import { DraftNavigationProvider } from "./app/router/DraftNavigation";
import { useDraftNavigation } from "./app/router/DraftNavigation";
import { ConfigRecovery } from "./components/status/ConfigRecovery";
import { Button, Dialog, InlineFeedback } from "./components/common/primitives";
import { useTranslation } from "react-i18next";
import { AppShell } from "./components/layout/AppShell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { ToolsPage } from "./pages/tools/ToolsPage";
import { PoliciesPage } from "./pages/policies/PoliciesPage";
import { ActivityPage } from "./pages/activity/ActivityPage";
import { ApprovalsPage } from "./pages/approvals/ApprovalsPage";
import { SettingsPage, FirstRunWizard } from "./pages/settings/SettingsPage";
import type { DesktopNavigationIntent } from "./services/harness/client";
import "./styles/global.css";

function RoutedContent() {
  const data = useHarnessData();
  const { page, navigate, policyDraft, context } = useAppRouter();
  const [setupComplete, setSetupComplete] = useState(false);
  const [setupActive, setSetupActive] = useState(false);
  const [mainReached, setMainReached] = useState(false);
  const [backgroundNoticeOpen, setBackgroundNoticeOpen] = useState(false);
  const configBlocked = Boolean(data.configError);

  useEffect(() => {
    let mounted = true;
    let stopNavigation: (() => void) | undefined;
    let stopBackgroundNotice: (() => void) | undefined;
    const handleNavigation = (intent: DesktopNavigationIntent) => {
      if (intent.page === "overview") navigate("overview");
      else if (intent.page === "approvals" || intent.page === "approvalsUnavailable") {
        navigate("approvals", { objectId: intent.objectId });
      }
    };
    void Promise.all([
      data.client.desktopNavigationSubscribe(handleNavigation),
      data.client.desktopBackgroundNoticeSubscribe(() => setBackgroundNoticeOpen(true)),
    ]).then(([navigation, notice]) => {
      if (!mounted) { navigation(); notice(); return; }
      stopNavigation = navigation;
      stopBackgroundNotice = notice;
    }).catch(() => undefined);
    return () => {
      mounted = false;
      stopNavigation?.();
      stopBackgroundNotice?.();
    };
  }, [data.client, navigate]);

  useEffect(() => {
    if (!setupActive && data.config && !data.setupRequired && !configBlocked) setMainReached(true);
    if (!setupComplete && ((data.setupRequired && !configBlocked) || (configBlocked && !mainReached))) setSetupActive(true);
  }, [configBlocked, data.config, data.setupRequired, mainReached, setupActive, setupComplete]);

  if ((setupActive || (configBlocked && !mainReached)) && !setupComplete) {
    return <FirstRunWizard onDone={() => {
      setSetupComplete(true);
      setSetupActive(false);
      void data.refreshConfig();
    }} />;
  }
  const content = page === "overview" ? <OverviewPage onNavigate={navigate} /> : page === "tools" ? <ToolsPage /> : page === "policies" ? <PoliciesPage initialDraft={policyDraft} initialRuleId={context.objectId} onNavigate={navigate} /> : page === "activity" ? <ActivityPage onNavigate={navigate} /> : page === "approvals" ? <ApprovalsPage onNavigate={navigate} initialRequestId={context.objectId} /> : <SettingsPage initialSection={context.settingsSection} />;
  return <>
    <AppShell page={page} onNavigate={navigate}>{configBlocked && page !== "settings" && <RecoveryInWorkspace />}{content}</AppShell>
    <BackgroundContinuityDialog open={backgroundNoticeOpen} onClose={() => setBackgroundNoticeOpen(false)} />
  </>;
}

function BackgroundContinuityDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!open) return null;
  const continueInBackground = async () => {
    if (saving) return;
    setSaving(true); setError(null);
    const result = await data.continueInBackground();
    if (result.ok) onClose();
    else setError(result.error?.message ?? t("desktop.backgroundNotice.failed"));
    setSaving(false);
  };
  const quit = async () => {
    if (saving) return;
    setSaving(true); setError(null);
    const result = await data.quitDesktop();
    if (!result.ok) { setError(result.error?.message ?? t("desktop.backgroundNotice.failed")); setSaving(false); }
  };
  return <Dialog title={t("desktop.backgroundNotice.title")} onClose={() => { if (!saving) onClose(); }}>
    <p>{t("desktop.backgroundNotice.closeHides")}</p>
    <p>{t("desktop.backgroundNotice.continuesRunning")}</p>
    <p className="muted">{t("desktop.backgroundNotice.separateActions")}</p>
    {error && <InlineFeedback error>{error}</InlineFeedback>}
    <div className="settings-actions">
      <Button variant="secondary" onClick={() => void quit()} disabled={saving}>{t("desktop.backgroundNotice.quit")}</Button>
      <Button variant="primary" onClick={() => void continueInBackground()} disabled={saving}>{t("desktop.backgroundNotice.continue")}</Button>
    </div>
  </Dialog>;
}

function RecoveryInWorkspace() {
  const data = useHarnessData();
  const { t } = useTranslation();
  const { requestLeave, finishDraft } = useDraftNavigation();
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  return <><ConfigRecovery disabled={saving} onReset={() => requestLeave(() => { setFailed(false); setOpen(true); })} />
    {open && <Dialog title={t("m8FirstRun.resetTitle")} onClose={() => { if (!saving) setOpen(false); }}><p>{t("m8FirstRun.resetLoss")}</p>{failed && <InlineFeedback error>{t("common.loadFailed")}</InlineFeedback>}<div className="settings-actions"><Button disabled={saving} onClick={() => setOpen(false)}>{t("common.cancel")}</Button><Button disabled={saving} variant="danger" onClick={() => { setSaving(true); void data.resetConfig().then((result) => { if (result.ok) { finishDraft(); setOpen(false); } else setFailed(true); }).finally(() => setSaving(false)); }}>{saving ? t("common.saving") : t("firstRun.reset")}</Button></div></Dialog>}
  </>;
}

export default function App() {
  return <HarnessDataProvider><DraftNavigationProvider><RoutedContent /></DraftNavigationProvider></HarnessDataProvider>;
}


