use harness_control_protocol::{ActivityEvent, ApprovalRequest};
use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
};

use super::PendingApprovalCount;

pub type PendingCount = PendingApprovalCount;

const MAX_DEDUPE_KEYS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovalIdentity {
    pub runtime_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationContent {
    pub identity: ApprovalIdentity,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRefresh {
    pub pending_count: PendingCount,
    pub notification: Option<NotificationContent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalClickRoute {
    Pending(String),
    Terminal(String),
    Unavailable(String),
}

#[derive(Default)]
pub struct ApprovalNotificationCoordinator {
    seen: HashSet<ApprovalIdentity>,
    order: VecDeque<ApprovalIdentity>,
    delivery_unavailable: bool,
}

impl ApprovalNotificationCoordinator {
    pub fn bootstrap(&mut self, approvals: &[ApprovalRequest]) -> ApprovalRefresh {
        for approval in approvals.iter().filter(|item| is_pending(item)) {
            self.remember(identity(approval));
        }
        self.refresh(approvals)
    }

    pub fn refresh(&mut self, approvals: &[ApprovalRequest]) -> ApprovalRefresh {
        ApprovalRefresh {
            pending_count: PendingCount::Known(
                approvals.iter().filter(|item| is_pending(item)).count() as u64,
            ),
            notification: None,
        }
    }

    pub fn runtime_unavailable(&mut self) -> ApprovalRefresh {
        ApprovalRefresh {
            pending_count: PendingCount::Unavailable,
            notification: None,
        }
    }

    pub fn observe_activity(
        &mut self,
        event: &ActivityEvent,
        approvals: &[ApprovalRequest],
    ) -> ApprovalRefresh {
        let pending_count =
            PendingCount::Known(approvals.iter().filter(|item| is_pending(item)).count() as u64);
        let should_consider_new = matches!(
            event.event_type,
            harness_control_protocol::ActivityEventType::ApprovalRequested
        );
        if !should_consider_new {
            return ApprovalRefresh {
                pending_count,
                notification: None,
            };
        }
        let Some(request_id) = event.request_id.as_deref() else {
            return ApprovalRefresh {
                pending_count,
                notification: None,
            };
        };
        let Some(approval) = approvals
            .iter()
            .find(|item| item.request_id == request_id && is_pending(item))
        else {
            return ApprovalRefresh {
                pending_count,
                notification: None,
            };
        };
        let key = identity(approval);
        if !self.seen.insert(key.clone()) {
            return ApprovalRefresh {
                pending_count,
                notification: None,
            };
        }
        self.order.push_back(key);
        self.trim();
        ApprovalRefresh {
            pending_count,
            notification: format_approval_notification(approval),
        }
    }

    pub fn record_delivery_failure(&mut self) {
        self.delivery_unavailable = true;
    }

    pub fn delivery_unavailable(&self) -> bool {
        self.delivery_unavailable
    }

    fn remember(&mut self, key: ApprovalIdentity) {
        if self.seen.insert(key.clone()) {
            self.order.push_back(key);
            self.trim();
        }
    }

    fn trim(&mut self) {
        while self.order.len() > MAX_DEDUPE_KEYS {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }
    }
}

fn identity(approval: &ApprovalRequest) -> ApprovalIdentity {
    ApprovalIdentity {
        runtime_id: approval.runtime_id.clone(),
        request_id: approval.request_id.clone(),
    }
}

fn is_pending(approval: &ApprovalRequest) -> bool {
    approval.status == harness_control_protocol::ApprovalStatus::Pending
}

pub fn format_approval_notification(approval: &ApprovalRequest) -> Option<NotificationContent> {
    let tool = match approval.tool.as_str() {
        "read_file" | "glob" | "write" | "edit" | "bash" => approval.tool.as_str(),
        _ => return None,
    };
    let identity = identity(approval);
    Some(NotificationContent {
        identity,
        title: "ChatGPT-Harness — Approval required".into(),
        body: format!("Approval required for {tool}. Open ChatGPT-Harness to review."),
    })
}

pub fn route_notification_click(
    runtime_id: &str,
    request_id: &str,
    approvals: &[ApprovalRequest],
) -> ApprovalClickRoute {
    let Some(approval) = approvals
        .iter()
        .find(|item| item.runtime_id == runtime_id && item.request_id == request_id)
    else {
        return ApprovalClickRoute::Unavailable(request_id.into());
    };
    if is_pending(approval) {
        ApprovalClickRoute::Pending(request_id.into())
    } else {
        ApprovalClickRoute::Terminal(request_id.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        ActivityEventType, ApprovalStatus, BashApprovalDetails, BashApprovalSegment,
        BashClassification, BashRiskFlag, BashWorkingDirectory, PolicyReason, RiskLevel,
    };

    fn approval(
        runtime_id: &str,
        request_id: &str,
        status: ApprovalStatus,
        tool: &str,
    ) -> ApprovalRequest {
        ApprovalRequest {
            request_id: request_id.into(),
            runtime_id: runtime_id.into(),
            generation_id: format!("generation-{runtime_id}"),
            tool: tool.into(),
            sanitized_arguments: [("path".into(), "SECRET_PATH".into())]
                .into_iter()
                .collect(),
            scope_summary: "C:\\private\\SECRET_PATH".into(),
            matched_rule_ids: vec![],
            policy_reason: PolicyReason::MatchedUserRules,
            risk_level: RiskLevel::Medium,
            bash_details: None::<BashApprovalDetails>,
            allow_for_session_eligible: None,
            allow_for_session_unavailable_reason: None,
            created_at: "2026-10-24T00:00:00Z".into(),
            expires_at: "2026-10-24T00:05:00Z".into(),
            status,
            resolved_at: None,
            decision: None,
        }
    }

    fn event(event_type: ActivityEventType, request_id: &str) -> ActivityEvent {
        ActivityEvent {
            event_id: format!("event-{request_id}"),
            runtime_id: "runtime-1".into(),
            sequence: harness_control_protocol::DecimalU64::new(1),
            event_type,
            request_id: Some(request_id.into()),
            timestamp: "2026-10-24T00:00:00Z".into(),
            summary: "safe".into(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn new_pending_emits_once_and_duplicate_or_poll_does_not_repeat() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        assert!(coordinator
            .observe_activity(
                &event(ActivityEventType::ApprovalRequested, "request-1"),
                &[pending.clone()]
            )
            .notification
            .is_some());
        assert!(coordinator
            .observe_activity(
                &event(ActivityEventType::ApprovalRequested, "request-1"),
                &[pending.clone()]
            )
            .notification
            .is_none());
        assert!(coordinator.refresh(&[pending]).notification.is_none());
    }

    #[test]
    fn bootstrap_seeds_existing_pending_without_notification() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        let result = coordinator.bootstrap(&[pending]);
        assert_eq!(result.pending_count, PendingCount::Known(1));
        assert!(result.notification.is_none());
    }

    #[test]
    fn identity_is_runtime_and_request_and_distinct_approvals_notify_twice() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let first = approval(
            "runtime-1",
            "same-request",
            ApprovalStatus::Pending,
            "write",
        );
        let second = approval("runtime-2", "same-request", ApprovalStatus::Pending, "edit");
        assert!(coordinator
            .observe_activity(
                &event(ActivityEventType::ApprovalRequested, "same-request"),
                &[first]
            )
            .notification
            .is_some());
        assert!(coordinator
            .observe_activity(
                &event(ActivityEventType::ApprovalRequested, "same-request"),
                &[second]
            )
            .notification
            .is_some());
    }

    #[test]
    fn terminal_before_send_is_not_notified_and_count_tracks_authoritative_list() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let terminal = approval("runtime-1", "request-1", ApprovalStatus::Approved, "write");
        let result = coordinator.observe_activity(
            &event(ActivityEventType::ApprovalRequested, "request-1"),
            &[terminal],
        );
        assert_eq!(result.pending_count, PendingCount::Known(0));
        assert!(result.notification.is_none());
    }

    #[test]
    fn pending_count_transitions_follow_authoritative_refreshes() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        let terminal = approval("runtime-1", "request-1", ApprovalStatus::Expired, "write");
        assert_eq!(
            coordinator.refresh(&[]).pending_count,
            PendingCount::Known(0)
        );
        assert_eq!(
            coordinator
                .observe_activity(
                    &event(ActivityEventType::ApprovalRequested, "request-1"),
                    &[pending]
                )
                .pending_count,
            PendingCount::Known(1)
        );
        assert_eq!(
            coordinator.refresh(&[terminal]).pending_count,
            PendingCount::Known(0)
        );
    }

    #[test]
    fn formatter_excludes_scope_arguments_paths_and_secret_sentinels() {
        let mut pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        pending
            .sanitized_arguments
            .insert("apiKey".into(), "M01_RAW_API_KEY_SENTINEL".into());
        pending
            .sanitized_arguments
            .insert("content".into(), "M01_FILE_CONTENT_SENTINEL".into());
        pending.scope_summary = "M01_PHYSICAL_PATH_SENTINEL".into();
        pending.matched_rule_ids = vec![
            "M01_ERROR_DETAIL_SENTINEL".into(),
            "M01_METADATA_JSON_SENTINEL".into(),
        ];
        pending.bash_details = Some(BashApprovalDetails {
            command_preview: "M01_RAW_BASH_SENTINEL".into(),
            classification: BashClassification::Classified,
            normalized_segments: vec![BashApprovalSegment {
                executable: "M01_RAW_BASH_SENTINEL".into(),
                argv: vec!["M01_RAW_BASH_SENTINEL".into()],
                argv_redacted: true,
            }],
            risk_flags: vec![BashRiskFlag::Eval],
            cwd: BashWorkingDirectory::ActiveWorkspace,
            preview_redacted: true,
            preview_truncated: false,
            destructive: false,
            destructive_reasons: Vec::new(),
        });
        let content = format_approval_notification(&pending).unwrap();
        let payload = serde_json::json!({
            "title": content.title,
            "body": content.body,
            "runtimeId": content.identity.runtime_id,
            "requestId": content.identity.request_id,
        })
        .to_string();
        assert!(payload.contains("write"));
        for sentinel in [
            "M01_RAW_API_KEY_SENTINEL",
            "M01_PHYSICAL_PATH_SENTINEL",
            "M01_RAW_BASH_SENTINEL",
            "M01_FILE_CONTENT_SENTINEL",
            "M01_ERROR_DETAIL_SENTINEL",
            "M01_METADATA_JSON_SENTINEL",
            "sanitizedArguments",
        ] {
            assert!(
                !payload.contains(sentinel),
                "notification leaked {sentinel}"
            );
        }
    }

    #[test]
    fn delivery_failure_does_not_change_pending_source_or_retry_key() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        let pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        let first = coordinator.observe_activity(
            &event(ActivityEventType::ApprovalRequested, "request-1"),
            &[pending.clone()],
        );
        coordinator.record_delivery_failure();
        let second = coordinator.observe_activity(
            &event(ActivityEventType::ApprovalRequested, "request-1"),
            &[pending.clone()],
        );
        assert!(first.notification.is_some());
        assert!(second.notification.is_none());
        assert_eq!(pending.status, ApprovalStatus::Pending);
        assert_eq!(
            coordinator.refresh(&[pending]).pending_count,
            PendingCount::Known(1)
        );
    }

    #[test]
    fn runtime_loss_is_unavailable_and_click_routes_are_non_authorizing() {
        let mut coordinator = ApprovalNotificationCoordinator::default();
        assert_eq!(
            coordinator.runtime_unavailable().pending_count,
            PendingCount::Unavailable
        );
        let pending = approval("runtime-1", "request-1", ApprovalStatus::Pending, "write");
        assert_eq!(
            route_notification_click("runtime-1", "request-1", &[pending]),
            ApprovalClickRoute::Pending("request-1".into())
        );
        let terminal = approval("runtime-1", "request-2", ApprovalStatus::Denied, "write");
        assert_eq!(
            route_notification_click("runtime-1", "request-2", &[terminal]),
            ApprovalClickRoute::Terminal("request-2".into())
        );
        assert_eq!(
            route_notification_click("runtime-1", "missing", &[]),
            ApprovalClickRoute::Unavailable("missing".into())
        );
    }
}
