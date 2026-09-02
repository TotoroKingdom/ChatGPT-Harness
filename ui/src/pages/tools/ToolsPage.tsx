import type { PolicyDecision, RiskLevel, ToolDescriptor } from "@protocol/v1";
import { useMemo, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { AsyncStateView } from "@/components/status/AsyncStates";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

const decisionTone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";
const readinessTone = (readiness: ToolDescriptor["readiness"]) => readiness === "ready" ? "success" : "neutral";
const riskTone = (risk: RiskLevel) => risk === "high" ? "danger" : risk === "medium" ? "warning" : "neutral";

export function ToolsPage() {
  const { tools } = useHarnessData();
  const [query, setQuery] = useState("");
  return <div className="page-stack"><SectionHeading eyebrow="What ChatGPT can do right now" title="Tool catalog" description="The catalog below is reported by the Runtime. M1 is read-only; tools cannot be changed here." action={<div className="catalog-count">{tools.status === "success" || tools.status === "empty" ? `${tools.data.items.length} entries` : "M0 catalog"}</div>} /><AsyncStateView state={tools} empty={() => <><Card className="filter-bar"><input aria-label="Search tools" placeholder="Search tools" value={query} onChange={(event) => setQuery(event.target.value)} /></Card><div><div className="empty-inline">No tools are exposed in this mock scenario.</div></div></>}>{(data) => { const filtered = data.items.filter((tool) => `${tool.name} ${tool.description}`.toLowerCase().includes(query.toLowerCase())); return <><Card className="filter-bar"><span className="search-icon">⌕</span><input aria-label="Search tools" placeholder="Search tools" value={query} onChange={(event) => setQuery(event.target.value)} /><span className="muted">{filtered.length} shown</span></Card><div className="tool-list">{filtered.map((tool) => <ToolRow key={tool.name} tool={tool} />)}</div>{filtered.length === 0 && <div className="empty-inline">No tools match this search.</div>}</> }}</AsyncStateView></div>;
}

function ToolRow({ tool }: { tool: ToolDescriptor }) {
  return <Card className={`tool-row ${!tool.exposed ? "tool-muted" : ""}`}><div className="tool-leading"><div className="tool-symbol">{tool.name.slice(0, 1).toUpperCase()}</div><div><div className="tool-title"><strong>{tool.name}</strong><StatusBadge tone={tool.exposed ? "success" : "neutral"}>{tool.exposed ? "Exposed" : "Not exposed"}</StatusBadge></div><p>{tool.description}</p><span className="tool-scope mono">{tool.scope}</span></div></div><div className="tool-facts"><span><small>Enabled</small><StatusBadge tone={tool.enabled ? "success" : "neutral"}>{tool.enabled ? "Yes" : "No"}</StatusBadge></span><span><small>Policy</small><StatusBadge tone={decisionTone(tool.effectivePolicy)}>{tool.effectivePolicy.toUpperCase()}</StatusBadge></span><span><small>Readiness</small><StatusBadge tone={readinessTone(tool.readiness)}>{tool.readiness}</StatusBadge></span><span><small>Risk</small><StatusBadge tone={riskTone(tool.riskLevel)}>{tool.riskLevel}</StatusBadge></span></div></Card>;
}
