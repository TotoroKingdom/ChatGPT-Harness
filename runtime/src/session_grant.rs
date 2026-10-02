//! Narrow, memory-only grants created by an AllowForSession decision.

use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::sync::Mutex;

struct SessionGrant {
    runtime_id: String,
    tool: String,
    scope: String,
    source_request_id: String,
    created_at: String,
}

/// Runtime-generation-local session grants. The key is deliberately the
/// displayed exact scope rather than a tool-wide permission.
pub(crate) struct SessionGrantStore {
    runtime_id: String,
    grants: Mutex<BTreeMap<(String, String), SessionGrant>>,
}

impl SessionGrantStore {
    pub(crate) fn new(runtime_id: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            grants: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    pub(crate) fn matches(&self, runtime_id: &str, tool: &str, scope: &str) -> bool {
        if runtime_id != self.runtime_id {
            return false;
        }
        let grants = self
            .grants
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        grants.contains_key(&(tool.to_owned(), scope.to_owned()))
    }

    pub(crate) fn insert_from_approved_scope(
        &self,
        runtime_id: &str,
        source_request_id: &str,
        tool: &str,
        scope: &str,
        created_at: DateTime<Utc>,
    ) {
        if runtime_id != self.runtime_id {
            return;
        }
        let mut grants = self
            .grants
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        grants.insert(
            (tool.to_owned(), scope.to_owned()),
            SessionGrant {
                runtime_id: runtime_id.to_owned(),
                tool: tool.to_owned(),
                scope: scope.to_owned(),
                source_request_id: source_request_id.to_owned(),
                created_at: created_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            },
        );
    }

    pub(crate) fn clear(&self) {
        self.grants
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.grants
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    #[cfg(test)]
    fn debug_snapshot(&self) -> Vec<(String, String, String, String, String)> {
        self.grants
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .map(|grant| {
                (
                    grant.runtime_id.clone(),
                    grant.tool.clone(),
                    grant.scope.clone(),
                    grant.source_request_id.clone(),
                    grant.created_at.clone(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_current_generation_tool_and_exact_scope() {
        let store = SessionGrantStore::new("generation-a");
        let created_at = DateTime::parse_from_rfc3339("2026-10-02T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        store.insert_from_approved_scope(
            "generation-a",
            "request-a",
            "write",
            "foo.txt",
            created_at,
        );
        assert!(store.matches("generation-a", "write", "foo.txt"));
        assert!(!store.matches("generation-a", "write", "bar.txt"));
        assert!(!store.matches("generation-a", "edit", "foo.txt"));
        assert!(!store.matches("generation-b", "write", "foo.txt"));
        assert_eq!(store.len(), 1);
        assert_eq!(store.debug_snapshot()[0].3, "request-a");
    }

    #[test]
    fn clear_removes_all_memory_only_grants() {
        let store = SessionGrantStore::new("generation-a");
        let now = Utc::now();
        store.insert_from_approved_scope("generation-a", "a", "write", "foo.txt", now);
        store.insert_from_approved_scope("generation-a", "b", "edit", "foo.txt", now);
        store.clear();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn foreign_generation_cannot_insert_a_grant() {
        let store = SessionGrantStore::new("generation-a");
        store.insert_from_approved_scope(
            "generation-b",
            "request-b",
            "write",
            "foo.txt",
            Utc::now(),
        );
        assert_eq!(store.len(), 0);
    }
}
