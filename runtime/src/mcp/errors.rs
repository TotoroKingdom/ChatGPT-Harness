use crate::bash::BashRuntimeError;
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
    TargetChanged,
    IoFailure,
    RuntimeNotInitialized,
    RuntimeNotReady,
    ToolDisabled,
    PolicyDenied,
    ApprovalRequired,
    ApprovalDenied,
    ApprovalExpired,
    ApprovalCancelled,
    ExecutionFailed,
    BashUnavailable,
    BashInvalidCommand,
    BashCommandTooLong,
    BashTimeout,
    BashOutputLimit,
    BashConcurrencyLimit,
    BashSpawnFailed,
    BashCancelled,
    BashProcessFailed,
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
            Self::TargetChanged => "TARGET_CHANGED",
            Self::IoFailure => "IO_FAILURE",
            Self::RuntimeNotInitialized => "RUNTIME_NOT_INITIALIZED",
            Self::RuntimeNotReady => "RUNTIME_NOT_READY",
            Self::ToolDisabled => "TOOL_DISABLED",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::ApprovalRequired => "APPROVAL_REQUIRED",
            Self::ApprovalDenied => "APPROVAL_DENIED",
            Self::ApprovalExpired => "APPROVAL_EXPIRED",
            Self::ApprovalCancelled => "APPROVAL_CANCELLED",
            Self::ExecutionFailed => "EXECUTION_FAILED",
            Self::BashUnavailable => "BASH_UNAVAILABLE",
            Self::BashInvalidCommand => "BASH_INVALID_COMMAND",
            Self::BashCommandTooLong => "BASH_COMMAND_TOO_LONG",
            Self::BashTimeout => "BASH_TIMEOUT",
            Self::BashOutputLimit => "BASH_OUTPUT_LIMIT",
            Self::BashConcurrencyLimit => "BASH_CONCURRENCY_LIMIT",
            Self::BashSpawnFailed => "BASH_SPAWN_FAILED",
            Self::BashCancelled => "BASH_CANCELLED",
            Self::BashProcessFailed => "BASH_PROCESS_FAILED",
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
            Self::TargetChanged => "The workspace target changed during the operation.",
            Self::IoFailure => "The runtime could not access the requested workspace target.",
            Self::RuntimeNotInitialized => "Runtime is not initialized.",
            Self::RuntimeNotReady => "Runtime is waiting for the initialized acknowledgement.",
            Self::ToolDisabled => "Tool is disabled by the saved Runtime configuration.",
            Self::PolicyDenied => "Tool request was blocked by policy.",
            Self::ApprovalRequired => "Tool request requires approval.",
            Self::ApprovalDenied => "Tool request was denied by the human approver.",
            Self::ApprovalExpired => "Tool approval expired before execution.",
            Self::ApprovalCancelled => "Tool approval was cancelled by Runtime lifecycle.",
            Self::ExecutionFailed => "Tool execution failed.",
            Self::BashUnavailable => "Git Bash is unavailable in the current Runtime generation.",
            Self::BashInvalidCommand => "Bash command is invalid.",
            Self::BashCommandTooLong => "Bash command exceeds the Runtime command limit.",
            Self::BashTimeout => "Bash command exceeded the Runtime timeout.",
            Self::BashOutputLimit => "Bash output exceeded the Runtime capture limit.",
            Self::BashConcurrencyLimit => "Another Bash invocation is already running.",
            Self::BashSpawnFailed => "Runtime could not start the Bash process.",
            Self::BashCancelled => {
                "Bash invocation was cancelled and its process tree was cleaned up."
            }
            Self::BashProcessFailed => {
                "Bash process failed before a structured result was available."
            }
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
        ExecutionError::ToolDisabled => error_result(McpErrorCategory::ToolDisabled),
        ExecutionError::InvalidArguments => error_result(McpErrorCategory::InvalidArguments),
        ExecutionError::PolicyDenied => error_result(McpErrorCategory::PolicyDenied),
        ExecutionError::ApprovalRequired => error_result(McpErrorCategory::ApprovalRequired),
        ExecutionError::ApprovalDenied => error_result(McpErrorCategory::ApprovalDenied),
        ExecutionError::ApprovalExpired => error_result(McpErrorCategory::ApprovalExpired),
        ExecutionError::ApprovalCancelled => error_result(McpErrorCategory::ApprovalCancelled),
        ExecutionError::BashWorkspace(_) => error_result(McpErrorCategory::WorkspaceViolation),
        ExecutionError::BashRuntime(error) => bash_runtime_error(error),
        ExecutionError::InvalidTransition(_) => error_result(McpErrorCategory::ExecutionFailed),
    }
}

fn bash_runtime_error(error: &BashRuntimeError) -> CallToolResult {
    let category = match error {
        BashRuntimeError::Unavailable => McpErrorCategory::BashUnavailable,
        BashRuntimeError::InvalidCommand => McpErrorCategory::BashInvalidCommand,
        BashRuntimeError::CommandTooLong => McpErrorCategory::BashCommandTooLong,
        BashRuntimeError::Timeout => McpErrorCategory::BashTimeout,
        BashRuntimeError::OutputLimit => McpErrorCategory::BashOutputLimit,
        BashRuntimeError::ConcurrencyLimit => McpErrorCategory::BashConcurrencyLimit,
        BashRuntimeError::SpawnFailed => McpErrorCategory::BashSpawnFailed,
        BashRuntimeError::Cancelled => McpErrorCategory::BashCancelled,
        BashRuntimeError::ProcessFailed => McpErrorCategory::BashProcessFailed,
    };
    error_result(category)
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
        WriteError::UnsupportedContent { .. } => McpErrorCategory::UnsupportedContent,
        WriteError::TargetChanged { .. } => McpErrorCategory::TargetChanged,
        WriteError::IoFailure { .. } | WriteError::AtomicFailure { .. } => {
            McpErrorCategory::IoFailure
        }
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
        EditError::TargetChanged { .. } => McpErrorCategory::TargetChanged,
        EditError::IoFailure { .. } | EditError::AtomicFailure { .. } => {
            McpErrorCategory::IoFailure
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{AtomicPhase, EditError, WriteError};
    use std::path::PathBuf;

    #[test]
    fn m1_mutation_errors_are_stable_and_do_not_echo_paths_or_os_details() {
        let secret_path = PathBuf::from(r"C:\workspace\SECRET.txt");
        let target_changed = write_error(&WriteError::TargetChanged {
            path: secret_path.clone(),
        });
        assert_eq!(
            target_changed.structured_content.as_ref().unwrap()["code"],
            "TARGET_CHANGED"
        );
        assert!(!serde_json::to_string(&target_changed)
            .unwrap()
            .contains("SECRET.txt"));

        let atomic_failure = edit_error(&EditError::AtomicFailure {
            path: secret_path,
            phase: AtomicPhase::Commit,
            kind: std::io::ErrorKind::PermissionDenied,
        });
        assert_eq!(
            atomic_failure.structured_content.as_ref().unwrap()["code"],
            "IO_FAILURE"
        );
        assert!(!serde_json::to_string(&atomic_failure)
            .unwrap()
            .contains("PermissionDenied"));
    }
}
