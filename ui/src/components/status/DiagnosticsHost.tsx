import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { Navigate } from "@/app/router/routes";
import { Button, Inspector } from "@/components/common/primitives";
import { DiagnosticsPanel } from "./DiagnosticsPanel";

const OPEN_DIAGNOSTICS = "harness:open-diagnostics";

export function openConnectionDiagnostics() {
  if (typeof window !== "undefined") window.dispatchEvent(new Event(OPEN_DIAGNOSTICS));
}

export function DiagnosticsTrigger({
  children,
  className = "",
  variant = "secondary",
}: {
  children?: ReactNode;
  className?: string;
  variant?: "primary" | "secondary" | "quiet" | "danger";
}) {
  const { t } = useTranslation();
  return (
    <Button variant={variant} className={className} onClick={openConnectionDiagnostics}>
      {children ?? t("diagnostics.open")}
    </Button>
  );
}

export function DiagnosticsHost({ onNavigate }: { onNavigate?: Navigate }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [open, setOpen] = useState(false);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    const listener = () => setOpen(true);
    window.addEventListener(OPEN_DIAGNOSTICS, listener);
    return () => window.removeEventListener(OPEN_DIAGNOSTICS, listener);
  }, []);

  const run = async () => {
    setRunning(true);
    try {
      return await data.runDiagnostics();
    } finally {
      setRunning(false);
    }
  };

  if (!open) return null;
  return (
    <Inspector title={t("diagnostics.title")} onClose={() => setOpen(false)}>
      <DiagnosticsPanel
        run={data.lastDiagnostics}
        startup={data.latestStartupAttempt ?? null}
        running={running}
        onRun={run}
        onNavigate={(page, options) => {
          setOpen(false);
          onNavigate?.(page, options);
        }}
      />
    </Inspector>
  );
}
