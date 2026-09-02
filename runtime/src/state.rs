use crate::events::EventBus;
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, RuntimeConfigSnapshot, RuntimeErrorSummary, RuntimeLifecycle,
    RuntimeReadiness, RuntimeState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WaitingInitialize,
    InitializedPendingAck,
    Ready,
    Stopping,
}

pub struct RuntimeCore {
    pub runtime_id: String,
    pub started_at: String,
    pub phase: Phase,
    pub config: Option<RuntimeConfigSnapshot>,
    pub events: Option<EventBus>,
    pub last_error: Option<RuntimeErrorSummary>,
}

impl RuntimeCore {
    pub fn new() -> Self {
        Self {
            runtime_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            phase: Phase::WaitingInitialize,
            config: None,
            events: None,
            last_error: None,
        }
    }

    pub fn initialize(&mut self, config: RuntimeConfigSnapshot) {
        self.events = Some(EventBus::from_limits(&config.runtime_limits));
        self.config = Some(config);
        self.phase = Phase::InitializedPendingAck;
        self.last_error = None;
    }

    pub fn acknowledge(&mut self) -> Option<ActivityEvent> {
        if self.phase != Phase::InitializedPendingAck {
            return None;
        }
        self.phase = Phase::Ready;
        Some(self.publish(
            ActivityEventType::RuntimeStarted,
            "Runtime started.",
            [("source".into(), "runtime".into())],
            None,
        ))
    }

    pub fn stop(&mut self) -> Option<ActivityEvent> {
        if self.phase == Phase::Stopping {
            return None;
        }
        self.phase = Phase::Stopping;
        Some(self.publish(
            ActivityEventType::RuntimeStopped,
            "Runtime stopped.",
            [("source".into(), "runtime".into())],
            None,
        ))
    }

    pub fn publish(
        &mut self,
        event_type: ActivityEventType,
        summary: impl Into<String>,
        metadata: impl IntoIterator<Item = (String, String)>,
        request_id: Option<String>,
    ) -> ActivityEvent {
        self.events
            .as_mut()
            .expect("event bus exists after initialize")
            .publish(event_type, summary, metadata, request_id)
    }

    pub fn status(&self) -> Option<RuntimeState> {
        let config = self.config.as_ref()?;
        let (lifecycle, readiness) = match self.phase {
            Phase::WaitingInitialize => {
                (RuntimeLifecycle::Initializing, RuntimeReadiness::NotReady)
            }
            Phase::InitializedPendingAck => (
                RuntimeLifecycle::Initializing,
                RuntimeReadiness::Initializing,
            ),
            Phase::Ready => (RuntimeLifecycle::Ready, RuntimeReadiness::Ready),
            Phase::Stopping => (RuntimeLifecycle::Stopping, RuntimeReadiness::NotReady),
        };
        Some(RuntimeState {
            runtime_id: self.runtime_id.clone(),
            started_at: self.started_at.clone(),
            lifecycle,
            readiness,
            config_revision: config.config_revision,
            active_workspace: config.workspace.root_path.clone(),
            security_mode: config.security.mode.clone(),
            exposed_tool_catalog: Vec::new(),
            active_requests: 0,
            pending_approvals: 0,
            running_executions: 0,
            last_error: self.last_error.clone(),
        })
    }
}
