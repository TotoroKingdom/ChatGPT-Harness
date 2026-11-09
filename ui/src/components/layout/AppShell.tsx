import { useEffect, useRef, type ReactNode } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { PageKey } from "@/app/router/routes";
import { pages } from "@/app/router/routes";
import { Sidebar } from "./Sidebar";
import { useTranslation } from "react-i18next";
import { hasPolicyCritical } from "@/i18n/state-presentation";
import { DiagnosticsTrigger } from "@/components/status/DiagnosticsHost";
import { Button, StatusBadge } from "@/components/common/primitives";

export function AppShell({ page, onNavigate, children }: { page: PageKey; onNavigate: (page: PageKey) => void; children: ReactNode }) {
  const { t } = useTranslation();
  const pageMeta = pages.find((item) => item.key === page) ?? pages[0];
  const data = useHarnessData();
  const mainRef = useRef<HTMLElement>(null);
  const pending = data.approvals?.status === "success" || data.approvals?.status === "empty" ? data.approvals.data.items.filter((item) => item.status === "pending").length : undefined;
  const critical = data.policies ? hasPolicyCritical(data.components.runtime, data.policies) : false;
  const effectiveBashDecision = data.policies?.status === "success" || data.policies?.status === "empty"
    ? data.policies.data.effective?.perTool.find((tool) => tool.tool === "bash")?.decision
    : undefined;
  const trustMode = data.config?.policies?.toolDefaults?.bash === "allow" || effectiveBashDecision === "allow";
  const status = data.overall ?? "stopped";
  const observed = data.harnessObserved !== false;
  useEffect(() => {
    const main = mainRef.current;
    if (!main) return;
    if (typeof main.scrollTo === "function") main.scrollTo({ top: 0, behavior: "auto" });
    else main.scrollTop = 0;
  }, [page]);
  return <div className="app-shell">
    <Sidebar activePage={page} onNavigate={onNavigate} adapterKind={data.client.adapterKind} runtimeStatus={status} pendingCount={pending} restartRequired={data.restartRequired} observed={data.harnessObserved} critical={critical} />
    <main className="main-area" ref={mainRef}>
      <header className="page-header">
        <div className="page-heading">
          <div className="page-kicker">{t(pageMeta.eyebrow)}</div>
          <h1>{t(pageMeta.label)}</h1>
          <p>{t(pageMeta.description)}</p>
          <span className="sr-only">{t("app.localOnly")} · {t("app.dataSource")}</span>
        </div>
        <div className="page-header-actions">
          <StatusBadge
            tone={critical ? "warning" : observed ? (status === "ready" ? "success" : status === "error" ? "danger" : status === "stopped" ? "neutral" : "warning") : "neutral"}
            visual={!observed ? "unknown" : critical ? "warning" : status}
          >
            {critical ? t("m8.needsAttention") : t("m8.harnessState", { status: observed ? t(`status.${status}`) : t("m8.unknown") })}
          </StatusBadge>
          {trustMode && <Button variant="quiet" className="trust-mode-indicator" title={t("policies.bashTrustModeResidualRisk")} onClick={() => onNavigate("policies")}><StatusBadge tone="warning" visual="warning">{t("policies.bashTrustModeBadge")}</StatusBadge></Button>}
          <DiagnosticsTrigger variant="quiet" />
        </div>
      </header>
      <div className="page-content">{children}</div>
    </main>
  </div>;
}
