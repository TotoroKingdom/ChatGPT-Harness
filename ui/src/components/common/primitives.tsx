import { useEffect, useLayoutEffect, useId, useRef, useState, type ReactNode, type ButtonHTMLAttributes, type InputHTMLAttributes, type SelectHTMLAttributes } from "react";
import { useTranslation } from "react-i18next";

export type StatusTone = "success" | "warning" | "danger" | "neutral" | "info";
export type StatusVisual =
  | "ready"
  | "running"
  | "starting"
  | "degraded"
  | "error"
  | "stopped"
  | "stopping"
  | "unknown"
  | "success"
  | "warning"
  | "danger"
  | "neutral"
  | "info";

function StatusGlyph({ visual }: { visual: StatusVisual }) {
  if (visual === "ready" || visual === "running" || visual === "success") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="m4.6 8.1 2.1 2.1 4.8-4.8" /></svg>;
  }
  if (visual === "starting" || visual === "stopping") {
    return <svg className="status-spinner" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.2" /><path d="M8 2.2a5.8 5.8 0 0 1 5.8 5.8" /></svg>;
  }
  if (visual === "degraded" || visual === "warning") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1.9 15 14H1z" /><path d="M8 5.6v4.1M8 12.1v.2" /></svg>;
  }
  if (visual === "error" || visual === "danger") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="m5.5 5.5 5 5m0-5-5 5" /></svg>;
  }
  if (visual === "stopped") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.2" y="3.2" width="9.6" height="9.6" rx="2.2" /></svg>;
  }
  if (visual === "neutral") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="M5.4 8h5.2" /></svg>;
  }
  if (visual === "info") {
    return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="M8 7.2v4M8 4.6v.2" /></svg>;
  }
  return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="M5.9 6.1a2.1 2.1 0 1 1 3.3 1.8c-.8.5-1.2 1-1.2 1.9M8 12v.1" /></svg>;
}

export function StatusBadge({
  tone = "neutral",
  visual,
  children,
}: {
  tone?: StatusTone;
  visual?: StatusVisual;
  children: ReactNode;
}) {
  const resolved = visual ?? tone;
  return <span className={`status-badge status-${tone}`}><StatusGlyph visual={resolved} /><span>{children}</span></span>;
}

export function ObservedTime({ value, fallback, className = "" }: { value: string | null | undefined; fallback: string; className?: string }) {
  const { i18n } = useTranslation();
  if (!value) return <time className={className}>{fallback}</time>;
  const parsed = new Date(value);
  const valid = Number.isFinite(parsed.getTime());
  if (!valid) return <time className={className} title={value}>{value}</time>;
  const formatted = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? "en", {
    year: "numeric",
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(parsed);
  return <time className={className} dateTime={value} title={value}><span>{formatted}</span><span className="sr-only" aria-hidden="true">{value}</span></time>;
}

export function Card({ className = "", children }: { className?: string; children: ReactNode }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function Stat({ label, value, hint }: { label: string; value: ReactNode; hint?: string }) {
  return <div className="stat"><span className="stat-label">{label}</span><strong className="stat-value">{value}</strong>{hint && <span className="stat-hint">{hint}</span>}</div>;
}

export function SectionHeading({ eyebrow, title, description, action, level = 2 }: { eyebrow?: string; title: string; description?: string; action?: ReactNode; level?: 2 | 3 }) {
  const Title = level === 3 ? "h3" : "h2";
  return <div className="section-heading"><div className="section-heading-copy">{eyebrow && <div className="eyebrow">{eyebrow}</div>}<Title>{title}</Title>{description && <p>{description}</p>}</div>{action && <div className="section-heading-action">{action}</div>}</div>;
}

export function Button({ variant = "secondary", className = "", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "quiet" | "danger" }) {
  return <button type="button" className={`button button-${variant} ${className}`} {...props} />;
}
export function TextField({ label, error, helper, ...props }: InputHTMLAttributes<HTMLInputElement> & { label: string; error?: string; helper?: string }) {
  const id = useId();
  return <label className="form-field"><span>{label}</span><input {...props} aria-label={label} aria-invalid={Boolean(error) || undefined} aria-describedby={error || helper ? id : undefined} />{(error || helper) && <small id={id} className={error ? "danger-text" : "muted"}>{error ?? helper}</small>}</label>;
}
export function Select({ label, children, ...props }: SelectHTMLAttributes<HTMLSelectElement> & { label: string; children: ReactNode }) {
  return <label className="form-field"><span>{label}</span><select {...props} aria-label={label}>{children}</select></label>;
}
export function Switch({ checked, onChange, label, disabled = false }: { checked: boolean; onChange: (value: boolean) => void; label: string; disabled?: boolean }) {
  return <button type="button" className="switch" role="switch" aria-checked={checked} aria-label={label} disabled={disabled} onClick={(event) => { event.stopPropagation(); onChange(!checked); }}><span /></button>;
}
export function Segmented<T extends string>({ value, options, onChange, label, disabled = false }: { value: T; options: { value: T; label: string }[]; onChange: (value: T) => void; label: string; disabled?: boolean }) {
  return <div className="segmented" role="radiogroup" aria-label={label}>{options.map((option, index) => <button type="button" key={option.value} role="radio" aria-checked={value === option.value} tabIndex={value === option.value ? 0 : -1} disabled={disabled} className={value === option.value ? "selected" : ""} onClick={(event) => { event.stopPropagation(); if (option.value !== value) onChange(option.value); }} onKeyDown={(event) => {
    const delta = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (!delta) return;
    event.preventDefault(); const target = (index + delta + options.length) % options.length;
    if (!disabled && options[target].value !== value) onChange(options[target].value); (event.currentTarget.parentElement?.children[target] as HTMLElement)?.focus();
  }}>{option.label}</button>)}</div>;
}
export function Row({ title, summary, controls, selected, onInspect }: { title: ReactNode; summary?: ReactNode; controls?: ReactNode; selected?: boolean; onInspect?: () => void }) {
  return <div className={`workspace-row ${selected ? "selected" : ""}`}>
    {onInspect ? <button type="button" className="row-identity" onClick={onInspect}>{title}{summary && <span className="row-summary">{summary}</span>}</button> : <div className="row-identity">{title}{summary && <span className="row-summary">{summary}</span>}</div>}
    {controls && <div className="row-controls">{controls}</div>}
  </div>;
}
export function Section({ title, children, action }: { title: string; children: ReactNode; action?: ReactNode }) {
  return <section className="workspace-section"><SectionHeading title={title} action={action} />{children}</section>;
}
export function InlineFeedback({ error, children }: { error?: boolean; children: ReactNode }) {
  return <div className={`inline-feedback ${error ? "inline-feedback-error" : ""}`} role={error ? "alert" : "status"} aria-live="polite"><span className="inline-feedback-mark" aria-hidden="true">{error ? "!" : "i"}</span><span>{children}</span></div>;
}
export function Disclosure({ title, children, open = false }: { title: string; children: ReactNode; open?: boolean }) {
  return <details className="disclosure" open={open || undefined}><summary>{title}</summary><div>{children}</div></details>;
}

function Surface({ title, children, onClose, kind }: { title: string; children: ReactNode; onClose: () => void; kind: "inspector" | "sheet" | "dialog" }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLElement | null>(null);
  const id = useId();
  const canDock = () => {
    const main = document.querySelector<HTMLElement>(".main-area");
    const width = main?.clientWidth || window.innerWidth - 220;
    return width - 64 - 360 >= 600;
  };
  const [wide, setWide] = useState(canDock);
  const modal = kind !== "inspector" || !wide;
  const closeRef = useRef(onClose); closeRef.current = onClose;
  useLayoutEffect(() => {
    const active = document.activeElement;
    if (active instanceof HTMLElement && !ref.current?.contains(active)) trigger.current = active;
  }, [title]);
  useEffect(() => {
    const resize = () => setWide(canDock());
    const main = document.querySelector<HTMLElement>(".main-area");
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(resize) : null;
    if (main) observer?.observe(main);
    window.addEventListener("resize", resize);
    return () => { observer?.disconnect(); window.removeEventListener("resize", resize); };
  }, []);
  useEffect(() => {
    if (!modal) return;
    const surface = ref.current;
    const root = surface?.closest(".surface-layer");
    const disabled: HTMLElement[] = [];
    let branch = root;
    while (branch?.parentElement && branch.parentElement !== document.body) {
      for (const sibling of Array.from(branch.parentElement.children)) if (sibling !== branch && sibling instanceof HTMLElement && !sibling.inert) { sibling.inert = true; disabled.push(sibling); }
      branch = branch.parentElement;
    }
    return () => { disabled.forEach((element) => { element.inert = false; }); };
  }, [modal]);
  useEffect(() => {
    ref.current?.focus();
    return () => {
      if (trigger.current?.isConnected) trigger.current.focus();
      else {
        const fallback = document.querySelector<HTMLElement>("main h1, .page-header h1");
        if (fallback) { fallback.tabIndex = -1; fallback.focus(); }
      }
    };
  }, []);
  return <div className={`surface-layer ${modal ? "modal-layer" : "docked-layer"} ${kind}-layer`} onMouseDown={(event) => { if (modal && event.target === event.currentTarget) closeRef.current(); }}><div ref={ref} className={`context-surface ${kind}`} role={modal ? "dialog" : "complementary"} aria-modal={modal || undefined} aria-labelledby={id} tabIndex={-1} onKeyDown={(event) => {
    if (event.key === "Escape") { event.stopPropagation(); closeRef.current(); }
    if (event.key !== "Tab" || !modal) return;
    const controls = Array.from(ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]') ?? []);
    const visible = controls.filter((control) => {
      let node: HTMLElement | null = control;
      while (node?.parentElement) {
        const parent: HTMLElement = node.parentElement;
        if (parent.tagName === "DETAILS" && !parent.hasAttribute("open") && parent.firstElementChild !== node) return false;
        node = parent;
      }
      return true;
    });
    const first = visible[0], last = visible.at(-1);
    if (event.shiftKey && (document.activeElement === first || document.activeElement === ref.current)) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && (document.activeElement === last || document.activeElement === ref.current)) { event.preventDefault(); first?.focus(); }
  }}><header className="surface-header"><div><span className="surface-kicker">{t("common.details")}</span><h2 id={id}>{title}</h2></div><Button variant="quiet" className="icon-button" aria-label={t("common.close")} onClick={onClose}><svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" /></svg></Button></header>{children}</div></div>;
}
export function Inspector(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="inspector" />; }
export function Sheet(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="sheet" />; }
export function Dialog(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="dialog" />; }
