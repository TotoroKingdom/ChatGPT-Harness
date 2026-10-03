import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import type { HarnessAdapterKind, ComponentStatus } from "@/services/harness/client";
import { useTranslation } from "react-i18next";

const icons: Record<PageKey, string> = { overview: "◈", tools: "⌘", policies: "◇", activity: "≋", approvals: "✓", settings: "⚙" };

export function Sidebar({ activePage, onNavigate, adapterKind, runtimeStatus }: { activePage: PageKey; onNavigate: (page: PageKey) => void; adapterKind: HarnessAdapterKind; runtimeStatus: ComponentStatus }) {
  const { t } = useTranslation();
  return <aside className="sidebar">
    <div className="brand"><div className="brand-mark">H</div><div><strong>Harness</strong><span>{t("app.controlPlane")}</span></div></div>
    <nav className="nav-list" aria-label={t("app.mainNavigation")}>
      <span className="nav-label">{t("app.workspace")}</span>
      {pages.slice(0, 5).map((item) => <button key={item.key} className={`nav-item ${activePage === item.key ? "active" : ""}`} onClick={() => onNavigate(item.key)}><span className="nav-icon">{icons[item.key]}</span><span>{t(item.label)}</span>{activePage === item.key && <span className="nav-active-line" />}</button>)}
      <span className="nav-label nav-label-spaced">{t("app.system")}</span>
      <button className={`nav-item ${activePage === "settings" ? "active" : ""}`} onClick={() => onNavigate("settings")}><span className="nav-icon">{icons.settings}</span><span>{t("nav.settings")}</span>{activePage === "settings" && <span className="nav-active-line" />}</button>
    </nav>
    <div className="sidebar-footer"><StatusPill adapterKind={adapterKind} runtimeStatus={runtimeStatus} /><span>{t(adapterKind === "real" ? "app.realSource" : "app.mockSource")}</span></div>
  </aside>;
}

function StatusPill({ adapterKind, runtimeStatus }: { adapterKind: HarnessAdapterKind; runtimeStatus: ComponentStatus }) {
  const { t } = useTranslation();
  const connected = runtimeStatus === "ready" || runtimeStatus === "degraded";
  const label = t(adapterKind === "real" ? "app.runtimeStatus" : "app.mockStatus", { status: t(`status.${runtimeStatus}`) });
  return <div className="connection-pill"><span className={`status-dot ${connected ? "" : "status-dot-muted"}`} />{label}</div>;
}
