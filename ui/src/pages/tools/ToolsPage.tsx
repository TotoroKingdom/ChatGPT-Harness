import type { PolicyDecision, RiskLevel, ToolDescriptor } from "@protocol/v1";
import { useMemo, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { AsyncStateView } from "@/components/status/AsyncStates";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

const decisionTone = (decision: PolicyDecision) => decision === "allow" ? "success" : decision === "ask" ? "warning" : "danger";
const readinessTone = (readiness: ToolDescriptor["readiness"]) => readiness === "ready" ? "success" : "neutral";
const riskTone = (risk: RiskLevel) => risk === "high" ? "danger" : risk === "medium" ? "warning" : "neutral";
const toolStatus = (tool: ToolDescriptor) => !tool.exposed || tool.readiness === "unavailable" ? "Unavailable" : !tool.enabled ? "Disabled" : "Available";
const toolStatusReason = (tool: ToolDescriptor) => !tool.implemented ? "Not implemented by Runtime" : !tool.exposed ? "Not exposed by Runtime" : tool.readiness === "unavailable" ? "Runtime readiness unavailable" : !tool.enabled ? "Disabled by Runtime" : "";

export function ToolsPage() {
  const { tools } = useHarnessData();
  const [query, setQuery] = useState("");
  return <div className="page-stack"><SectionHeading eyebrow="What ChatGPT can do right now" title="Tool catalog" description="The effective catalog below is reported by the Runtime. This view is read-only; tools cannot be changed here." action={<div className="catalog-count">{tools.status === "success" || tools.status === "empty" ? `${tools.data.items.length} entries` : "Runtime unavailable"}</div>} /><AsyncStateView state={tools} empty={() => <><Card className="filter-bar"><input aria-label="Search tools" placeholder="Search tools" value={query} onChange={(event) => setQuery(event.target.value)} /></Card><div><div className="empty-inline">No tools are currently available from the Runtime.</div></div></>}>{(data) => { const filtered = data.items.filter((tool) => `${tool.name} ${tool.description}`.toLowerCase().includes(query.toLowerCase())); return <><Card className="filter-bar"><span className="search-icon">⌕</span><input aria-label="Search tools" placeholder="Search tools" value={query} onChange={(event) => setQuery(event.target.value)} /><span className="muted">{filtered.length} shown</span></Card><div className="tool-list">{filtered.map((tool) => <ToolRow key={tool.name} tool={tool} />)}</div>{filtered.length === 0 && <div className="empty-inline">No tools match this search.</div>}</> }}</AsyncStateView></div>;
}

function ToolRow({ tool }: { tool: ToolDescriptor }) {
  return <Card className={`tool-row ${!tool.exposed ? "tool-muted" : ""}`}><div className="tool-leading"><div className="tool-symbol">{tool.name.slice(0, 1).toUpperCase()}</div><div><div className="tool-title"><strong>{tool.name}</strong><StatusBadge tone={toolStatus(tool) === "Available" ? "success" : toolStatus(tool) === "Disabled" ? "warning" : "neutral"}>{toolStatus(tool)}</StatusBadge></div>{toolStatusReason(tool) && <small className="muted">{toolStatusReason(tool)}</small>}<p>{tool.description}</p><span className="tool-scope mono">{tool.scope}</span></div></div><div className="tool-facts"><span><small>Enabled</small><StatusBadge tone={tool.enabled ? "success" : "neutral"}>{tool.enabled ? "Yes" : "No"}</StatusBadge></span><span><small>Policy</small><StatusBadge tone={decisionTone(tool.effectivePolicy)}>{tool.effectivePolicy.toUpperCase()}</StatusBadge></span><span><small>Readiness</small><StatusBadge tone={readinessTone(tool.readiness)}>{tool.readiness}</StatusBadge></span><span><small>Risk</small><StatusBadge tone={riskTone(tool.riskLevel)}>{tool.riskLevel}</StatusBadge></span></div></Card>;
}
