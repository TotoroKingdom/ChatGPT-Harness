import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";

const icons: Record<PageKey, string> = { overview: "◈", tools: "⌘", policies: "◇", activity: "≋", settings: "⚙" };

export function Sidebar({ activePage, onNavigate }: { activePage: PageKey; onNavigate: (page: PageKey) => void }) {
  return <aside className="sidebar">
    <div className="brand"><div className="brand-mark">H</div><div><strong>Harness</strong><span>ChatGPT control plane</span></div></div>
    <nav className="nav-list" aria-label="Main navigation">
      <span className="nav-label">Workspace</span>
      {pages.slice(0, 4).map((item) => <button key={item.key} className={`nav-item ${activePage === item.key ? "active" : ""}`} onClick={() => onNavigate(item.key)}><span className="nav-icon">{icons[item.key]}</span><span>{item.label}</span>{activePage === item.key && <span className="nav-active-line" />}</button>)}
      <span className="nav-label nav-label-spaced">System</span>
      <button className={`nav-item ${activePage === "settings" ? "active" : ""}`} onClick={() => onNavigate("settings")}><span className="nav-icon">{icons.settings}</span><span>Settings</span>{activePage === "settings" && <span className="nav-active-line" />}</button>
    </nav>
    <div className="sidebar-footer"><StatusPill /><span>v0.1.0 · M1 skeleton</span></div>
  </aside>;
}

function StatusPill() { return <div className="connection-pill"><span className="status-dot" />Mock Runtime</div>; }
