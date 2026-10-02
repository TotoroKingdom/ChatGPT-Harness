use super::{ExecutionError, ExecutionPermit};
use crate::tools::{
    execute_edit, execute_glob, execute_read_file, execute_write, validate_glob_pattern, EditError,
    EditOutput, EditRequest, GlobOutput, GlobRequest, ReadFileOutput, ReadFileRequest, WriteError,
    WriteOutput, WriteRequest,
};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) enum WorkspaceTarget<'a> {
    ExistingPath(&'a Path),
    WriteTarget(&'a Path),
}

/// Runtime-generated facts used to build a sanitized approval projection and
/// an exact internal request binding. The fingerprint is never sent to the
/// control plane or written to activity evidence.
pub(crate) struct ToolApprovalFacts {
    pub(crate) sanitized_arguments: BTreeMap<String, String>,
    pub(crate) scope_summary: String,
    pub(crate) fingerprint: String,
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
        })
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
        })
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
        })
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
        })
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

fn logical_path(path: std::path::PathBuf) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        ".".into()
    } else {
        value
    }
}
