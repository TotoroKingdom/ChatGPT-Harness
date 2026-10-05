import { useEffect, useLayoutEffect, useId, useRef, useState, type ReactNode, type ButtonHTMLAttributes, type InputHTMLAttributes, type SelectHTMLAttributes } from "react";
import { useTranslation } from "react-i18next";

export function StatusBadge({ tone = "neutral", children }: { tone?: "success" | "warning" | "danger" | "neutral"; children: ReactNode }) {
  return <span className={`status-badge status-${tone}`}><span className="status-dot" />{children}</span>;
}

export function Card({ className = "", children }: { className?: string; children: ReactNode }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function Stat({ label, value, hint }: { label: string; value: ReactNode; hint?: string }) {
  return <div className="stat"><span className="stat-label">{label}</span><strong className="stat-value">{value}</strong>{hint && <span className="stat-hint">{hint}</span>}</div>;
}

export function SectionHeading({ eyebrow, title, description, action }: { eyebrow?: string; title: string; description?: string; action?: ReactNode }) {
  return <div className="section-heading"><div>{eyebrow && <div className="eyebrow">{eyebrow}</div>}<h2>{title}</h2>{description && <p>{description}</p>}</div>{action}</div>;
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
  return <div className={`inline-feedback ${error ? "danger-text" : "muted"}`} role={error ? "alert" : "status"} aria-live="polite">{children}</div>;
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
  return <div className={`surface-layer ${modal ? "modal-layer" : "docked-layer"} ${kind}-layer`}><div ref={ref} className={`context-surface ${kind}`} role={modal ? "dialog" : "complementary"} aria-modal={modal || undefined} aria-labelledby={id} tabIndex={-1} onKeyDown={(event) => {
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
  }}><header className="surface-header"><h2 id={id}>{title}</h2><Button variant="quiet" aria-label={t("common.close")} onClick={onClose}><svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" /></svg></Button></header>{children}</div></div>;
}
export function Inspector(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="inspector" />; }
export function Sheet(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="sheet" />; }
export function Dialog(props: { title: string; children: ReactNode; onClose: () => void }) { return <Surface {...props} kind="dialog" />; }
