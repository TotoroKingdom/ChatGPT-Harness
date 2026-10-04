use super::{ExecutionError, ExecutionPermit};
use crate::bash::{
    analyze, validate_command, BashClassification, BashCommandAnalysis,
    BashRequest as RuntimeBashRequest, BashRuntime,
};
use crate::policy::PolicyContext;
use crate::tools::{
    execute_edit, execute_glob, execute_read_file, execute_write, validate_glob_pattern, EditError,
    EditOutput, EditRequest, GlobOutput, GlobRequest, ReadFileOutput, ReadFileRequest, WriteError,
    WriteOutput, WriteRequest,
};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use harness_control_protocol::{
    BashApprovalDetails, BashApprovalSegment, BashRiskFlag as WireBashRiskFlag,
    BashWorkingDirectory,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) enum WorkspaceTarget<'a> {
    ExistingPath(&'a Path),
    WriteTarget(&'a Path),
    WorkspaceRoot,
}

/// Runtime-generated facts used to build a sanitized approval projection and
/// an exact internal request binding. The fingerprint is never sent to the
/// control plane or written to activity evidence.
pub(crate) struct ToolApprovalFacts {
    pub(crate) sanitized_arguments: BTreeMap<String, String>,
    pub(crate) scope_summary: String,
    pub(crate) fingerprint: String,
    /// Exact normalized scope used only for SessionGrant matching.  It must
    /// never be copied into ApprovalRequest or Activity/Audit projections.
    pub(crate) grant_scope: Option<String>,
    pub(crate) bash_details: Option<BashApprovalDetails>,
    pub(crate) allow_for_session_eligible: Option<bool>,
}

/// Typed Bash request owned by the Pipeline adapter.  The original command is
/// retained for the final Runtime invocation while the analyzer facts are
/// carried separately into policy/approval evaluation.  The Runtime request
/// passed to BashRuntime is reconstructed from `command` unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashToolRequest {
    command: String,
    analysis: BashCommandAnalysis,
}

impl BashToolRequest {
    pub(crate) fn new(command: String) -> Self {
        // Reject oversized/NUL/blank input before feeding it to the analyzer.
        // The analyzer is intentionally bounded for normal commands, but MCP
        // input validation must not allocate a token vector for an attacker
        // supplied oversized string.
        let analysis = if validate_command(&command).is_ok() {
            analyze(&command)
        } else {
            analyze("")
        };
        Self { command, analysis }
    }
}

/// Each adapter provides typed validation and execution, never authorization.
/// The pipeline always validates its workspace target before policy evaluation.
/// The permit is only created by the pipeline after ALLOW; it cannot be forged
/// by an MCP handler or RuntimeCore helper.
pub(crate) trait ToolExecutor {
    type Output;
    const TOOL_NAME: &'static str;

    fn validate(&self) -> Result<(), ExecutionError>;
    fn workspace_target(&self) -> WorkspaceTarget<'_>;
    fn policy_context<'a>(&'a self, security: &super::SecurityContext) -> PolicyContext<'a> {
        PolicyContext::new(Self::TOOL_NAME, security.mode.clone(), "active_workspace")
    }
    fn approval_facts(&self, _guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        Err(ExecutionError::ApprovalRequired)
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError;
    fn execute(
        &self,
        permit: &ExecutionPermit,
        guard: &WorkspaceGuard,
        max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError>;

    fn arguments_evidence(
        &self,
        guard: &WorkspaceGuard,
    ) -> Option<harness_control_protocol::SanitizedArgumentsEvidence> {
        use harness_control_protocol::SanitizedArgumentsEvidence as E;
        let facts = self.approval_facts(guard).ok()?;
        let field = |key| facts.sanitized_arguments.get(key).cloned();
        match Self::TOOL_NAME {
            "read_file" => E::ReadFile {
                logical_path: field("path")?,
            },
            "glob" => E::Glob {
                pattern: field("pattern")?,
            },
            "write" => E::Write {
                logical_path: field("path")?,
            },
            "edit" => E::Edit {
                logical_path: field("path")?,
            },
            "bash" => E::Bash {
                command_preview: field("command")?,
            },
            _ => return None,
        }
        .sanitized()
    }

    fn record_output(&self, _context: &mut super::RequestContext, _output: &Self::Output) {}

    /// File tools retain their existing synchronous implementation.  Bash
    /// overrides this seam with a cancellation-aware async process wait.
    fn execute_async<'a>(
        &'a self,
        permit: &'a ExecutionPermit,
        guard: &'a WorkspaceGuard,
        max_read_bytes: u32,
        _bash_runtime: Option<Arc<BashRuntime>>,
        _cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, ExecutionError>> + Send + 'a>>
    where
        Self: Sync,
        Self::Output: Send + 'a,
    {
        Box::pin(async move { self.execute(permit, guard, max_read_bytes) })
    }
}

impl ToolExecutor for ReadFileRequest {
    type Output = ReadFileOutput;
    const TOOL_NAME: &'static str = "read_file";

    fn validate(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::ExistingPath(&self.path)
    }
    fn approval_facts(&self, guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        let approved = guard.approve(&self.path).map_err(|error| {
            ExecutionError::ReadFile(crate::tools::ReadFileError::Workspace(error))
        })?;
        let scope = logical_path(guard.logical_relative_path(&self.path, &approved));
        let mut sanitized_arguments = BTreeMap::new();
        sanitized_arguments.insert("path".into(), scope.clone());
        sanitized_arguments.insert("offset".into(), self.offset.to_string());
        if let Some(limit) = self.limit {
            sanitized_arguments.insert("limit".into(), limit.to_string());
        }
        Ok(ToolApprovalFacts {
            sanitized_arguments,
            scope_summary: scope.clone(),
            fingerprint: serde_json::json!({
                "tool": Self::TOOL_NAME,
                "scope": scope,
                "offset": self.offset,
                "limit": self.limit,
            })
            .to_string(),
            grant_scope: Some(scope),
            bash_details: None,
            allow_for_session_eligible: None,
        })
    }
    fn record_output(&self, context: &mut super::RequestContext, output: &Self::Output) {
        context.result_evidence = Some(
            harness_control_protocol::ExecutionResultEvidence::ReadFile {
                bytes_read: output.bytes_read,
            },
        );
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::ReadFile(crate::tools::ReadFileError::Workspace(error))
    }
    fn execute(
        &self,
        permit: &ExecutionPermit,
        guard: &WorkspaceGuard,
        max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError> {
        execute_read_file(permit, guard, max_read_bytes, self).map_err(ExecutionError::ReadFile)
    }
}

impl ToolExecutor for GlobRequest {
    type Output = GlobOutput;
    const TOOL_NAME: &'static str = "glob";

    fn validate(&self) -> Result<(), ExecutionError> {
        validate_glob_pattern(&self.pattern).map_err(ExecutionError::Glob)
    }
    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::ExistingPath(self.base_path.as_deref().unwrap_or_else(|| Path::new(".")))
    }
    fn approval_facts(&self, guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        let requested_base = self.base_path.as_deref().unwrap_or_else(|| Path::new("."));
        let approved = guard
            .approve(requested_base)
            .map_err(|error| ExecutionError::Glob(crate::tools::GlobError::Workspace(error)))?;
        let base = logical_path(guard.logical_relative_path(requested_base, &approved));
        let scope = if base == "." {
            format!("pattern:{}", self.pattern)
        } else {
            format!("base:{};pattern:{}", base, self.pattern)
        };
        let mut sanitized_arguments = BTreeMap::new();
        sanitized_arguments.insert("pattern".into(), self.pattern.clone());
        sanitized_arguments.insert(
            "basePath".into(),
            if base == "." {
                ".".into()
            } else {
                base.clone()
            },
        );
        Ok(ToolApprovalFacts {
            sanitized_arguments,
            scope_summary: scope.clone(),
            fingerprint: serde_json::json!({
                "tool": Self::TOOL_NAME,
                "basePath": base,
                "pattern": self.pattern,
            })
            .to_string(),
            grant_scope: Some(scope),
            bash_details: None,
            allow_for_session_eligible: None,
        })
    }
    fn record_output(&self, context: &mut super::RequestContext, output: &Self::Output) {
        context.result_evidence = Some(harness_control_protocol::ExecutionResultEvidence::Glob {
            match_count: output.matches.len() as u64,
        });
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::Glob(crate::tools::GlobError::Workspace(error))
    }
    fn execute(
        &self,
        permit: &ExecutionPermit,
        guard: &WorkspaceGuard,
        _max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError> {
        execute_glob(permit, guard, self).map_err(ExecutionError::Glob)
    }
}

impl ToolExecutor for WriteRequest {
    type Output = WriteOutput;
    const TOOL_NAME: &'static str = "write";

    fn validate(&self) -> Result<(), ExecutionError> {
        if self.path.as_os_str().is_empty() {
            return Err(ExecutionError::Write(WriteError::InvalidArguments {
                message: "path must not be empty".into(),
            }));
        }
        Ok(())
    }

    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::WriteTarget(&self.path)
    }
    fn approval_facts(&self, guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        let approved = guard
            .approve_write_target(&self.path)
            .map_err(|error| ExecutionError::Write(WriteError::Workspace(error)))?;
        let scope = logical_path(guard.logical_relative_write_path(&self.path, &approved));
        let mut sanitized_arguments = BTreeMap::new();
        sanitized_arguments.insert("path".into(), scope.clone());
        sanitized_arguments.insert("content".into(), "<redacted content>".into());
        Ok(ToolApprovalFacts {
            sanitized_arguments,
            scope_summary: scope.clone(),
            fingerprint: serde_json::json!({
                "tool": Self::TOOL_NAME,
                "scope": scope,
                "content": self.content,
            })
            .to_string(),
            grant_scope: Some(scope),
            bash_details: None,
            allow_for_session_eligible: None,
        })
    }

    fn record_output(&self, context: &mut super::RequestContext, output: &Self::Output) {
        context.result_evidence = Some(harness_control_protocol::ExecutionResultEvidence::Write {
            bytes_written: output.bytes_written,
            disposition: if output.created {
                harness_control_protocol::WriteDisposition::Created
            } else {
                harness_control_protocol::WriteDisposition::Overwritten
            },
        });
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::Write(WriteError::Workspace(error))
    }

    fn execute(
        &self,
        permit: &ExecutionPermit,
        guard: &WorkspaceGuard,
        _max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError> {
        execute_write(permit, guard, self).map_err(ExecutionError::Write)
    }
}

impl ToolExecutor for EditRequest {
    type Output = EditOutput;
    const TOOL_NAME: &'static str = "edit";

    fn validate(&self) -> Result<(), ExecutionError> {
        if self.path.as_os_str().is_empty() {
            return Err(ExecutionError::Edit(EditError::InvalidArguments {
                message: "path must not be empty".into(),
            }));
        }
        if self.old_text.is_empty() {
            return Err(ExecutionError::Edit(EditError::InvalidArguments {
                message: "old_text must not be empty".into(),
            }));
        }
        Ok(())
    }

    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::ExistingPath(&self.path)
    }
    fn approval_facts(&self, guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        let approved = guard
            .approve(&self.path)
            .map_err(|error| ExecutionError::Edit(EditError::Workspace(error)))?;
        let scope = logical_path(guard.logical_relative_path(&self.path, &approved));
        let mut sanitized_arguments = BTreeMap::new();
        sanitized_arguments.insert("path".into(), scope.clone());
        sanitized_arguments.insert("oldText".into(), "<redacted text>".into());
        sanitized_arguments.insert("newText".into(), "<redacted text>".into());
        Ok(ToolApprovalFacts {
            sanitized_arguments,
            scope_summary: scope.clone(),
            fingerprint: serde_json::json!({
                "tool": Self::TOOL_NAME,
                "scope": scope,
                "oldText": self.old_text,
                "newText": self.new_text,
            })
            .to_string(),
            grant_scope: Some(scope),
            bash_details: None,
            allow_for_session_eligible: None,
        })
    }

    fn record_output(&self, context: &mut super::RequestContext, output: &Self::Output) {
        context.result_evidence = Some(harness_control_protocol::ExecutionResultEvidence::Edit {
            replacements: output.replacements,
            bytes_written: output.bytes_written,
        });
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::Edit(EditError::Workspace(error))
    }

    fn execute(
        &self,
        permit: &ExecutionPermit,
        guard: &WorkspaceGuard,
        _max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError> {
        execute_edit(permit, guard, self).map_err(ExecutionError::Edit)
    }
}

impl ToolExecutor for BashToolRequest {
    type Output = crate::bash::BashOutput;
    const TOOL_NAME: &'static str = "bash";

    fn validate(&self) -> Result<(), ExecutionError> {
        validate_command(&self.command).map_err(ExecutionError::BashRuntime)
    }

    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::WorkspaceRoot
    }

    fn policy_context<'a>(&'a self, security: &super::SecurityContext) -> PolicyContext<'a> {
        PolicyContext::new(Self::TOOL_NAME, security.mode.clone(), "active_workspace")
            .with_bash_analysis(&self.analysis)
    }

    fn approval_facts(&self, _guard: &WorkspaceGuard) -> Result<ToolApprovalFacts, ExecutionError> {
        let analysis = &self.analysis;
        let preview = analysis.sanitized_preview();
        let sanitized_scope = analysis.sanitized_scope();
        let display_complete = analysis.sanitized_scope_is_complete();
        let preview_truncated =
            preview.ends_with('…') || sanitized_scope == r#"{"version":1,"redacted":true}"#;
        let preview_redacted = !display_complete || preview_truncated;
        let classification = match analysis.classification {
            BashClassification::Classified => {
                harness_control_protocol::BashClassification::Classified
            }
            BashClassification::UnclassifiedHighRisk => {
                harness_control_protocol::BashClassification::UnclassifiedHighRisk
            }
        };
        let risk_flags = analysis
            .risk_flags()
            .into_iter()
            .map(|flag| match flag {
                crate::bash::BashRiskFlag::CommandSubstitution => {
                    WireBashRiskFlag::CommandSubstitution
                }
                crate::bash::BashRiskFlag::Backticks => WireBashRiskFlag::Backticks,
                crate::bash::BashRiskFlag::Eval => WireBashRiskFlag::Eval,
                crate::bash::BashRiskFlag::NestedShell => WireBashRiskFlag::NestedShell,
                crate::bash::BashRiskFlag::InterpreterInlineCode => {
                    WireBashRiskFlag::InterpreterInlineCode
                }
                crate::bash::BashRiskFlag::Redirection => WireBashRiskFlag::Redirection,
                crate::bash::BashRiskFlag::DynamicExpansion => WireBashRiskFlag::DynamicExpansion,
                crate::bash::BashRiskFlag::MalformedQuoting => WireBashRiskFlag::MalformedQuoting,
                crate::bash::BashRiskFlag::UnsupportedGrammar => {
                    WireBashRiskFlag::UnsupportedGrammar
                }
            })
            .collect();
        let normalized_segments = sanitized_segments(analysis);
        let bash_details = BashApprovalDetails {
            command_preview: preview,
            classification: classification.clone(),
            normalized_segments,
            risk_flags,
            cwd: BashWorkingDirectory::ActiveWorkspace,
            preview_redacted,
            preview_truncated,
        };
        let classified = matches!(
            classification,
            harness_control_protocol::BashClassification::Classified
        );
        let allow_for_session_eligible = classified && analysis.allow_for_session();
        let mut sanitized_arguments = BTreeMap::new();
        sanitized_arguments.insert("command".into(), bash_details.command_preview.clone());
        sanitized_arguments.insert(
            "classification".into(),
            match bash_details.classification {
                harness_control_protocol::BashClassification::Classified => "classified",
                harness_control_protocol::BashClassification::UnclassifiedHighRisk => {
                    "unclassified_high_risk"
                }
            }
            .into(),
        );
        Ok(ToolApprovalFacts {
            sanitized_arguments,
            scope_summary: sanitized_scope,
            fingerprint: serde_json::json!({
                "tool": Self::TOOL_NAME,
                "command": self.command,
            })
            .to_string(),
            grant_scope: allow_for_session_eligible.then(|| analysis.normalized_scope()),
            bash_details: Some(bash_details),
            allow_for_session_eligible: Some(allow_for_session_eligible),
        })
    }

    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::BashWorkspace(error)
    }

    fn execute(
        &self,
        _permit: &ExecutionPermit,
        _guard: &WorkspaceGuard,
        _max_read_bytes: u32,
    ) -> Result<Self::Output, ExecutionError> {
        Err(ExecutionError::BashRuntime(
            crate::bash::BashRuntimeError::ProcessFailed,
        ))
    }

    fn record_output(&self, context: &mut super::RequestContext, output: &Self::Output) {
        context.set_bash_result(output.exit_code, output.duration_ms);
        context.result_evidence = Some(harness_control_protocol::ExecutionResultEvidence::Bash {
            exit_code: output.exit_code,
        });
    }

    fn execute_async<'a>(
        &'a self,
        permit: &'a ExecutionPermit,
        _guard: &'a WorkspaceGuard,
        _max_read_bytes: u32,
        bash_runtime: Option<Arc<BashRuntime>>,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, ExecutionError>> + Send + 'a>>
    where
        Self: Sync,
        Self::Output: Send + 'a,
    {
        let runtime = bash_runtime.ok_or(ExecutionError::BashRuntime(
            crate::bash::BashRuntimeError::Unavailable,
        ));
        let request = self.clone();
        Box::pin(async move {
            let runtime = runtime?;
            runtime
                .execute(
                    permit,
                    RuntimeBashRequest {
                        command: request.command,
                    },
                    cancel,
                )
                .await
                .map_err(ExecutionError::BashRuntime)
        })
    }
}

fn sanitized_segments(analysis: &BashCommandAnalysis) -> Vec<BashApprovalSegment> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&analysis.sanitized_scope()) else {
        return Vec::new();
    };
    let Some(display_segments) = value.get("segments").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    display_segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let executable = segment.get("executable")?.as_str()?.to_owned();
            let argv = segment
                .get("argv")?
                .as_array()?
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let raw = analysis.segments.get(index);
            let argv_redacted = raw.is_none_or(|raw| {
                raw.executable != executable
                    || raw.argv.len() != argv.len()
                    || raw
                        .argv
                        .iter()
                        .zip(&argv)
                        .any(|(raw, display)| raw != display)
            });
            Some(BashApprovalSegment {
                executable,
                argv,
                argv_redacted,
            })
        })
        .collect()
}

fn logical_path(path: std::path::PathBuf) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        ".".into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    #[test]
    fn bash_approval_projection_redacts_sensitive_argv_without_changing_private_scope() {
        let root = std::env::temp_dir().join(format!(
            "chatgpt-harness-bash-approval-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("workspace");
        let workspace = Workspace::new(&root).expect("valid workspace");
        let guard = WorkspaceGuard::new(&workspace);
        let request = BashToolRequest::new("git --token=SECRET_VALUE status".into());
        let facts = request.approval_facts(&guard).expect("approval facts");
        let details = facts.bash_details.expect("Bash details");
        let serialized = serde_json::to_string(&details).expect("serialize details");
        assert!(!serialized.contains("SECRET_VALUE"));
        assert!(details.preview_redacted);
        assert!(facts
            .grant_scope
            .as_deref()
            .is_some_and(|scope| scope.contains("SECRET_VALUE")));
        let _ = fs::remove_dir_all(root);
    }
}
