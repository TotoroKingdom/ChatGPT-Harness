use crate::tools::{GlobError, ReadFileError};
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
    IoFailure,
    RuntimeNotInitialized,
    RuntimeNotReady,
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
            Self::IoFailure => "IO_FAILURE",
            Self::RuntimeNotInitialized => "RUNTIME_NOT_INITIALIZED",
            Self::RuntimeNotReady => "RUNTIME_NOT_READY",
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
            Self::IoFailure => "The runtime could not read the requested workspace target.",
            Self::RuntimeNotInitialized => "Runtime is not initialized.",
            Self::RuntimeNotReady => "Runtime is waiting for the initialized acknowledgement.",
        }
    }
}

pub(crate) fn invalid_arguments() -> CallToolResult {
    error_result(McpErrorCategory::InvalidArguments)
}

pub(crate) fn runtime_not_ready() -> CallToolResult {
    error_result(McpErrorCategory::RuntimeNotReady)
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
