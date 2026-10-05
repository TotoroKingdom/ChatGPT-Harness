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
import "./styles/global.css";

function RoutedContent() {
  const data = useHarnessData();
  const { page, navigate, policyDraft, context } = useAppRouter();
  const [setupComplete, setSetupComplete] = useState(false);
  const [setupActive, setSetupActive] = useState(false);
  const [mainReached, setMainReached] = useState(false);
  const configBlocked = Boolean(data.configError);

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
  return <AppShell page={page} onNavigate={navigate}>{configBlocked && page !== "settings" && <RecoveryInWorkspace />}{content}</AppShell>;
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


