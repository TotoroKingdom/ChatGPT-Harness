import type { ReactNode } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import { Sidebar } from "./Sidebar";

export function AppShell({ page, onNavigate, children }: { page: PageKey; onNavigate: (page: PageKey) => void; children: ReactNode }) {
  const pageMeta = pages.find((item) => item.key === page) ?? pages[0];
  const data = useHarnessData();
  return <div className="app-shell"><Sidebar activePage={page} onNavigate={onNavigate} adapterKind={data.client.adapterKind} runtimeStatus={data.components.runtime} /><main className="main-area"><header className="topbar"><div><div className="eyebrow">{pageMeta.eyebrow}</div><h1>{pageMeta.label}</h1></div><div className="topbar-meta"><span className="secure-indicator"><span className="status-dot" />Local only</span><span className="topbar-separator" />Runtime data source</div></header><div className="page-content">{children}</div></main></div>;
}
