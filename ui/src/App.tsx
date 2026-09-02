import { HarnessDataProvider } from "./app/providers/HarnessDataProvider";
import { useAppRouter } from "./app/router/useAppRouter";
import { AppShell } from "./components/layout/AppShell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { ToolsPage } from "./pages/tools/ToolsPage";
import { PoliciesPage } from "./pages/policies/PoliciesPage";
import { ActivityPage } from "./pages/activity/ActivityPage";
import { SettingsPage } from "./pages/settings/SettingsPage";
import "./styles/global.css";

function RoutedContent() {
  const { page, navigate } = useAppRouter();
  const content = page === "overview" ? <OverviewPage /> : page === "tools" ? <ToolsPage /> : page === "policies" ? <PoliciesPage /> : page === "activity" ? <ActivityPage /> : <SettingsPage />;
  return <AppShell page={page} onNavigate={navigate}>{content}</AppShell>;
}

export default function App() {
  return <HarnessDataProvider><RoutedContent /></HarnessDataProvider>;
}
