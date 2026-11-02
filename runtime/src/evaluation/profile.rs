use super::mcp_client::exposed_tool_names;
use super::{EvalError, EvalResult, BASELINE_TOOLS};

pub(crate) const M05_CANDIDATE_PROFILE_ID: &str = "m05-grep-candidate-v1";
pub(crate) const M05_PAIRED_CONTROL_PROFILE_ID: &str = "m05-paired-control-v1";
pub(crate) const M05_CANDIDATE_TOOLS: [&str; 5] = ["read_file", "glob", "grep", "write", "edit"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvalProfile {
    M04Baseline,
    M05GrepCandidate,
    M05PairedControl,
}

impl EvalProfile {
    pub(crate) fn parse(value: &str) -> EvalResult<Self> {
        match value {
            "m04-baseline" => Ok(Self::M04Baseline),
            M05_CANDIDATE_PROFILE_ID => Ok(Self::M05GrepCandidate),
            M05_PAIRED_CONTROL_PROFILE_ID => Ok(Self::M05PairedControl),
            _ => Err(EvalError::new(
                "INVALID_ARGUMENT",
                format!("unknown eval profile '{value}'"),
            )),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::M04Baseline => "m04-baseline",
            Self::M05GrepCandidate => M05_CANDIDATE_PROFILE_ID,
            Self::M05PairedControl => M05_PAIRED_CONTROL_PROFILE_ID,
        }
    }

    pub(crate) fn tool_names(self) -> &'static [&'static str] {
        match self {
            Self::M04Baseline | Self::M05PairedControl => &BASELINE_TOOLS,
            Self::M05GrepCandidate => &M05_CANDIDATE_TOOLS,
        }
    }

    pub(crate) fn grep_enabled(self) -> bool {
        matches!(self, Self::M05GrepCandidate)
    }

    pub(crate) fn validate_catalog(self, tools_result: &serde_json::Value) -> EvalResult<()> {
        let names = exposed_tool_names(tools_result)?;
        let expected = self
            .tool_names()
            .iter()
            .map(|tool| (*tool).to_owned())
            .collect::<Vec<_>>();
        if names != expected {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!(
                    "{} tools/list catalog drifted: expected {expected:?}, observed {names:?}",
                    self.id()
                ),
            ));
        }
        if names.iter().any(|name| name == "bash") {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("bash is forbidden in {}", self.id()),
            ));
        }
        if !self.grep_enabled() && names.iter().any(|name| name == "grep") {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("grep is forbidden in {}", self.id()),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(names: &[&str]) -> serde_json::Value {
        json!({
            "tools": names.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()
        })
    }

    #[test]
    fn baseline_and_paired_control_keep_the_frozen_four_tool_catalog() {
        let baseline = catalog(&BASELINE_TOOLS);
        assert!(EvalProfile::M04Baseline.validate_catalog(&baseline).is_ok());
        assert!(EvalProfile::M05PairedControl
            .validate_catalog(&baseline)
            .is_ok());
        let with_grep = catalog(&M05_CANDIDATE_TOOLS);
        assert_eq!(
            EvalProfile::M04Baseline
                .validate_catalog(&with_grep)
                .unwrap_err()
                .code,
            "HARNESS_CONTRACT_ERROR"
        );
        assert_eq!(
            EvalProfile::M05PairedControl
                .validate_catalog(&with_grep)
                .unwrap_err()
                .code,
            "HARNESS_CONTRACT_ERROR"
        );
    }

    #[test]
    fn candidate_requires_exact_five_tool_catalog_without_bash() {
        assert!(EvalProfile::M05GrepCandidate
            .validate_catalog(&catalog(&M05_CANDIDATE_TOOLS))
            .is_ok());
        for drift in [
            catalog(&BASELINE_TOOLS),
            catalog(&["read_file", "glob", "grep", "write", "edit", "bash"]),
            catalog(&["read_file", "glob", "grep", "write"]),
        ] {
            assert_eq!(
                EvalProfile::M05GrepCandidate
                    .validate_catalog(&drift)
                    .unwrap_err()
                    .code,
                "HARNESS_CONTRACT_ERROR"
            );
        }
    }

    #[test]
    fn profile_ids_are_versioned_and_parse_strictly() {
        for profile in [
            EvalProfile::M04Baseline,
            EvalProfile::M05GrepCandidate,
            EvalProfile::M05PairedControl,
        ] {
            assert_eq!(EvalProfile::parse(profile.id()).unwrap(), profile);
        }
        assert!(EvalProfile::parse("m05-latest").is_err());
    }
}
