use harness_control_protocol::{
    PolicyDecision, RiskLevel, ToolConfig, ToolDescriptor, ToolReadiness,
};
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
    WorkspaceWrite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegisteredTool {
    descriptor: ToolDescriptor,
    capability: ToolCapability,
}

impl ToolRegistry {
    /// Resolve a declared capability without deriving policy or executable state.
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

    pub(crate) fn is_disabled(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.descriptor.name == name)
            .is_some_and(|entry| !entry.descriptor.enabled)
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
                RegisteredTool {
                    capability: ToolCapability::WorkspaceWrite,
                    descriptor: ToolDescriptor {
                        name: "write".into(),
                        description: "Write a workspace file.".into(),
                        implemented: true,
                        enabled: true,
                        exposed: true,
                        effective_policy: PolicyDecision::Allow,
                        scope: WORKSPACE_SCOPE.into(),
                        readiness: ToolReadiness::Ready,
                        risk_level: RiskLevel::Medium,
                    },
                },
                RegisteredTool {
                    capability: ToolCapability::WorkspaceWrite,
                    descriptor: ToolDescriptor {
                        name: "edit".into(),
                        description: "Edit a workspace text file.".into(),
                        implemented: true,
                        enabled: true,
                        exposed: true,
                        effective_policy: PolicyDecision::Allow,
                        scope: WORKSPACE_SCOPE.into(),
                        readiness: ToolReadiness::Ready,
                        risk_level: RiskLevel::Medium,
                    },
                },
            ],
        }
    }

    /// Build a generation-specific registry from the immutable Runtime
    /// configuration snapshot. Built-in definitions and order remain owned by
    /// this registry; configuration only supplies the desired enabled state.
    pub fn from_config(config: &ToolConfig) -> Self {
        let mut registry = Self::normal_mode();
        for (name, enabled) in [
            ("read_file", config.read_file.enabled),
            ("glob", config.glob.enabled),
            ("write", config.write.enabled),
            ("edit", config.edit.enabled),
        ] {
            if let Some(entry) = registry
                .entries
                .iter_mut()
                .find(|entry| entry.descriptor.name == name)
            {
                entry.descriptor.enabled = enabled;
            }
        }
        // `bash` is intentionally a persisted future field only. It is not
        // registered here until its M5 implementation exists.
        registry
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
    #[cfg(test)]
    pub(crate) fn with_test_enabled(mut self, index: usize, enabled: bool) -> Self {
        self.entries[index].descriptor.enabled = enabled;
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
    fn normal_mode_registry_contains_the_stable_m0_tool_order() {
        let registry = ToolRegistry::normal_mode();

        assert_eq!(
            registry
                .list()
                .iter()
                .map(|descriptor| descriptor.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit"]
        );
        let forbidden = ["bash", "tree", "list_files", "git", "database"];
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
    fn normal_mode_descriptors_report_read_and_write_tools_as_ready() {
        let descriptors = ToolRegistry::normal_mode().list();
        let read_file = &descriptors[0];
        let glob = &descriptors[1];
        let write = &descriptors[2];
        let edit = &descriptors[3];

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

        for descriptor in [write, edit] {
            assert!(descriptor.implemented);
            assert!(descriptor.enabled);
            assert!(descriptor.exposed);
            assert_eq!(descriptor.effective_policy, PolicyDecision::Allow);
            assert_eq!(descriptor.scope, WORKSPACE_SCOPE);
            assert_eq!(descriptor.readiness, ToolReadiness::Ready);
            assert_eq!(descriptor.risk_level, RiskLevel::Medium);
        }
    }
    #[test]
    fn capability_comes_from_the_same_registered_entry() {
        let registry = ToolRegistry::normal_mode();
        assert_eq!(
            registry.capability("read_file"),
            Some(ToolCapability::WorkspaceRead)
        );
        assert_eq!(
            registry.capability("glob"),
            Some(ToolCapability::WorkspaceRead)
        );
        assert_eq!(
            registry.capability("write"),
            Some(ToolCapability::WorkspaceWrite)
        );
        assert_eq!(
            registry.capability("edit"),
            Some(ToolCapability::WorkspaceWrite)
        );
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

    #[test]
    fn configured_registry_preserves_order_and_declared_capability_when_disabled() {
        let mut config = ToolConfig::default();
        config.write.enabled = false;
        let registry = ToolRegistry::from_config(&config);

        assert_eq!(
            registry
                .list()
                .iter()
                .map(|descriptor| descriptor.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit"]
        );
        assert_eq!(
            registry.declared_capability("write"),
            Some(ToolCapability::WorkspaceWrite)
        );
        assert_eq!(registry.capability("write"), None);
        assert!(!registry.list()[2].enabled);
    }

    #[test]
    fn each_implemented_tool_can_be_disabled_independently() {
        for (index, name) in ["read_file", "glob", "write", "edit"].iter().enumerate() {
            let mut config = ToolConfig::default();
            match index {
                0 => config.read_file.enabled = false,
                1 => config.glob.enabled = false,
                2 => config.write.enabled = false,
                3 => config.edit.enabled = false,
                _ => unreachable!(),
            }
            let registry = ToolRegistry::from_config(&config);
            assert!(!registry.list()[index].enabled, "{name} remained enabled");
            assert_eq!(registry.capability(name), None);
            assert!(registry.declared_capability(name).is_some());
            assert!(registry
                .list()
                .iter()
                .enumerate()
                .all(|(other_index, tool)| other_index == index || tool.enabled));
        }
    }

    #[test]
    fn bash_desired_state_does_not_register_an_executor_or_catalog_entry() {
        let mut config = ToolConfig::default();
        config.bash.enabled = true;
        let registry = ToolRegistry::from_config(&config);

        assert_eq!(registry.capability("bash"), None);
        assert!(!registry.list().iter().any(|tool| tool.name == "bash"));
    }
}
