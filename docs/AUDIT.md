# Audit Events

## Purpose

Runtime emits a small, structured event for each tool request so a future Activity
view or JSONL consumer can correlate the request, lifecycle, authorization result,
workspace-boundary result, outcome, and timestamp. M6-C1 only defines and emits
these events; it does not persist, query, display, or upload them.

## Event types

Each request keeps one `request_id` and emits events in pipeline order:

- `request_received`
- `workspace_validated`
- `policy_evaluated`
- `execution_started`
- `execution_completed`
- `execution_failed`
- `request_denied`
- `approval_required`

The terminal branches are `execution_completed`, `execution_failed`,
`request_denied`, or `approval_required` as applicable. Workspace validation
failure emits `workspace_validated` with a failed result and does not evaluate
policy or execute a tool.

## Event model and classification

`AuditEvent` is serde-serializable and contains only bounded metadata:
`event_id`, `request_id`, UTC `timestamp`, event type, tool name, a safe lifecycle
state, logical `workspace_id`, workspace validation result, policy decision,
matched rule IDs, and a coarse outcome.

These fields are operational metadata and authorization evidence. The workspace
identifier is the logical label `active`, never a configured or canonical path.
Policy evidence is copied from the one evaluation already performed by the
pipeline; audit emission never calls `PolicyEngine`.

## Redaction boundary

Audit events do not contain raw MCP arguments, tool payloads, paths, credentials,
environment variables, errors, or tool results. The pipeline context and report
therefore cannot echo secrets. If a future feature needs an argument projection,
it must first pass through the runtime redaction helper; sensitive keys including
`secret`, `api_key`, `credential`, `password`, `token`, and `authorization` are
replaced with `[REDACTED]`.

This is a data-minimization boundary, not a claim that arbitrary future logs are
safe. New audit fields require an explicit classification and redaction review.

## Future storage direction

A later phase may consume emitted events through a bounded event sink and choose
JSONL or another local storage implementation. Storage limits, retention,
querying, UI Activity integration, and remote upload are intentionally not part
of M6-C1.
