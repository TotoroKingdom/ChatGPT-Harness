use super::{ExecutionError, ExecutionPermit};
use crate::tools::{
    execute_edit, execute_glob, execute_read_file, execute_write, validate_glob_pattern, EditError,
    EditOutput, EditRequest, GlobOutput, GlobRequest, ReadFileOutput, ReadFileRequest, WriteError,
    WriteOutput, WriteRequest,
};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use std::path::Path;

pub(crate) enum WorkspaceTarget<'a> {
    ExistingPath(&'a Path),
    WriteTarget(&'a Path),
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
