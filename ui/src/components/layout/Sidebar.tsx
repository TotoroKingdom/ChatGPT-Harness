import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import type { HarnessAdapterKind, ComponentStatus } from "@/services/harness/client";
import { useTranslation } from "react-i18next";

const icons: Record<PageKey, string> = { overview: "M3 3h4v4H3zM11 3h4v4h-4zM3 11h4v4H3zM11 11h4v4h-4z", tools: "m3 4 4 4-4 4m6 2h6", policies: "M9 2 3 5v4c0 4 6 7 6 7s6-3 6-7V5z", activity: "M2 9h3l2-5 4 10 2-5h3", approvals: "m3 9 4 4 8-9", settings: "M3 5h12M3 13h12M6 3v4m6 4v4" };

export function Sidebar({ activePage, onNavigate, adapterKind, runtimeStatus, pendingCount, restartRequired, observed, critical }: { activePage: PageKey; onNavigate: (page: PageKey) => void; adapterKind: HarnessAdapterKind; runtimeStatus: ComponentStatus; pendingCount?: number; restartRequired?: boolean; observed?: boolean; critical?: boolean }) {
  const { t } = useTranslation();
  return <aside className="sidebar">
    <div className="brand"><div className="brand-mark">H</div><div><strong>Harness</strong><span>{t("app.controlPlane")}</span></div></div>
    <nav className="nav-list" aria-label={t("app.mainNavigation")}>
      <span className="nav-label">{t("app.workspace")}</span>
      {pages.slice(0, 5).map((item) => <button key={item.key} aria-current={activePage === item.key ? "page" : undefined} className={`nav-item ${activePage === item.key ? "active" : ""}`} onClick={() => onNavigate(item.key)}><svg className="nav-icon" width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d={icons[item.key]} /></svg><span>{t(item.label)}</span>{item.key === "approvals" && pendingCount !== undefined && pendingCount > 0 && <span className="pending-count" aria-label={t("m8.pending", { count: pendingCount })}>{pendingCount}</span>}</button>)}
      <span className="nav-label nav-label-spaced">{t("app.system")}</span>
      <button aria-current={activePage === "settings" ? "page" : undefined} className={`nav-item ${activePage === "settings" ? "active" : ""}`} onClick={() => onNavigate("settings")}><svg className="nav-icon" width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d={icons.settings} /></svg><span>{t("nav.settings")}</span></button>
    </nav>
    <div className="sidebar-footer"><button className="button button-quiet" onClick={() => onNavigate("overview")}><span className="connection-pill"><span className="status-dot" />{t("m8.harnessState", { status: critical ? t("m8.needsAttention") : observed ? t(`status.${runtimeStatus}`) : t("m8.unknown") })}</span>{restartRequired && <span className="warning-text">{t("m8.restart")}</span>}</button><span>{t(adapterKind === "real" ? "app.realSource" : "app.mockSource")}</span></div>
  </aside>;
}
