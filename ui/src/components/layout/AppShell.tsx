import type { ReactNode } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import { Sidebar } from "./Sidebar";
import { useTranslation } from "react-i18next";

export function AppShell({ page, onNavigate, children }: { page: PageKey; onNavigate: (page: PageKey) => void; children: ReactNode }) {
  const { t } = useTranslation();
  const pageMeta = pages.find((item) => item.key === page) ?? pages[0];
  const data = useHarnessData();
  return <div className="app-shell"><Sidebar activePage={page} onNavigate={onNavigate} adapterKind={data.client.adapterKind} runtimeStatus={data.components.runtime} /><main className="main-area"><header className="topbar"><div><div className="eyebrow">{t(pageMeta.eyebrow)}</div><h1>{t(pageMeta.label)}</h1></div><div className="topbar-meta"><span className="secure-indicator"><span className="status-dot" />{t("app.localOnly")}</span><span className="topbar-separator" />{t("app.dataSource")}</div></header><div className="page-content">{children}</div></main></div>;
}
