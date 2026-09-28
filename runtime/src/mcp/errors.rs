use crate::execution::ExecutionError;
use crate::tools::{EditError, GlobError, ReadFileError, WriteError};
use crate::workspace::WorkspaceError;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpErrorCategory {
    InvalidArguments,
    WorkspaceViolation,
    TargetNotFound,
    TargetNotFile,
    TargetNotDirectory,
    UnsupportedContent,
    ReadLimitExceeded,
    InvalidPattern,
    InvalidBasePath,
    EditMatchNotFound,
    EditMatchNotUnique,
    IoFailure,
    RuntimeNotInitialized,
    RuntimeNotReady,
    PolicyDenied,
    ApprovalRequired,
    ExecutionFailed,
}

impl McpErrorCategory {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidArguments => "INVALID_ARGUMENTS",
            Self::WorkspaceViolation => "WORKSPACE_VIOLATION",
            Self::TargetNotFound => "TARGET_NOT_FOUND",
            Self::TargetNotFile => "TARGET_NOT_FILE",
            Self::TargetNotDirectory => "TARGET_NOT_DIRECTORY",
            Self::UnsupportedContent => "UNSUPPORTED_CONTENT",
            Self::ReadLimitExceeded => "READ_LIMIT_EXCEEDED",
            Self::InvalidPattern => "INVALID_PATTERN",
            Self::InvalidBasePath => "INVALID_BASE_PATH",
            Self::EditMatchNotFound => "EDIT_MATCH_NOT_FOUND",
            Self::EditMatchNotUnique => "EDIT_MATCH_NOT_UNIQUE",
            Self::IoFailure => "IO_FAILURE",
            Self::RuntimeNotInitialized => "RUNTIME_NOT_INITIALIZED",
            Self::RuntimeNotReady => "RUNTIME_NOT_READY",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::ApprovalRequired => "APPROVAL_REQUIRED",
            Self::ExecutionFailed => "EXECUTION_FAILED",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "Tool arguments are invalid.",
            Self::WorkspaceViolation => "Requested path is outside the active workspace.",
            Self::TargetNotFound => "Requested target was not found in the active workspace.",
            Self::TargetNotFile => "Requested target is not a file.",
            Self::TargetNotDirectory => "Requested base path is not a directory.",
            Self::UnsupportedContent => "Requested file is not supported text content.",
            Self::ReadLimitExceeded => "Requested file line exceeds the configured read limit.",
            Self::InvalidPattern => "Glob pattern is invalid.",
            Self::InvalidBasePath => "Glob base path is invalid.",
            Self::EditMatchNotFound => "The requested edit text was not found.",
            Self::EditMatchNotUnique => "The requested edit text was not unique.",
            Self::IoFailure => "The runtime could not access the requested workspace target.",
            Self::RuntimeNotInitialized => "Runtime is not initialized.",
            Self::RuntimeNotReady => "Runtime is waiting for the initialized acknowledgement.",
            Self::PolicyDenied => "Tool request was blocked by policy.",
            Self::ApprovalRequired => {
                "Tool request requires approval; approval is not implemented."
            }
            Self::ExecutionFailed => "Tool execution failed.",
        }
    }
}

pub(crate) fn execution_error(error: &ExecutionError) -> CallToolResult {
    match error {
        ExecutionError::ReadFile(error) => read_file_error(error),
        ExecutionError::Glob(error) => glob_error(error),
        ExecutionError::Write(error) => write_error(error),
        ExecutionError::Edit(error) => edit_error(error),
        ExecutionError::RuntimeNotInitialized => {
            error_result(McpErrorCategory::RuntimeNotInitialized)
        }
        ExecutionError::RuntimeNotReady => error_result(McpErrorCategory::RuntimeNotReady),
        ExecutionError::InvalidArguments => error_result(McpErrorCategory::InvalidArguments),
        ExecutionError::PolicyDenied => error_result(McpErrorCategory::PolicyDenied),
        ExecutionError::ApprovalRequired => error_result(McpErrorCategory::ApprovalRequired),
        ExecutionError::InvalidTransition(_) => error_result(McpErrorCategory::ExecutionFailed),
    }
}
pub(crate) fn read_file_error(error: &ReadFileError) -> CallToolResult {
    let category = match error {
        ReadFileError::RuntimeNotInitialized => McpErrorCategory::RuntimeNotInitialized,
        ReadFileError::Workspace(error) => workspace_error_category(error),
        ReadFileError::TargetNotFile { .. } => McpErrorCategory::TargetNotFile,
        ReadFileError::UnsupportedContent { .. } => McpErrorCategory::UnsupportedContent,
        ReadFileError::ReadLimitExceeded { .. } => McpErrorCategory::ReadLimitExceeded,
        ReadFileError::IoFailure { .. } => McpErrorCategory::IoFailure,
        ReadFileError::InvalidArguments { .. } => McpErrorCategory::InvalidArguments,
    };
    error_result(category)
}

pub(crate) fn glob_error(error: &GlobError) -> CallToolResult {
    let category = match error {
        GlobError::RuntimeNotInitialized => McpErrorCategory::RuntimeNotInitialized,
        GlobError::InvalidPattern { .. } => McpErrorCategory::InvalidPattern,
        GlobError::Workspace(WorkspaceError::TargetNotFound { .. }) => {
            McpErrorCategory::InvalidBasePath
        }
        GlobError::Workspace(error) => workspace_error_category(error),
        GlobError::TargetNotDirectory { .. } => McpErrorCategory::TargetNotDirectory,
        GlobError::IoFailure { .. } => McpErrorCategory::IoFailure,
    };
    error_result(category)
}

pub(crate) fn write_error(error: &WriteError) -> CallToolResult {
    let category = match error {
        WriteError::Workspace(error) => workspace_error_category(error),
        WriteError::TargetNotFile { .. } => McpErrorCategory::TargetNotFile,
        WriteError::IoFailure { .. } => McpErrorCategory::IoFailure,
        WriteError::InvalidArguments { .. } => McpErrorCategory::InvalidArguments,
    };
    error_result(category)
}

pub(crate) fn edit_error(error: &EditError) -> CallToolResult {
    let category = match error {
        EditError::Workspace(error) => workspace_error_category(error),
        EditError::TargetNotFile { .. } => McpErrorCategory::TargetNotFile,
        EditError::UnsupportedContent { .. } => McpErrorCategory::UnsupportedContent,
        EditError::MatchNotFound { .. } => McpErrorCategory::EditMatchNotFound,
        EditError::MatchNotUnique { .. } => McpErrorCategory::EditMatchNotUnique,
        EditError::IoFailure { .. } => McpErrorCategory::IoFailure,
        EditError::InvalidArguments { .. } => McpErrorCategory::InvalidArguments,
    };
    error_result(category)
}

fn workspace_error_category(error: &WorkspaceError) -> McpErrorCategory {
    match error {
        WorkspaceError::TargetNotFound { .. } => McpErrorCategory::TargetNotFound,
        WorkspaceError::WorkspaceEscape { .. } | WorkspaceError::UnsupportedPathForm { .. } => {
            McpErrorCategory::WorkspaceViolation
        }
        WorkspaceError::InvalidWorkspaceRoot { .. }
        | WorkspaceError::WorkspaceNotFound { .. }
        | WorkspaceError::WorkspaceNotDirectory { .. } => McpErrorCategory::InvalidBasePath,
        WorkspaceError::CanonicalizationFailure { .. } => McpErrorCategory::IoFailure,
    }
}

fn error_result(category: McpErrorCategory) -> CallToolResult {
    let message = category.message();
    let mut result = CallToolResult::structured_error(json!({
        "code": category.code(),
        "message": message,
    }));
    result.content.push(ContentBlock::text(message));
    result
}
