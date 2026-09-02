import type { PolicyDecision, PolicyRule } from "@protocol/v1";
import { useMemo, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { AsyncStateView } from "@/components/status/AsyncStates";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

const decisions: PolicyDecision[] = ["allow", "ask", "deny"];
const labels: Record<PolicyDecision, string> = { allow: "ALLOW", ask: "ASK", deny: "DENY" };
const tone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";

export function PoliciesPage() {
  const { policies } = useHarnessData();
  const [selected, setSelected] = useState<PolicyDecision | "all">("all");
  return <div className="page-stack"><SectionHeading eyebrow="Security posture" title="Policies" description="A read-only view of the effective rules reported by the Runtime." /><AsyncStateView state={policies} empty={() => <EmptyPolicies />}>{(data) => <><div className="segmented" role="tablist" aria-label="Policy filters"><button className={selected === "all" ? "selected" : ""} onClick={() => setSelected("all")}>All <span>{data.items.length}</span></button>{decisions.map((decision) => <button key={decision} className={selected === decision ? "selected" : ""} onClick={() => setSelected(decision)}>{labels[decision]} <span>{data.items.filter((rule) => rule.decision === decision).length}</span></button>)}</div><div className="policy-groups">{decisions.filter((decision) => selected === "all" || selected === decision).map((decision) => { const group = data.items.filter((rule) => rule.decision === decision); return group.length ? <div key={decision}><div className="group-heading"><StatusBadge tone={tone(decision)}>{labels[decision]}</StatusBadge><span>{group.length} rule{group.length === 1 ? "" : "s"}</span></div><div className="policy-list">{group.map((rule) => <PolicyRow key={rule.id} rule={rule} />)}</div></div> : null; })}</div></>}</AsyncStateView></div>;
}

function PolicyRow({ rule }: { rule: PolicyRule }) { return <Card className="policy-row"><div><div className="policy-title"><strong>{rule.name}</strong><StatusBadge tone={rule.source === "system" ? "neutral" : "warning"}>{rule.source}</StatusBadge></div><p>{rule.reason}</p></div><div className="policy-facts"><span><small>Scope</small><b className="mono">{rule.scope}</b></span><span><small>Enabled</small><b>{rule.enabled ? "Yes" : "No"}</b></span><span><small>Editing</small><b>{rule.editable ? "Available later" : "Read-only"}</b></span></div></Card>; }
function EmptyPolicies() { return <Card className="state-card"><div className="state-icon">—</div><strong>No policy rules returned</strong><span className="muted">The Runtime has not provided any rules for this view.</span></Card>; }
