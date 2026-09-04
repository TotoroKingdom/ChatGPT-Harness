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
                    implemented: true,
                    enabled: true,
                    exposed: true,
                    effective_policy: PolicyDecision::Allow,
                    scope: WORKSPACE_SCOPE.into(),
                    readiness: ToolReadiness::Ready,
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
    fn normal_mode_descriptors_report_read_file_as_ready_and_glob_as_unavailable() {
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

        assert!(!glob.implemented);
        assert!(glob.enabled);
        assert!(glob.exposed);
        assert_eq!(glob.effective_policy, PolicyDecision::Allow);
        assert_eq!(glob.scope, WORKSPACE_SCOPE);
        assert_eq!(glob.readiness, ToolReadiness::Unavailable);
        assert_eq!(glob.risk_level, RiskLevel::Low);
    }
}
