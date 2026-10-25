use chrono::{SecondsFormat, Utc};
use harness_control_protocol::{
    StartupAttempt, StartupCohort, StartupOutcome, StartupPhase, StartupPhaseStatus,
    StartupPhaseTiming,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

const PHASES: [StartupPhase; 9] = [
    StartupPhase::EligibilityPrepare,
    StartupPhase::RuntimeSpawn,
    StartupPhase::RuntimeInitialize,
    StartupPhase::McpEndpointReady,
    StartupPhase::TunnelResolve,
    StartupPhase::TunnelLaunch,
    StartupPhase::ControlPlaneConnected,
    StartupPhase::McpTargetReady,
    StartupPhase::HarnessReady,
];

#[derive(Clone)]
pub(crate) struct StartupPhaseRecorder {
    inner: Arc<Mutex<RecorderInner>>,
}

struct RecorderInner {
    latest: Option<RecordedAttempt>,
}

struct RecordedAttempt {
    attempt: StartupAttempt,
    monotonic_started: Instant,
    open_phase: Option<(StartupPhase, Instant)>,
}

impl Default for StartupPhaseRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupPhaseRecorder {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RecorderInner { latest: None })),
        }
    }

    pub(crate) fn begin(&self, harness_session_id: impl Into<String>, cohort: StartupCohort) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if let Some(previous) = inner.latest.as_mut() {
            if previous.attempt.finished_at.is_none() {
                finalize_open_phase(previous, StartupPhaseStatus::Incomplete, Some("SUPERSEDED"));
                finish_attempt(previous, StartupOutcome::Incomplete, Some("SUPERSEDED"));
            }
        }
        inner.latest = Some(RecordedAttempt {
            attempt: StartupAttempt {
                attempt_id: Uuid::new_v4().to_string(),
                harness_session_id: harness_session_id.into(),
                cohort,
                started_at: timestamp(),
                finished_at: None,
                outcome: StartupOutcome::Incomplete,
                total_duration_ms: None,
                phases: Vec::new(),
                failure_code: None,
                app_version: env!("CARGO_PKG_VERSION").into(),
                runtime_version: None,
                tunnel_version: None,
            },
            monotonic_started: Instant::now(),
            open_phase: None,
        });
    }

    pub(crate) fn start_phase(&self, session_id: &str, phase: StartupPhase) -> bool {
        self.with_active(session_id, |record| {
            if record.attempt.finished_at.is_some() {
                return false;
            }
            if record
                .attempt
                .phases
                .iter()
                .any(|timing| timing.phase == phase)
            {
                return false;
            }
            finalize_open_phase(
                record,
                StartupPhaseStatus::Incomplete,
                Some("PHASE_ADVANCED"),
            );
            let now = Instant::now();
            record.open_phase = Some((phase.clone(), now));
            record.attempt.phases.push(StartupPhaseTiming {
                phase,
                started_offset_ms: elapsed_ms(record.monotonic_started, now),
                duration_ms: 0,
                status: StartupPhaseStatus::Incomplete,
                safe_error_code: None,
            });
            true
        })
        .unwrap_or(false)
    }

    pub(crate) fn complete_phase(&self, session_id: &str, phase: StartupPhase) -> bool {
        self.complete_phase_with(session_id, phase, StartupPhaseStatus::Completed, None)
    }

    pub(crate) fn fail_phase(&self, session_id: &str, phase: StartupPhase, code: &str) -> bool {
        self.complete_phase_with(
            session_id,
            phase,
            StartupPhaseStatus::Failed,
            Some(safe_code(code)),
        )
    }

    fn complete_phase_with(
        &self,
        session_id: &str,
        phase: StartupPhase,
        status: StartupPhaseStatus,
        code: Option<String>,
    ) -> bool {
        self.with_active(session_id, |record| {
            if record.attempt.finished_at.is_some() {
                return false;
            }
            let now = Instant::now();
            let Some(index) = record
                .attempt
                .phases
                .iter()
                .position(|timing| timing.phase == phase)
            else {
                record.attempt.phases.push(StartupPhaseTiming {
                    phase: phase.clone(),
                    started_offset_ms: elapsed_ms(record.monotonic_started, now),
                    duration_ms: 0,
                    status: status.clone(),
                    safe_error_code: code.clone(),
                });
                return true;
            };
            let timing = &mut record.attempt.phases[index];
            timing.duration_ms = record
                .open_phase
                .as_ref()
                .filter(|(open, _)| *open == phase)
                .map(|(_, started)| elapsed_ms(*started, now))
                .unwrap_or(timing.duration_ms);
            timing.status = status;
            timing.safe_error_code = code;
            if record
                .open_phase
                .as_ref()
                .is_some_and(|(open, _)| *open == phase)
            {
                record.open_phase = None;
            }
            true
        })
        .unwrap_or(false)
    }

    pub(crate) fn fail_active(&self, session_id: &str, code: &str) -> bool {
        let safe = safe_code(code);
        self.with_active(session_id, |record| {
            if record.attempt.finished_at.is_some() {
                return false;
            }
            finalize_open_phase(record, StartupPhaseStatus::Failed, Some(&safe));
            finish_attempt(record, StartupOutcome::Failed, Some(&safe));
            true
        })
        .unwrap_or(false)
    }

    pub(crate) fn cancel(&self, session_id: &str) -> bool {
        self.finish_non_ready(session_id, StartupOutcome::Cancelled, "CANCELLED")
    }

    pub(crate) fn incomplete(&self, session_id: &str, code: &str) -> bool {
        self.finish_non_ready(session_id, StartupOutcome::Incomplete, code)
    }

    fn finish_non_ready(&self, session_id: &str, outcome: StartupOutcome, code: &str) -> bool {
        let safe = safe_code(code);
        self.with_active(session_id, |record| {
            if record.attempt.finished_at.is_some() {
                return false;
            }
            finalize_open_phase(record, StartupPhaseStatus::Incomplete, Some(&safe));
            finish_attempt(record, outcome, Some(&safe));
            true
        })
        .unwrap_or(false)
    }

    pub(crate) fn ready(&self, session_id: &str) -> bool {
        self.with_active(session_id, |record| {
            if record.attempt.finished_at.is_some() {
                return false;
            }
            let _ = complete_phase_inner(
                record,
                StartupPhase::HarnessReady,
                StartupPhaseStatus::Completed,
                None,
            );
            finish_attempt(record, StartupOutcome::Ready, None);
            true
        })
        .unwrap_or(false)
    }

    pub(crate) fn set_runtime_version(&self, session_id: &str, version: impl Into<String>) {
        let version = bounded_value(&version.into());
        let _ = self.with_active(session_id, |record| {
            record.attempt.runtime_version = Some(version);
        });
    }

    pub(crate) fn set_tunnel_version(&self, session_id: &str, version: impl Into<String>) {
        let version = bounded_value(&version.into());
        let _ = self.with_active(session_id, |record| {
            record.attempt.tunnel_version = Some(version);
        });
    }

    pub(crate) fn latest(&self) -> Option<StartupAttempt> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.latest.as_ref().map(|record| record.attempt.clone()))
    }

    fn with_active<T>(
        &self,
        session_id: &str,
        operation: impl FnOnce(&mut RecordedAttempt) -> T,
    ) -> Option<T> {
        let mut inner = self.inner.lock().ok()?;
        let record = inner.latest.as_mut()?;
        if record.attempt.harness_session_id != session_id {
            return None;
        }
        Some(operation(record))
    }
}

fn complete_phase_inner(
    record: &mut RecordedAttempt,
    phase: StartupPhase,
    status: StartupPhaseStatus,
    code: Option<String>,
) -> bool {
    let now = Instant::now();
    if let Some(index) = record
        .attempt
        .phases
        .iter()
        .position(|timing| timing.phase == phase)
    {
        let timing = &mut record.attempt.phases[index];
        timing.duration_ms = record
            .open_phase
            .as_ref()
            .filter(|(open, _)| *open == phase)
            .map(|(_, started)| elapsed_ms(*started, now))
            .unwrap_or(timing.duration_ms);
        timing.status = status;
        timing.safe_error_code = code;
        if record
            .open_phase
            .as_ref()
            .is_some_and(|(open, _)| *open == phase)
        {
            record.open_phase = None;
        }
        true
    } else {
        record.attempt.phases.push(StartupPhaseTiming {
            phase,
            started_offset_ms: elapsed_ms(record.monotonic_started, now),
            duration_ms: 0,
            status,
            safe_error_code: code,
        });
        true
    }
}

fn finalize_open_phase(
    record: &mut RecordedAttempt,
    status: StartupPhaseStatus,
    code: Option<&str>,
) {
    let Some((phase, started)) = record.open_phase.take() else {
        return;
    };
    if let Some(timing) = record
        .attempt
        .phases
        .iter_mut()
        .find(|timing| timing.phase == phase)
    {
        timing.duration_ms = elapsed_ms(started, Instant::now());
        timing.status = status;
        timing.safe_error_code = code.map(safe_code);
    }
}

fn finish_attempt(record: &mut RecordedAttempt, outcome: StartupOutcome, code: Option<&str>) {
    let now = Instant::now();
    for phase in PHASES {
        if !record
            .attempt
            .phases
            .iter()
            .any(|timing| timing.phase == phase)
        {
            record.attempt.phases.push(StartupPhaseTiming {
                phase,
                started_offset_ms: elapsed_ms(record.monotonic_started, now),
                duration_ms: 0,
                status: StartupPhaseStatus::Skipped,
                safe_error_code: code.map(safe_code),
            });
        }
    }
    record
        .attempt
        .phases
        .sort_by_key(|timing| phase_rank(&timing.phase));
    record.attempt.finished_at = Some(timestamp());
    record.attempt.total_duration_ms = Some(elapsed_ms(record.monotonic_started, now));
    record.attempt.outcome = outcome;
    record.attempt.failure_code = code.map(safe_code);
}

fn phase_rank(phase: &StartupPhase) -> usize {
    PHASES
        .iter()
        .position(|candidate| candidate == phase)
        .unwrap_or(PHASES.len())
}

fn elapsed_ms(start: Instant, end: Instant) -> u32 {
    end.saturating_duration_since(start)
        .as_millis()
        .min(u32::MAX as u128) as u32
}

fn safe_code(value: &str) -> String {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_uppercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        });
    if !valid {
        "UNKNOWN".into()
    } else {
        value.to_owned()
    }
}

fn bounded_value(value: &str) -> String {
    value.chars().take(128).collect()
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn session(recorder: &StartupPhaseRecorder) -> String {
        let session = "session-1".to_owned();
        recorder.begin(session.clone(), StartupCohort::Cold);
        session
    }

    #[test]
    fn successful_attempt_has_fixed_phases_and_monotonic_offsets() {
        let recorder = StartupPhaseRecorder::new();
        let session = session(&recorder);
        for phase in PHASES {
            recorder.start_phase(&session, phase.clone());
            thread::sleep(Duration::from_millis(1));
            recorder.complete_phase(&session, phase);
        }
        assert!(recorder.ready(&session));
        let attempt = recorder.latest().unwrap();
        assert_eq!(attempt.outcome, StartupOutcome::Ready);
        assert_eq!(attempt.phases.len(), PHASES.len());
        assert!(attempt.total_duration_ms.is_some());
        assert!(attempt
            .phases
            .windows(2)
            .all(|phases| phases[0].started_offset_ms <= phases[1].started_offset_ms));
        assert!(attempt
            .phases
            .iter()
            .all(|phase| phase.status == StartupPhaseStatus::Completed));
    }

    #[test]
    fn stale_session_cannot_close_the_current_attempt() {
        let recorder = StartupPhaseRecorder::new();
        let first = session(&recorder);
        recorder.start_phase(&first, StartupPhase::EligibilityPrepare);
        recorder.begin("session-2", StartupCohort::Warm);
        assert!(!recorder.complete_phase(&first, StartupPhase::EligibilityPrepare));
        assert_eq!(recorder.latest().unwrap().harness_session_id, "session-2");
    }

    #[test]
    fn failure_is_bounded_and_never_contains_raw_error_text() {
        let recorder = StartupPhaseRecorder::new();
        let session = session(&recorder);
        recorder.start_phase(&session, StartupPhase::RuntimeSpawn);
        recorder.fail_active(&session, "RUNTIME_SECRET=raw-api-key\nlong error");
        let attempt = recorder.latest().unwrap();
        assert_eq!(attempt.outcome, StartupOutcome::Failed);
        assert!(!serde_json::to_string(&attempt)
            .unwrap()
            .contains("raw-api-key"));
        assert!(attempt.failure_code.as_deref().unwrap().len() <= 64);
        assert_eq!(attempt.phases.len(), PHASES.len());
    }
}
