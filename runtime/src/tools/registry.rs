use harness_control_protocol::{PolicyDecision, RiskLevel, ToolDescriptor, ToolReadiness};

const WORKSPACE_SCOPE: &str = "workspace";

/// The Runtime's single source of truth for tool implementation and exposure state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRegistry {
    descriptors: Vec<ToolDescriptor>,
}

impl ToolRegistry {
    /// Build the Normal Mode catalog in its stable, protocol-visible order.
    pub fn normal_mode() -> Self {
        Self {
            descriptors: vec![
                ToolDescriptor {
                    name: "read_file".into(),
                    description: "Read a workspace file.".into(),
                    implemented: false,
                    enabled: true,
                    exposed: true,
                    effective_policy: PolicyDecision::Allow,
                    scope: WORKSPACE_SCOPE.into(),
                    readiness: ToolReadiness::Unavailable,
                    risk_level: RiskLevel::Low,
                },
                ToolDescriptor {
                    name: "glob".into(),
                    description: "Find workspace paths.".into(),
                    implemented: false,
                    enabled: true,
                    exposed: true,
                    effective_policy: PolicyDecision::Allow,
                    scope: WORKSPACE_SCOPE.into(),
                    readiness: ToolReadiness::Unavailable,
                    risk_level: RiskLevel::Low,
                },
            ],
        }
    }

    pub fn list(&self) -> Vec<ToolDescriptor> {
        self.descriptors.clone()
    }

    pub fn exposed_catalog(&self) -> Vec<ToolDescriptor> {
        self.descriptors
            .iter()
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
    fn normal_mode_descriptors_report_real_unavailable_readiness() {
        let registry = ToolRegistry::normal_mode();

        for descriptor in registry.list() {
            assert!(!descriptor.implemented);
            assert!(descriptor.enabled);
            assert!(descriptor.exposed);
            assert_eq!(descriptor.effective_policy, PolicyDecision::Allow);
            assert_eq!(descriptor.scope, WORKSPACE_SCOPE);
            assert_eq!(descriptor.readiness, ToolReadiness::Unavailable);
            assert_eq!(descriptor.risk_level, RiskLevel::Low);
        }
    }
}
