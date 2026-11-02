use super::fixture::{fixture_tree_hash, safe_relative};
use super::manifest::canonical_hash;
use super::{
    EvalError, EvalResult, CASES_PER_CATEGORY, CASE_VERSION, CATEGORY_COUNT, MAX_MODEL_TURNS,
    MAX_TOOL_CALLS,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvalCategory {
    ExactRead,
    PathDiscovery,
    ContentSearch,
    WriteState,
    EditState,
    EnforcementAndRecovery,
}

impl EvalCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExactRead => "exact_read",
            Self::PathDiscovery => "path_discovery",
            Self::ContentSearch => "content_search",
            Self::WriteState => "write_state",
            Self::EditState => "edit_state",
            Self::EnforcementAndRecovery => "enforcement_and_recovery",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SelectionOracle {
    pub(crate) strategies: Vec<SelectionStrategy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SelectionStrategy {
    pub(crate) id: String,
    pub(crate) steps: Vec<SelectionStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StepRepeat {
    One,
    OneOrMore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SelectionStep {
    pub(crate) tool: String,
    #[serde(default)]
    pub(crate) optional: bool,
    #[serde(default = "default_repeat")]
    pub(crate) repeat: StepRepeat,
}

fn default_repeat() -> StepRepeat {
    StepRepeat::One
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArgumentConstraint {
    pub(crate) tool: String,
    #[serde(default)]
    pub(crate) path: Option<String>,
    #[serde(default)]
    pub(crate) path_any: Vec<String>,
    #[serde(default)]
    pub(crate) path_suffix: Option<String>,
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) old_text: Option<String>,
    #[serde(default)]
    pub(crate) new_text: Option<String>,
    #[serde(default)]
    pub(crate) pattern: Option<String>,
    #[serde(default)]
    pub(crate) pattern_must_match_any: Vec<String>,
    #[serde(default)]
    pub(crate) optional: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeOutcome {
    Success,
    PolicyDenied,
    ApprovalDenied,
    WorkspaceViolation,
    InvalidArguments,
    TargetNotFound,
    ExecutionFailed,
    Cancelled,
    TimedOut,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExpectedRuntimeOutcome {
    pub(crate) tool: String,
    #[serde(default = "default_occurrence")]
    pub(crate) occurrence: u32,
    pub(crate) outcome: RuntimeOutcome,
}

fn default_occurrence() -> u32 {
    1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PolicyExpectationValue {
    Allow,
    Ask,
    Deny,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PolicyDefaults {
    pub(crate) read_file: PolicyExpectationValue,
    pub(crate) glob: PolicyExpectationValue,
    pub(crate) write: PolicyExpectationValue,
    pub(crate) edit: PolicyExpectationValue,
    pub(crate) bash: PolicyExpectationValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PolicyExpectation {
    pub(crate) defaults: PolicyDefaults,
    #[serde(default)]
    pub(crate) deny_tools: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalPlan {
    None,
    AllowOnce,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum FinalStateAssertion {
    FileExact { path: String, content: String },
    FileExists { path: String },
    FileAbsent { path: String },
    Unchanged { path: String, sha256: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResponseMatchKind {
    Equals,
    Contains,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FinalResponseAssertion {
    pub(crate) kind: ResponseMatchKind,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EvalCaseV1 {
    pub(crate) case_id: String,
    pub(crate) case_version: u32,
    pub(crate) category: EvalCategory,
    pub(crate) prompt: String,
    pub(crate) fixture_ref: String,
    pub(crate) fixture_hash: String,
    #[serde(default)]
    pub(crate) preconditions: Vec<String>,
    pub(crate) selection_oracle: SelectionOracle,
    pub(crate) argument_constraints: Vec<ArgumentConstraint>,
    pub(crate) expected_runtime_outcomes: Vec<ExpectedRuntimeOutcome>,
    pub(crate) policy_expectation: PolicyExpectation,
    pub(crate) approval_plan: ApprovalPlan,
    pub(crate) final_state_assertions: Vec<FinalStateAssertion>,
    #[serde(default)]
    pub(crate) final_response_assertion: Option<FinalResponseAssertion>,
    #[serde(default)]
    pub(crate) max_turns: Option<u32>,
    #[serde(default)]
    pub(crate) max_tool_calls: Option<u32>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

impl EvalCaseV1 {
    pub(crate) fn max_turns(&self) -> u32 {
        self.max_turns.unwrap_or(MAX_MODEL_TURNS)
    }

    pub(crate) fn max_tool_calls(&self) -> u32 {
        self.max_tool_calls.unwrap_or(MAX_TOOL_CALLS)
    }

    pub(crate) fn case_hash(&self) -> EvalResult<String> {
        canonical_hash(
            &serde_json::to_value(self)
                .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))?,
        )
    }

    pub(crate) fn validate(&self, fixture_root: &Path, system_prompt: &str) -> EvalResult<()> {
        if self.case_id.trim().is_empty() {
            return Err(self.invalid("caseId is empty"));
        }
        if self.case_version != CASE_VERSION {
            return Err(self.invalid("unsupported caseVersion"));
        }
        if self.prompt.trim().is_empty() {
            return Err(self.invalid("prompt is empty"));
        }
        if system_prompt.trim().is_empty() {
            return Err(self.invalid("system prompt is empty"));
        }
        let fixture_relative = safe_relative(&self.fixture_ref)
            .map_err(|error| self.invalid(&format!("invalid fixtureRef: {error}")))?;
        let fixture_path = fixture_root.join(fixture_relative);
        if !fixture_path.is_dir() {
            return Err(self.invalid("fixtureRef does not resolve to a directory"));
        }
        let actual_fixture_hash = fixture_tree_hash(&fixture_path)?;
        if actual_fixture_hash != self.fixture_hash {
            return Err(self.invalid("fixtureHash does not match the fixture tree"));
        }
        if self.selection_oracle.strategies.is_empty() {
            return Err(self.invalid("selectionOracle has no strategies"));
        }
        let mut required_tools = Vec::new();
        for strategy in &self.selection_oracle.strategies {
            if strategy.id.trim().is_empty() || strategy.steps.is_empty() {
                return Err(self.invalid("selection strategy id/steps are empty"));
            }
            for step in &strategy.steps {
                if !super::BASELINE_TOOLS.contains(&step.tool.as_str()) {
                    return Err(self.invalid(&format!("unknown strategy tool '{}'", step.tool)));
                }
                if !step.optional {
                    required_tools.push(step.tool.clone());
                }
            }
        }
        for constraint in &self.argument_constraints {
            if !super::BASELINE_TOOLS.contains(&constraint.tool.as_str()) {
                return Err(self.invalid(&format!("unknown constraint tool '{}'", constraint.tool)));
            }
            if constraint.path.is_none()
                && constraint.path_any.is_empty()
                && constraint.path_suffix.is_none()
                && constraint.pattern.is_none()
                && constraint.pattern_must_match_any.is_empty()
            {
                return Err(self.invalid("argument constraint has no path/pattern matcher"));
            }
            if let Some(path) = &constraint.path {
                safe_relative(path).map_err(|error| self.invalid(&error.to_string()))?;
            }
            for path in &constraint.path_any {
                safe_relative(path).map_err(|error| self.invalid(&error.to_string()))?;
            }
            for path in &constraint.pattern_must_match_any {
                safe_relative(path).map_err(|error| self.invalid(&error.to_string()))?;
            }
        }
        for tool in &required_tools {
            if !self
                .argument_constraints
                .iter()
                .any(|constraint| constraint.tool == *tool)
            {
                return Err(self.invalid(&format!(
                    "no argument constraint for required tool '{tool}'"
                )));
            }
        }
        if self.final_state_assertions.is_empty() && self.final_response_assertion.is_none() {
            return Err(self.invalid("case has no final-state or final-response assertion"));
        }
        for assertion in &self.final_state_assertions {
            let path = match assertion {
                FinalStateAssertion::FileExact { path, .. }
                | FinalStateAssertion::FileExists { path }
                | FinalStateAssertion::FileAbsent { path }
                | FinalStateAssertion::Unchanged { path, .. } => path,
            };
            safe_relative(path).map_err(|error| self.invalid(&error.to_string()))?;
        }
        if self.max_turns() == 0 || self.max_turns() > MAX_MODEL_TURNS {
            return Err(self.invalid("maxTurns exceeds the frozen global maximum"));
        }
        if self.max_tool_calls() == 0 || self.max_tool_calls() > MAX_TOOL_CALLS {
            return Err(self.invalid("maxToolCalls exceeds the frozen global maximum"));
        }
        let has_ask = required_tools.iter().any(|tool| {
            matches!(
                self.policy_expectation_value(tool),
                Some(PolicyExpectationValue::Ask)
            )
        });
        if (self.approval_plan == ApprovalPlan::AllowOnce
            || self.approval_plan == ApprovalPlan::Deny)
            && !has_ask
        {
            return Err(self.invalid("approval plan requires an ASK policy expectation"));
        }
        if self.approval_plan == ApprovalPlan::None && has_ask {
            return Err(self.invalid("ASK policy expectation requires an approval plan"));
        }
        Ok(())
    }

    fn invalid(&self, message: &str) -> EvalError {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("case '{}': {message}", self.case_id),
        )
    }

    pub(crate) fn policy_expectation_value(&self, tool: &str) -> Option<PolicyExpectationValue> {
        if self
            .policy_expectation
            .deny_tools
            .iter()
            .any(|denied| denied == tool)
        {
            return Some(PolicyExpectationValue::Deny);
        }
        match tool {
            "read_file" => Some(self.policy_expectation.defaults.read_file),
            "glob" => Some(self.policy_expectation.defaults.glob),
            "write" => Some(self.policy_expectation.defaults.write),
            "edit" => Some(self.policy_expectation.defaults.edit),
            "bash" => Some(self.policy_expectation.defaults.bash),
            _ => None,
        }
    }

    pub(crate) fn score_signature(&self) -> EvalResult<serde_json::Value> {
        Ok(json!({
            "caseHash": self.case_hash()?,
            "selectionOracle": self.selection_oracle,
            "argumentConstraints": self.argument_constraints,
            "expectedRuntimeOutcomes": self.expected_runtime_outcomes,
            "policyExpectation": self.policy_expectation,
            "approvalPlan": self.approval_plan,
            "finalStateAssertions": self.final_state_assertions,
            "finalResponseAssertion": self.final_response_assertion,
        }))
    }
}

pub(crate) fn validate_category_counts(cases: &[EvalCaseV1]) -> EvalResult<()> {
    if cases.len() != super::CASE_COUNT {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("dataset must contain {} cases", super::CASE_COUNT),
        ));
    }
    if CATEGORY_COUNT != 6 || CASES_PER_CATEGORY != 4 {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "frozen category constants drifted",
        ));
    }
    for category in [
        EvalCategory::ExactRead,
        EvalCategory::PathDiscovery,
        EvalCategory::ContentSearch,
        EvalCategory::WriteState,
        EvalCategory::EditState,
        EvalCategory::EnforcementAndRecovery,
    ] {
        let count = cases
            .iter()
            .filter(|case| case.category == category)
            .count();
        if count != CASES_PER_CATEGORY {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("category {} has {count} cases", category.as_str()),
            ));
        }
    }
    Ok(())
}
