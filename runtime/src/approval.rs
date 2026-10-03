//! Runtime-owned approval lifecycle state.
//!
//! The manager is deliberately independent from MCP and Tauri. It owns the
//! current generation's safe approval projection and the async wake-up handle
//! for the exact invocation that registered it.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use harness_control_protocol::{
    ApprovalDecision, ApprovalRequest, ApprovalStatus, BashApprovalDetails,
    BashSessionGrantUnavailableReason, PolicyReason, RiskLevel, SanitizedArguments,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) const DEFAULT_APPROVAL_TTL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Clock injection keeps expiry tests deterministic without weakening the
/// production finite-TTL contract.
pub(crate) trait ApprovalClock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

struct SystemApprovalClock;

impl ApprovalClock for SystemApprovalClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub(crate) struct ApprovalFacts {
    pub(crate) request_id: Uuid,
    pub(crate) tool: String,
    pub(crate) sanitized_arguments: SanitizedArguments,
    pub(crate) scope_summary: String,
    /// Internal exact-request binding. This value is never serialized into
    /// ApprovalRequest or activity evidence.
    pub(crate) fingerprint: String,
    /// Exact normalized scope for internal SessionGrant matching only.
    pub(crate) grant_scope: Option<String>,
    pub(crate) bash_details: Option<BashApprovalDetails>,
    pub(crate) allow_for_session_eligible: Option<bool>,
    pub(crate) matched_rule_ids: Vec<String>,
    pub(crate) policy_reason: PolicyReason,
    pub(crate) risk_level: RiskLevel,
}

pub(crate) struct RegisteredApproval {
    pub(crate) request: ApprovalRequest,
    pub(crate) receiver: oneshot::Receiver<ApprovalResolution>,
    pub(crate) fingerprint: String,
}

pub(crate) struct ApprovalResolution {
    pub(crate) request_id: String,
    pub(crate) fingerprint: String,
    pub(crate) status: ApprovalStatus,
    pub(crate) decision: Option<ApprovalDecision>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalManagerError {
    DuplicateRequest,
    ApprovalNotFound,
    ApprovalAlreadyResolved,
    SessionGrantNotEligible,
}

struct Entry {
    request: ApprovalRequest,
    fingerprint: String,
    grant_scope: Option<String>,
    allow_for_session_eligible: bool,
    sender: Option<oneshot::Sender<ApprovalResolution>>,
}

struct ManagerState {
    entries: BTreeMap<String, Entry>,
}

/// Thread-safe shared state used concurrently by the MCP Tokio thread and
/// the synchronous control JSONL thread.
pub(crate) struct ApprovalManager {
    runtime_id: String,
    generation_id: String,
    ttl: std::time::Duration,
    clock: Arc<dyn ApprovalClock>,
    state: Mutex<ManagerState>,
}

impl ApprovalManager {
    pub(crate) fn new(runtime_id: impl Into<String>) -> Arc<Self> {
        let runtime_id = runtime_id.into();
        Arc::new(Self::with_clock(
            runtime_id.clone(),
            runtime_id,
            DEFAULT_APPROVAL_TTL,
            Arc::new(SystemApprovalClock),
        ))
    }

    #[cfg(test)]
    pub(crate) fn with_test_clock(
        runtime_id: impl Into<String>,
        generation_id: impl Into<String>,
        ttl: std::time::Duration,
        clock: Arc<dyn ApprovalClock>,
    ) -> Arc<Self> {
        Arc::new(Self::with_clock(
            runtime_id.into(),
            generation_id.into(),
            ttl,
            clock,
        ))
    }

    fn with_clock(
        runtime_id: String,
        generation_id: String,
        ttl: std::time::Duration,
        clock: Arc<dyn ApprovalClock>,
    ) -> Self {
        Self {
            runtime_id,
            generation_id,
            ttl,
            clock,
            state: Mutex::new(ManagerState {
                entries: BTreeMap::new(),
            }),
        }
    }

    pub(crate) fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    pub(crate) fn generation_id(&self) -> &str {
        &self.generation_id
    }

    pub(crate) fn ttl(&self) -> std::time::Duration {
        self.ttl
    }

    pub(crate) fn register(
        &self,
        facts: ApprovalFacts,
    ) -> Result<RegisteredApproval, ApprovalManagerError> {
        let now = self.clock.now();
        let expires_at =
            now + ChronoDuration::from_std(self.ttl).expect("approval TTL fits chrono duration");
        let request_id = facts.request_id.to_string();
        let fingerprint = facts.fingerprint.clone();
        let scope_summary = facts.scope_summary.clone();
        let grant_scope = facts.grant_scope.clone();
        let allow_for_session_eligible = facts.allow_for_session_eligible;
        let request = ApprovalRequest {
            request_id: request_id.clone(),
            runtime_id: self.runtime_id.clone(),
            generation_id: self.generation_id.clone(),
            tool: facts.tool,
            sanitized_arguments: facts.sanitized_arguments,
            scope_summary: scope_summary.clone(),
            matched_rule_ids: facts
                .matched_rule_ids
                .into_iter()
                .map(safe_rule_id)
                .collect(),
            policy_reason: facts.policy_reason,
            risk_level: facts.risk_level,
            bash_details: facts.bash_details.clone(),
            allow_for_session_eligible,
            allow_for_session_unavailable_reason: match allow_for_session_eligible {
                Some(false) => Some(BashSessionGrantUnavailableReason::UnclassifiedHighRisk),
                _ => None,
            },
            created_at: timestamp(now),
            expires_at: timestamp(expires_at),
            status: ApprovalStatus::Pending,
            resolved_at: None,
            decision: None,
        };
        let (sender, receiver) = oneshot::channel();
        let mut state = lock_state(&self.state);
        if state.entries.contains_key(&request_id) {
            return Err(ApprovalManagerError::DuplicateRequest);
        }
        state.entries.insert(
            request_id,
            Entry {
                request: request.clone(),
                fingerprint: fingerprint.clone(),
                grant_scope,
                allow_for_session_eligible: allow_for_session_eligible.unwrap_or(true),
                sender: Some(sender),
            },
        );
        Ok(RegisteredApproval {
            request,
            receiver,
            fingerprint,
        })
    }

    pub(crate) fn list(&self, status: Option<ApprovalStatus>) -> Vec<ApprovalRequest> {
        self.expire_due();
        let state = lock_state(&self.state);
        state
            .entries
            .values()
            .filter(|entry| {
                status
                    .as_ref()
                    .is_none_or(|wanted| &entry.request.status == wanted)
            })
            .map(|entry| entry.request.clone())
            .collect()
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.expire_due();
        let state = lock_state(&self.state);
        state
            .entries
            .values()
            .filter(|entry| entry.request.status == ApprovalStatus::Pending)
            .count()
    }

    pub(crate) fn resolve(
        &self,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRequest, ApprovalManagerError> {
        self.resolve_with(request_id, decision, |_request, _scope| {})
    }

    /// Resolves a request and runs a caller-owned preparation callback before
    /// the exact waiter is woken. T3 uses this to create a session grant
    /// before an AllowForSession continuation can execute.
    pub(crate) fn resolve_with<F>(
        &self,
        request_id: &str,
        decision: ApprovalDecision,
        prepare_wake: F,
    ) -> Result<ApprovalRequest, ApprovalManagerError>
    where
        F: FnOnce(&ApprovalRequest, Option<&str>),
    {
        self.expire_due();
        let now = self.clock.now();
        let mut state = lock_state(&self.state);
        let entry = state
            .entries
            .get_mut(request_id)
            .ok_or(ApprovalManagerError::ApprovalNotFound)?;
        if entry.request.status != ApprovalStatus::Pending {
            return Err(ApprovalManagerError::ApprovalAlreadyResolved);
        }

        if decision == ApprovalDecision::AllowForSession && !entry.allow_for_session_eligible {
            return Err(ApprovalManagerError::SessionGrantNotEligible);
        }

        let status = match decision {
            ApprovalDecision::AllowOnce | ApprovalDecision::AllowForSession => {
                ApprovalStatus::Approved
            }
            ApprovalDecision::Deny => ApprovalStatus::Denied,
        };
        entry.request.status = status.clone();
        entry.request.resolved_at = Some(timestamp(now));
        entry.request.decision = Some(decision);
        prepare_wake(&entry.request, entry.grant_scope.as_deref());
        let resolution = ApprovalResolution {
            request_id: entry.request.request_id.clone(),
            fingerprint: entry.fingerprint.clone(),
            status,
            decision: entry.request.decision,
        };
        if let Some(sender) = entry.sender.take() {
            let _ = sender.send(resolution);
        }
        Ok(entry.request.clone())
    }

    /// Transitions every pending request to Expired and wakes its exact
    /// waiter. Calling this is idempotent and safe against concurrent resolve.
    pub(crate) fn expire_due(&self) {
        let now = self.clock.now();
        let mut state = lock_state(&self.state);
        for entry in state.entries.values_mut() {
            if entry.request.status != ApprovalStatus::Pending {
                continue;
            }
            let expires_at = parse_timestamp(&entry.request.expires_at);
            if expires_at.is_some_and(|expires_at| expires_at <= now) {
                entry.request.status = ApprovalStatus::Expired;
                entry.request.resolved_at = Some(timestamp(now));
                let resolution = ApprovalResolution {
                    request_id: entry.request.request_id.clone(),
                    fingerprint: entry.fingerprint.clone(),
                    status: ApprovalStatus::Expired,
                    decision: None,
                };
                if let Some(sender) = entry.sender.take() {
                    let _ = sender.send(resolution);
                }
            }
        }
    }

    /// Cancels one waiter when the MCP request future is dropped (for example
    /// because the HTTP client disconnected). A concurrent control resolve or
    /// Runtime stop still wins the same terminal-transition race safely.
    pub(crate) fn cancel(&self, request_id: &str) {
        let now = self.clock.now();
        let mut state = lock_state(&self.state);
        let Some(entry) = state.entries.get_mut(request_id) else {
            return;
        };
        if entry.request.status != ApprovalStatus::Pending {
            return;
        }
        entry.request.status = ApprovalStatus::Cancelled;
        entry.request.resolved_at = Some(timestamp(now));
        let resolution = ApprovalResolution {
            request_id: entry.request.request_id.clone(),
            fingerprint: entry.fingerprint.clone(),
            status: ApprovalStatus::Cancelled,
            decision: None,
        };
        if let Some(sender) = entry.sender.take() {
            let _ = sender.send(resolution);
        }
    }

    /// Cancels current-generation pending approvals during Runtime stop or
    /// rollback. Terminal history remains observable until the manager is
    /// dropped, but no new generation reuses this state.
    pub(crate) fn cancel_all(&self) {
        let now = self.clock.now();
        let mut state = lock_state(&self.state);
        for entry in state.entries.values_mut() {
            if entry.request.status != ApprovalStatus::Pending {
                continue;
            }
            entry.request.status = ApprovalStatus::Cancelled;
            entry.request.resolved_at = Some(timestamp(now));
            let resolution = ApprovalResolution {
                request_id: entry.request.request_id.clone(),
                fingerprint: entry.fingerprint.clone(),
                status: ApprovalStatus::Cancelled,
                decision: None,
            };
            if let Some(sender) = entry.sender.take() {
                let _ = sender.send(resolution);
            }
        }
    }
}

fn lock_state(state: &Mutex<ManagerState>) -> std::sync::MutexGuard<'_, ManagerState> {
    state.lock().unwrap_or_else(|error| error.into_inner())
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn safe_rule_id(id: String) -> String {
    if id.len() <= 64
        && id.bytes().any(|byte| byte.is_ascii_lowercase())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        id
    } else {
        "<redacted>".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;
    use std::thread;

    struct ManualClock {
        now: Mutex<DateTime<Utc>>,
    }

    impl ManualClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                now: Mutex::new(
                    DateTime::parse_from_rfc3339("2026-10-02T00:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                ),
            })
        }

        fn advance(&self, duration: std::time::Duration) {
            let mut now = self.now.lock().unwrap();
            *now += ChronoDuration::from_std(duration).unwrap();
        }
    }

    impl ApprovalClock for ManualClock {
        fn now(&self) -> DateTime<Utc> {
            *self.now.lock().unwrap()
        }
    }

    fn facts(request_id: Uuid, tool: &str, scope: &str) -> ApprovalFacts {
        ApprovalFacts {
            request_id,
            tool: tool.into(),
            sanitized_arguments: [("path".into(), scope.into())].into_iter().collect(),
            scope_summary: scope.into(),
            fingerprint: format!("{tool}|{scope}"),
            grant_scope: Some(scope.into()),
            bash_details: None,
            allow_for_session_eligible: None,
            matched_rule_ids: vec!["ask-write".into()],
            policy_reason: PolicyReason::MatchedUserRules,
            risk_level: RiskLevel::Medium,
        }
    }

    #[tokio::test]
    async fn registers_lists_filters_and_resolves_each_decision() {
        let clock = ManualClock::new();
        let manager = ApprovalManager::with_test_clock(
            "runtime-a",
            "generation-a",
            DEFAULT_APPROVAL_TTL,
            clock,
        );
        let registration = manager
            .register(facts(Uuid::new_v4(), "write", "a.txt"))
            .unwrap();
        assert_eq!(manager.pending_count(), 1);
        assert_eq!(manager.list(Some(ApprovalStatus::Pending)).len(), 1);
        assert_eq!(registration.request.runtime_id, "runtime-a");
        assert_eq!(registration.request.generation_id, "generation-a");

        let resolved = manager
            .resolve(
                &registration.request.request_id,
                ApprovalDecision::AllowForSession,
            )
            .unwrap();
        assert_eq!(resolved.status, ApprovalStatus::Approved);
        assert_eq!(resolved.decision, Some(ApprovalDecision::AllowForSession));
        let wake = registration.receiver.await.unwrap();
        assert_eq!(wake.status, ApprovalStatus::Approved);
        assert_eq!(manager.list(Some(ApprovalStatus::Approved)).len(), 1);
        assert!(matches!(
            manager.resolve(&resolved.request_id, ApprovalDecision::Deny),
            Err(ApprovalManagerError::ApprovalAlreadyResolved)
        ));
    }

    #[tokio::test]
    async fn unknown_and_duplicate_requests_fail_closed() {
        let manager = ApprovalManager::new("runtime-a");
        let request_id = Uuid::new_v4();
        assert!(matches!(
            manager.resolve(&request_id.to_string(), ApprovalDecision::Deny),
            Err(ApprovalManagerError::ApprovalNotFound)
        ));
        let first = manager
            .register(facts(request_id, "write", "a.txt"))
            .unwrap();
        assert!(matches!(
            manager.register(facts(request_id, "write", "a.txt")),
            Err(ApprovalManagerError::DuplicateRequest)
        ));
        drop(first.receiver);
    }

    #[tokio::test]
    async fn deterministic_expiry_wakes_waiter_and_keeps_terminal_history() {
        let clock = ManualClock::new();
        let manager = ApprovalManager::with_test_clock(
            "runtime-a",
            "generation-a",
            std::time::Duration::from_secs(5),
            Arc::clone(&clock) as Arc<dyn ApprovalClock>,
        );
        let registration = manager
            .register(facts(Uuid::new_v4(), "write", "a.txt"))
            .unwrap();
        clock.advance(std::time::Duration::from_secs(5));
        manager.expire_due();
        let wake = registration.receiver.await.unwrap();
        assert_eq!(wake.status, ApprovalStatus::Expired);
        assert_eq!(manager.pending_count(), 0);
        assert_eq!(manager.list(Some(ApprovalStatus::Expired)).len(), 1);
    }

    #[tokio::test]
    async fn cancel_all_wakes_all_waiters_and_is_idempotent() {
        let manager = ApprovalManager::new("runtime-a");
        let first = manager
            .register(facts(Uuid::new_v4(), "write", "a.txt"))
            .unwrap();
        let second = manager
            .register(facts(Uuid::new_v4(), "edit", "b.txt"))
            .unwrap();
        manager.cancel_all();
        manager.cancel_all();
        assert_eq!(
            first.receiver.await.unwrap().status,
            ApprovalStatus::Cancelled
        );
        assert_eq!(
            second.receiver.await.unwrap().status,
            ApprovalStatus::Cancelled
        );
        assert_eq!(manager.list(Some(ApprovalStatus::Cancelled)).len(), 2);
    }

    #[tokio::test]
    async fn concurrent_resolve_has_one_winner_and_one_execution_wake() {
        let manager = ApprovalManager::new("runtime-a");
        let registration = manager
            .register(facts(Uuid::new_v4(), "write", "a.txt"))
            .unwrap();
        let request_id = registration.request.request_id.clone();
        let winners = Arc::new(AtomicUsize::new(0));
        thread::scope(|scope| {
            for decision in [ApprovalDecision::AllowOnce, ApprovalDecision::Deny] {
                let manager = Arc::clone(&manager);
                let winners = Arc::clone(&winners);
                let request_id = request_id.clone();
                scope.spawn(move || {
                    if manager.resolve(&request_id, decision).is_ok() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::SeqCst), 1);
        let wake = registration.receiver.await.unwrap();
        assert!(matches!(
            wake.status,
            ApprovalStatus::Approved | ApprovalStatus::Denied
        ));
    }

    #[tokio::test]
    async fn resolve_vs_expiry_has_one_terminal_transition() {
        let clock = ManualClock::new();
        let manager = ApprovalManager::with_test_clock(
            "runtime-a",
            "generation-a",
            std::time::Duration::from_secs(5),
            Arc::clone(&clock) as Arc<dyn ApprovalClock>,
        );
        let registration = manager
            .register(facts(Uuid::new_v4(), "write", "race-expiry.txt"))
            .unwrap();
        let request_id = registration.request.request_id.clone();
        clock.advance(std::time::Duration::from_secs(5));
        let barrier = Arc::new(Barrier::new(2));

        let resolve_result = thread::scope(|scope| {
            let resolve_barrier = Arc::clone(&barrier);
            let resolve_manager = Arc::clone(&manager);
            let resolve_id = request_id.clone();
            let resolve = scope.spawn(move || {
                resolve_barrier.wait();
                resolve_manager.resolve(&resolve_id, ApprovalDecision::AllowOnce)
            });
            let expire_barrier = Arc::clone(&barrier);
            let expire_manager = Arc::clone(&manager);
            let expire = scope.spawn(move || {
                expire_barrier.wait();
                expire_manager.expire_due();
            });
            let resolve_result = resolve.join().expect("resolve race worker");
            expire.join().expect("expiry race worker");
            resolve_result
        });

        assert_eq!(manager.pending_count(), 0);
        let approved = manager.list(Some(ApprovalStatus::Approved)).len();
        let expired = manager.list(Some(ApprovalStatus::Expired)).len();
        assert_eq!(approved + expired, 1);
        assert!(matches!(
            registration.receiver.await.unwrap().status,
            ApprovalStatus::Approved | ApprovalStatus::Expired
        ));
        assert!(
            resolve_result.is_ok() == (approved == 1),
            "resolve/expiry race produced inconsistent terminal ownership"
        );
    }

    #[tokio::test]
    async fn resolve_vs_cancel_has_one_terminal_transition() {
        let manager = ApprovalManager::new("runtime-a");
        let registration = manager
            .register(facts(Uuid::new_v4(), "write", "race-stop.txt"))
            .unwrap();
        let request_id = registration.request.request_id.clone();
        let barrier = Arc::new(Barrier::new(2));

        let resolve_result = thread::scope(|scope| {
            let resolve_barrier = Arc::clone(&barrier);
            let resolve_manager = Arc::clone(&manager);
            let resolve_id = request_id.clone();
            let resolve = scope.spawn(move || {
                resolve_barrier.wait();
                resolve_manager.resolve(&resolve_id, ApprovalDecision::AllowOnce)
            });
            let cancel_barrier = Arc::clone(&barrier);
            let cancel_manager = Arc::clone(&manager);
            let cancel = scope.spawn(move || {
                cancel_barrier.wait();
                cancel_manager.cancel_all();
            });
            let resolve_result = resolve.join().expect("resolve stop race worker");
            cancel.join().expect("cancel stop race worker");
            resolve_result
        });

        assert_eq!(manager.pending_count(), 0);
        let approved = manager.list(Some(ApprovalStatus::Approved)).len();
        let cancelled = manager.list(Some(ApprovalStatus::Cancelled)).len();
        assert_eq!(approved + cancelled, 1);
        assert!(matches!(
            registration.receiver.await.unwrap().status,
            ApprovalStatus::Approved | ApprovalStatus::Cancelled
        ));
        assert!(
            resolve_result.is_ok() == (approved == 1),
            "resolve/cancel race produced inconsistent terminal ownership"
        );
    }

    #[test]
    fn public_dto_never_contains_internal_fingerprint_or_raw_mutation_content() {
        let manager = ApprovalManager::new("runtime-a");
        let mut facts = facts(Uuid::new_v4(), "write", "private.txt");
        facts.fingerprint = "SECRET_RAW_WRITE_CONTENT".into();
        facts
            .sanitized_arguments
            .insert("content".into(), "<redacted content>".into());
        let request = manager.register(facts).unwrap().request;
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("<redacted content>"));
        assert!(!serialized.contains("SECRET_RAW_WRITE_CONTENT"));
        assert!(!serialized.contains("private content"));
    }

    #[tokio::test]
    async fn unclassified_bash_rejects_forced_session_grant_and_stays_pending() {
        let manager = ApprovalManager::new("runtime-a");
        let request_id = Uuid::new_v4();
        let mut registration = manager
            .register(ApprovalFacts {
                request_id,
                tool: "bash".into(),
                sanitized_arguments: [("command".into(), "echo <redacted>".into())]
                    .into_iter()
                    .collect(),
                scope_summary: "{\"version\":1,\"redacted\":true}".into(),
                fingerprint: "{\"command\":\"echo $(secret)\"}".into(),
                grant_scope: None,
                bash_details: None,
                allow_for_session_eligible: Some(false),
                matched_rule_ids: vec![],
                policy_reason: PolicyReason::ToolDefault,
                risk_level: RiskLevel::High,
            })
            .expect("register Bash approval");

        assert_eq!(registration.request.allow_for_session_eligible, Some(false));
        assert!(matches!(
            manager.resolve(
                &registration.request.request_id,
                ApprovalDecision::AllowForSession,
            ),
            Err(ApprovalManagerError::SessionGrantNotEligible)
        ));
        assert_eq!(manager.pending_count(), 1);
        assert!(matches!(
            registration.receiver.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
    }
}
