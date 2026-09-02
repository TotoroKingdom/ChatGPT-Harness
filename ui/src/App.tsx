import { useState } from "react";
import { HarnessDataProvider, useHarnessData } from "./app/providers/HarnessDataProvider";
import { useAppRouter } from "./app/router/useAppRouter";
import { AppShell } from "./components/layout/AppShell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { ToolsPage } from "./pages/tools/ToolsPage";
import { PoliciesPage } from "./pages/policies/PoliciesPage";
import { ActivityPage } from "./pages/activity/ActivityPage";
import { SettingsPage, FirstRunWizard } from "./pages/settings/SettingsPage";
import "./styles/global.css";

function RoutedContent() {
  const data = useHarnessData();
  const { page, navigate } = useAppRouter();
  const [setupComplete, setSetupComplete] = useState(false);
  const configBlocked = data.configError?.code === "CONFIG_INVALID";
  const setupRequested = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("setup") === "first-run";
  if ((data.setupRequired || configBlocked || setupRequested) && !setupComplete) {
    return <FirstRunWizard onDone={() => { setSetupComplete(true); void data.refreshConfig(); }} />;
  }
  const content = page === "overview" ? <OverviewPage /> : page === "tools" ? <ToolsPage /> : page === "policies" ? <PoliciesPage /> : page === "activity" ? <ActivityPage /> : <SettingsPage />;
  return <AppShell page={page} onNavigate={navigate}>{content}</AppShell>;
}

export default function App() {
  return <HarnessDataProvider><RoutedContent /></HarnessDataProvider>;
}


