use harness_control_protocol::{PolicyDecision, RiskLevel, ToolDescriptor, ToolReadiness};
use serde::{Deserialize, Serialize};

const WORKSPACE_SCOPE: &str = "workspace";

/// The Runtime's single source of truth for tool implementation and exposure state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRegistry {
    entries: Vec<RegisteredTool>,
}

/// Capability semantics live alongside the one authoritative tool registration.
/// Future capability additions must extend this type and the policy match explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    WorkspaceRead,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegisteredTool {
    descriptor: ToolDescriptor,
    capability: ToolCapability,
}

impl ToolRegistry {
    /// Resolve only a currently executable registration, without deriving policy.
    pub(crate) fn declared_capability(&self, name: &str) -> Option<ToolCapability> {
        self.entries
            .iter()
            .find_map(|entry| (entry.descriptor.name == name).then_some(entry.capability))
    }

    pub fn capability(&self, name: &str) -> Option<ToolCapability> {
        self.entries.iter().find_map(|entry| {
            let tool = &entry.descriptor;
            (tool.name == name
                && tool.implemented
                && tool.enabled
                && tool.exposed
                && tool.readiness == ToolReadiness::Ready)
                .then_some(entry.capability)
        })
    }
    /// Build the Normal Mode catalog in its stable, protocol-visible order.
    pub fn normal_mode() -> Self {
        Self {
            entries: vec![
                RegisteredTool {
                    capability: ToolCapability::WorkspaceRead,
                    descriptor: ToolDescriptor {
                        name: "read_file".into(),
                        description: "Read a workspace file.".into(),
                        implemented: true,
                        enabled: true,
                        exposed: true,
                        effective_policy: PolicyDecision::Allow,
                        scope: WORKSPACE_SCOPE.into(),
                        readiness: ToolReadiness::Ready,
                        risk_level: RiskLevel::Low,
                    },
                },
                RegisteredTool {
                    capability: ToolCapability::WorkspaceRead,
                    descriptor: ToolDescriptor {
                        name: "glob".into(),
                        description: "Find workspace paths.".into(),
                        implemented: true,
                        enabled: true,
                        exposed: true,
                        effective_policy: PolicyDecision::Allow,
                        scope: WORKSPACE_SCOPE.into(),
                        readiness: ToolReadiness::Ready,
                        risk_level: RiskLevel::Low,
                    },
                },
            ],
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_metadata(mut self, index: usize, description: &str) -> Self {
        self.entries[index].descriptor.description = description.into();
        self
    }

    #[cfg(test)]
    pub(crate) fn with_test_scope(mut self, index: usize, scope: &str) -> Self {
        self.entries[index].descriptor.scope = scope.into();
        self
    }

    pub fn list(&self) -> Vec<ToolDescriptor> {
        self.entries
            .iter()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    pub fn exposed_catalog(&self) -> Vec<ToolDescriptor> {
        self.entries
            .iter()
            .map(|entry| &entry.descriptor)
            .filter(|descriptor| descriptor.exposed)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mode_registry_contains_only_the_v1_tools() {
        let registry = ToolRegistry::normal_mode();

        assert_eq!(
            registry
                .list()
                .iter()
                .map(|descriptor| descriptor.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob"]
        );
        let forbidden = [
            "bash",
            "tree",
            "list_files",
            "git",
            "database",
            "write",
            "edit",
        ];
        assert!(!registry
            .list()
            .iter()
            .any(|descriptor| forbidden.contains(&descriptor.name.as_str())));
    }

    #[test]
    fn normal_mode_registry_order_is_stable() {
        let first = ToolRegistry::normal_mode().list();
        let second = ToolRegistry::normal_mode().list();

        assert_eq!(first, second);
    }

    #[test]
    fn normal_mode_descriptors_report_read_file_and_glob_as_ready() {
        let descriptors = ToolRegistry::normal_mode().list();
        let read_file = &descriptors[0];
        let glob = &descriptors[1];

        assert!(read_file.implemented);
        assert!(read_file.enabled);
        assert!(read_file.exposed);
        assert_eq!(read_file.effective_policy, PolicyDecision::Allow);
        assert_eq!(read_file.scope, WORKSPACE_SCOPE);
        assert_eq!(read_file.readiness, ToolReadiness::Ready);
        assert_eq!(read_file.risk_level, RiskLevel::Low);

        assert!(glob.implemented);
        assert!(glob.enabled);
        assert!(glob.exposed);
        assert_eq!(glob.effective_policy, PolicyDecision::Allow);
        assert_eq!(glob.scope, WORKSPACE_SCOPE);
        assert_eq!(glob.readiness, ToolReadiness::Ready);
        assert_eq!(glob.risk_level, RiskLevel::Low);
    }
    #[test]
    fn capability_comes_from_the_same_registered_entry() {
        let registry = ToolRegistry::normal_mode();
        for descriptor in registry.list() {
            assert_eq!(
                registry.capability(&descriptor.name),
                Some(ToolCapability::WorkspaceRead)
            );
        }
        assert_eq!(registry.capability("unknown"), None);
        assert_eq!(registry.capability("bash"), None);
    }

    #[test]
    fn capability_lookup_rejects_each_unavailable_registration_state() {
        for state in 0..4 {
            let mut registry = ToolRegistry::normal_mode();
            let descriptor = &mut registry.entries[0].descriptor;
            match state {
                0 => descriptor.implemented = false,
                1 => descriptor.enabled = false,
                2 => descriptor.exposed = false,
                3 => descriptor.readiness = ToolReadiness::Unavailable,
                _ => unreachable!(),
            }
            assert_eq!(registry.capability("read_file"), None);
        }
    }

    #[test]
    fn capability_metadata_does_not_use_the_placeholder_effective_policy() {
        let mut registry = ToolRegistry::normal_mode();
        registry.entries[0].descriptor.effective_policy = PolicyDecision::Deny;
        assert_eq!(
            registry.capability("read_file"),
            Some(ToolCapability::WorkspaceRead)
        );
    }
}
