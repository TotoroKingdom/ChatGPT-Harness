import type { ReactNode } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import { Sidebar } from "./Sidebar";
import { useTranslation } from "react-i18next";
import { hasPolicyCritical } from "@/i18n/state-presentation";

export function AppShell({ page, onNavigate, children }: { page: PageKey; onNavigate: (page: PageKey) => void; children: ReactNode }) {
  const { t } = useTranslation();
  const pageMeta = pages.find((item) => item.key === page) ?? pages[0];
  const data = useHarnessData();
  const pending = data.approvals?.status === "success" || data.approvals?.status === "empty" ? data.approvals.data.items.filter((item) => item.status === "pending").length : undefined;
  return <div className="app-shell"><Sidebar activePage={page} onNavigate={onNavigate} adapterKind={data.client.adapterKind} runtimeStatus={data.overall ?? "stopped"} pendingCount={pending} restartRequired={data.restartRequired} observed={data.harnessObserved} critical={data.policies ? hasPolicyCritical(data.components.runtime, data.policies) : false} /><main className="main-area"><header className="page-header"><h1>{t(pageMeta.label)}</h1><span className="muted">{t("app.localOnly")} · {t("app.dataSource")}</span></header><div className="page-content">{children}</div></main></div>;
}
