import { useState } from "react";
import { HarnessDataProvider, useHarnessData } from "./app/providers/HarnessDataProvider";
import { useAppRouter } from "./app/router/useAppRouter";
import { AppShell } from "./components/layout/AppShell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { ToolsPage } from "./pages/tools/ToolsPage";
import { PoliciesPage } from "./pages/policies/PoliciesPage";
import { ActivityPage } from "./pages/activity/ActivityPage";
import { SettingsPage } from "./pages/settings/SettingsPage";
import "./styles/global.css";
function RoutedContent(){const d=useHarnessData();const{page,navigate}=useAppRouter();const[setupComplete,setSetupComplete]=useState(false);const configBlocked=d.configError?.code === "CONFIG_INVALID";if((d.setupRequired||configBlocked)&&!setupComplete)return <SettingsPage setup onDone={()=>{setSetupComplete(true);void d.refreshConfig();}}/>;const content=page==="overview"?<OverviewPage/>:page==="tools"?<ToolsPage/>:page==="policies"?<PoliciesPage/>:page==="activity"?<ActivityPage/>:<SettingsPage/>;return <AppShell page={page} onNavigate={navigate}>{content}</AppShell>;}
export default function App(){return <HarnessDataProvider><RoutedContent/></HarnessDataProvider>}