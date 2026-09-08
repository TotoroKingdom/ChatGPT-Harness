import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useActivitySubscription } from "@/app/providers/useActivitySubscription";
import { AsyncStateView } from "@/components/status/AsyncStates";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import type { ActivityEvent } from "@protocol/v1";

const tone = (eventType: ActivityEvent["eventType"]) => eventType.includes("failed") || eventType === "validation_failed" || eventType === "workspace_violation" ? "danger" : eventType.includes("requested") || eventType === "approval_requested" ? "warning" : eventType.includes("completed") || eventType === "runtime_started" ? "success" : "neutral";

export function ActivityPage() {
  const { activity } = useHarnessData();
  const subscriptionState = useActivitySubscription();
  return <div className="page-stack"><SectionHeading eyebrow="Structured audit trail" title="Activity" description="Human-readable events from the Runtime. Raw transport logs stay outside the UI." action={<StatusBadge tone={subscriptionState === "subscribed" ? "success" : subscriptionState === "error" ? "danger" : "neutral"}>{subscriptionState === "subscribed" ? "Live subscription" : subscriptionState === "error" ? "Subscription unavailable" : "Connecting"}</StatusBadge>} /><AsyncStateView state={activity} empty={() => <Card className="state-card"><div className="state-icon">—</div><strong>No audit events yet</strong><span className="muted">New structured Runtime events will appear here.</span></Card>}>{(data) => <Card className="activity-card"><div className="activity-header"><span>Event</span><span>When</span></div><div className="activity-list">{data.items.map((item) => <ActivityRow key={item.eventId} event={item} />)}</div></Card>}</AsyncStateView></div>;
}

function ActivityRow({ event }: { event: ActivityEvent }) { return <details className="activity-row"><summary><span className="activity-type"><span className={`event-marker ${tone(event.eventType)}`} />{event.eventType}</span><span className="activity-summary">{event.summary}</span><time dateTime={event.timestamp}>{event.timestamp}</time></summary><div className="activity-detail"><span>Sequence {event.sequence}</span>{event.requestId && <span>Request <b className="mono">{event.requestId}</b></span>}<span className="metadata">{Object.entries(event.metadata).map(([key, value]) => <span key={key}><b>{key}</b> {value}</span>)}</span></div></details>; }

