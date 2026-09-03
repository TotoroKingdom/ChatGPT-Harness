use harness_control_protocol::{ActivityEvent, ActivityEventType, DecimalU64, RuntimeLimits};
use std::collections::VecDeque;

#[derive(Debug)]
pub struct EventBus {
    max_events: usize,
    next_sequence: u64,
    history: VecDeque<ActivityEvent>,
}

impl EventBus {
    pub fn new(max_events: u32) -> Self {
        Self {
            max_events: max_events.max(1) as usize,
            next_sequence: 0,
            history: VecDeque::new(),
        }
    }

    pub fn publish(
        &mut self,
        event_type: ActivityEventType,
        summary: impl Into<String>,
        metadata: impl IntoIterator<Item = (String, String)>,
        request_id: Option<String>,
    ) -> ActivityEvent {
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("activity sequence exhausted");
        let event = ActivityEvent {
            event_id: format!("event_{}", uuid::Uuid::new_v4()),
            sequence: DecimalU64::new(self.next_sequence),
            event_type,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            summary: summary.into(),
            metadata: metadata.into_iter().collect(),
        };
        self.history.push_back(event.clone());
        while self.history.len() > self.max_events {
            self.history.pop_front();
        }
        event
    }

    pub fn latest_sequence(&self) -> DecimalU64 {
        DecimalU64::new(self.next_sequence)
    }

    pub fn after(&self, after: Option<DecimalU64>, limit: Option<u32>) -> Vec<ActivityEvent> {
        let cursor = after.unwrap_or(DecimalU64::ZERO).get();
        let max = limit.unwrap_or(self.max_events as u32).max(1) as usize;
        self.history
            .iter()
            .filter(|event| event.sequence.get() > cursor)
            .take(max)
            .cloned()
            .collect()
    }

    pub fn from_limits(limits: &RuntimeLimits) -> Self {
        Self::new(limits.max_activity_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::ActivityEventType;

    #[test]
    fn subscribe_after_sequence_only_replays_missing_events() {
        let mut bus = EventBus::new(3);
        for index in 1..=3 {
            bus.publish(
                ActivityEventType::RuntimeStarted,
                format!("event {index}"),
                std::iter::empty::<(String, String)>(),
                None,
            );
        }

        let events = bus.after(Some(DecimalU64::new(1)), None);
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![DecimalU64::new(2), DecimalU64::new(3)]
        );
    }

    #[test]
    fn history_is_bounded_and_sequences_remain_monotonic() {
        let mut bus = EventBus::new(2);
        for index in 1..=3 {
            bus.publish(
                ActivityEventType::RuntimeStarted,
                format!("event {index}"),
                std::iter::empty::<(String, String)>(),
                None,
            );
        }

        let events = bus.after(None, None);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, DecimalU64::new(2));
        assert_eq!(events[1].sequence, DecimalU64::new(3));
        assert_eq!(bus.latest_sequence(), DecimalU64::new(3));
    }
}
