import { useEffect, useState } from "react";
import { HarnessDataProvider, useHarnessData } from "./app/providers/HarnessDataProvider";
import { useAppRouter } from "./app/router/useAppRouter";
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
  const { page, navigate, policyDraft } = useAppRouter();
  const [setupComplete, setSetupComplete] = useState(false);
  const [setupActive, setSetupActive] = useState(false);
  const configBlocked = ["CONFIG_INVALID", "CONFIG_SCHEMA_UNSUPPORTED", "CONFIG_MIGRATION_FAILED"].includes(data.configError?.code ?? "");

  useEffect(() => {
    if (!setupComplete && (data.setupRequired || configBlocked)) setSetupActive(true);
  }, [configBlocked, data.setupRequired, setupComplete]);

  if (setupActive && !setupComplete) {
    return <FirstRunWizard onDone={() => {
      setSetupComplete(true);
      setSetupActive(false);
      void data.refreshConfig();
    }} />;
  }
  const content = page === "overview" ? <OverviewPage /> : page === "tools" ? <ToolsPage /> : page === "policies" ? <PoliciesPage initialDraft={policyDraft} /> : page === "activity" ? <ActivityPage /> : page === "approvals" ? <ApprovalsPage onNavigate={navigate} /> : <SettingsPage />;
  return <AppShell page={page} onNavigate={navigate}>{content}</AppShell>;
}

export default function App() {
  return <HarnessDataProvider><RoutedContent /></HarnessDataProvider>;
}


