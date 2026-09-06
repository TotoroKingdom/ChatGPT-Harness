use crate::workspace::{WorkspaceError, WorkspaceGuard};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::PathBuf;

/// Runtime-internal input for the `read_file` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFileRequest {
    pub path: PathBuf,
    pub offset: u64,
    pub limit: Option<u64>,
}

impl ReadFileRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            offset: 0,
            limit: None,
        }
    }
}

/// Runtime-internal result for the `read_file` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFileOutput {
    pub normalized_path: PathBuf,
    pub content: String,
    pub offset: u64,
    pub lines_read: u64,
    pub bytes_read: u64,
    pub truncated: bool,
    pub next_offset: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedContent {
    InvalidUtf8,
    ContainsNul,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFileError {
    RuntimeNotInitialized,
    Workspace(WorkspaceError),
    TargetNotFile {
        path: PathBuf,
    },
    UnsupportedContent {
        path: PathBuf,
        reason: UnsupportedContent,
    },
    ReadLimitExceeded {
        path: PathBuf,
        max_read_bytes: u32,
    },
    IoFailure {
        path: PathBuf,
        kind: io::ErrorKind,
    },
    InvalidArguments {
        message: String,
    },
}

impl fmt::Display for ReadFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeNotInitialized => write!(formatter, "runtime is not initialized"),
            Self::Workspace(error) => write!(formatter, "workspace access rejected: {error}"),
            Self::TargetNotFile { path } => {
                write!(formatter, "target is not a file: {}", path.display())
            }
            Self::UnsupportedContent { path, reason } => match reason {
                UnsupportedContent::InvalidUtf8 => {
                    write!(formatter, "file is not valid UTF-8: {}", path.display())
                }
                UnsupportedContent::ContainsNul => {
                    write!(formatter, "file contains NUL bytes: {}", path.display())
                }
            },
            Self::ReadLimitExceeded {
                path,
                max_read_bytes,
            } => write!(
                formatter,
                "the first requested line in {} exceeds the read limit of {max_read_bytes} bytes",
                path.display()
            ),
            Self::IoFailure { path, kind } => {
                write!(formatter, "I/O failure for {} ({kind:?})", path.display())
            }
            Self::InvalidArguments { message } => {
                write!(formatter, "invalid read_file arguments: {message}")
            }
        }
    }
}

impl std::error::Error for ReadFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes a bounded, text-only workspace file read.
///
/// Path approval is intentionally delegated entirely to the caller-provided
/// `WorkspaceGuard`; this executor adds no independent workspace-boundary logic.
pub fn execute_read_file(
    workspace_guard: &WorkspaceGuard,
    max_read_bytes: u32,
    request: &ReadFileRequest,
) -> Result<ReadFileOutput, ReadFileError> {
    let approved_path = workspace_guard
        .approve(&request.path)
        .map_err(ReadFileError::Workspace)?;
    let normalized_path = approved_path.as_path().to_path_buf();

    let metadata = fs::metadata(&normalized_path).map_err(|error| ReadFileError::IoFailure {
        path: normalized_path.clone(),
        kind: error.kind(),
    })?;
    if !metadata.is_file() {
        return Err(ReadFileError::TargetNotFile {
            path: normalized_path,
        });
    }

    let file = File::open(&normalized_path).map_err(|error| ReadFileError::IoFailure {
        path: normalized_path.clone(),
        kind: error.kind(),
    })?;
    let mut reader = BufReader::new(file);
    let mut utf8_validator = Utf8Validator::default();

    for _ in 0..request.offset {
        if !discard_next_line(&mut reader, &mut utf8_validator, &normalized_path)? {
            return Ok(empty_output(normalized_path, request.offset));
        }
    }

    let byte_limit =
        usize::try_from(max_read_bytes).map_err(|_| ReadFileError::InvalidArguments {
            message: "max_read_bytes cannot be represented on this platform".into(),
        })?;
    let mut content = Vec::new();
    let mut lines_read = 0_u64;

    loop {
        if request.limit == Some(lines_read) {
            return finish_at_limit(
                &mut reader,
                &mut utf8_validator,
                normalized_path,
                request.offset,
                lines_read,
                content,
            );
        }

        let remaining = byte_limit.checked_sub(content.len()).ok_or_else(|| {
            ReadFileError::InvalidArguments {
                message: "content exceeded max_read_bytes".into(),
            }
        })?;
        match read_next_line(
            &mut reader,
            &mut utf8_validator,
            remaining,
            &normalized_path,
        )? {
            LineRead::Eof => {
                utf8_validator.finish(&normalized_path)?;
                return complete_output(normalized_path, request.offset, lines_read, content);
            }
            LineRead::Complete(line) => {
                content.extend_from_slice(&line);
                lines_read =
                    lines_read
                        .checked_add(1)
                        .ok_or_else(|| ReadFileError::InvalidArguments {
                            message: "line count overflow".into(),
                        })?;
            }
            LineRead::ExceededLimit => {
                if lines_read == 0 {
                    return Err(ReadFileError::ReadLimitExceeded {
                        path: normalized_path,
                        max_read_bytes,
                    });
                }
                return truncated_output(normalized_path, request.offset, lines_read, content);
            }
        }
    }
}

fn empty_output(normalized_path: PathBuf, offset: u64) -> ReadFileOutput {
    ReadFileOutput {
        normalized_path,
        content: String::new(),
        offset,
        lines_read: 0,
        bytes_read: 0,
        truncated: false,
        next_offset: None,
    }
}

fn finish_at_limit<R: Read>(
    reader: &mut R,
    utf8_validator: &mut Utf8Validator,
    normalized_path: PathBuf,
    offset: u64,
    lines_read: u64,
    content: Vec<u8>,
) -> Result<ReadFileOutput, ReadFileError> {
    if let Some(byte) = read_byte(reader, &normalized_path)? {
        utf8_validator.push(byte, &normalized_path)?;
        return truncated_output(normalized_path, offset, lines_read, content);
    }

    utf8_validator.finish(&normalized_path)?;
    complete_output(normalized_path, offset, lines_read, content)
}

fn complete_output(
    normalized_path: PathBuf,
    offset: u64,
    lines_read: u64,
    content: Vec<u8>,
) -> Result<ReadFileOutput, ReadFileError> {
    output(normalized_path, offset, lines_read, content, false)
}

fn truncated_output(
    normalized_path: PathBuf,
    offset: u64,
    lines_read: u64,
    content: Vec<u8>,
) -> Result<ReadFileOutput, ReadFileError> {
    output(normalized_path, offset, lines_read, content, true)
}

fn output(
    normalized_path: PathBuf,
    offset: u64,
    lines_read: u64,
    content: Vec<u8>,
    truncated: bool,
) -> Result<ReadFileOutput, ReadFileError> {
    let next_offset = if truncated {
        Some(
            offset
                .checked_add(lines_read)
                .ok_or_else(|| ReadFileError::InvalidArguments {
                    message: "next offset overflow".into(),
                })?,
        )
    } else {
        None
    };
    let bytes_read = u64::try_from(content.len()).map_err(|_| ReadFileError::InvalidArguments {
        message: "content length overflow".into(),
    })?;
    let content = String::from_utf8(content).map_err(|_| ReadFileError::UnsupportedContent {
        path: normalized_path.clone(),
        reason: UnsupportedContent::InvalidUtf8,
    })?;

    Ok(ReadFileOutput {
        normalized_path,
        content,
        offset,
        lines_read,
        bytes_read,
        truncated,
        next_offset,
    })
}

enum LineRead {
    Eof,
    Complete(Vec<u8>),
    ExceededLimit,
}

fn discard_next_line<R: Read>(
    reader: &mut R,
    utf8_validator: &mut Utf8Validator,
    path: &PathBuf,
) -> Result<bool, ReadFileError> {
    let mut found_content = false;
    while let Some(byte) = read_byte(reader, path)? {
        found_content = true;
        utf8_validator.push(byte, path)?;
        if byte == b'\n' {
            return Ok(true);
        }
    }

    utf8_validator.finish(path)?;
    Ok(found_content)
}

fn read_next_line<R: Read>(
    reader: &mut R,
    utf8_validator: &mut Utf8Validator,
    byte_limit: usize,
    path: &PathBuf,
) -> Result<LineRead, ReadFileError> {
    let mut line = Vec::new();
    while let Some(byte) = read_byte(reader, path)? {
        utf8_validator.push(byte, path)?;
        if line.len() == byte_limit {
            return Ok(LineRead::ExceededLimit);
        }
        line.push(byte);
        if byte == b'\n' {
            return Ok(LineRead::Complete(line));
        }
    }

    utf8_validator.finish(path)?;
    if line.is_empty() {
        Ok(LineRead::Eof)
    } else {
        Ok(LineRead::Complete(line))
    }
}

fn read_byte<R: Read>(reader: &mut R, path: &PathBuf) -> Result<Option<u8>, ReadFileError> {
    let mut byte = [0_u8; 1];
    match reader.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(error) => Err(ReadFileError::IoFailure {
            path: path.clone(),
            kind: error.kind(),
        }),
    }
}

#[derive(Default)]
struct Utf8Validator {
    remaining_continuations: u8,
    codepoint: u32,
    minimum_codepoint: u32,
}

impl Utf8Validator {
    fn push(&mut self, byte: u8, path: &PathBuf) -> Result<(), ReadFileError> {
        if byte == 0 {
            return Err(ReadFileError::UnsupportedContent {
                path: path.clone(),
                reason: UnsupportedContent::ContainsNul,
            });
        }

        if self.remaining_continuations == 0 {
            match byte {
                0x00..=0x7f => Ok(()),
                0xc2..=0xdf => {
                    self.remaining_continuations = 1;
                    self.codepoint = u32::from(byte & 0x1f);
                    self.minimum_codepoint = 0x80;
                    Ok(())
                }
                0xe0..=0xef => {
                    self.remaining_continuations = 2;
                    self.codepoint = u32::from(byte & 0x0f);
                    self.minimum_codepoint = 0x800;
                    Ok(())
                }
                0xf0..=0xf4 => {
                    self.remaining_continuations = 3;
                    self.codepoint = u32::from(byte & 0x07);
                    self.minimum_codepoint = 0x1_0000;
                    Ok(())
                }
                _ => Err(invalid_utf8(path)),
            }
        } else if byte & 0xc0 != 0x80 {
            Err(invalid_utf8(path))
        } else {
            self.codepoint = (self.codepoint << 6) | u32::from(byte & 0x3f);
            self.remaining_continuations -= 1;
            if self.remaining_continuations == 0
                && (self.codepoint < self.minimum_codepoint
                    || self.codepoint > 0x10_ffff
                    || (0xd800..=0xdfff).contains(&self.codepoint))
            {
                return Err(invalid_utf8(path));
            }
            Ok(())
        }
    }

    fn finish(&self, path: &PathBuf) -> Result<(), ReadFileError> {
        if self.remaining_continuations == 0 {
            Ok(())
        } else {
            Err(invalid_utf8(path))
        }
    }
}

fn invalid_utf8(path: &PathBuf) -> ReadFileError {
    ReadFileError::UnsupportedContent {
        path: path.clone(),
        reason: UnsupportedContent::InvalidUtf8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "chatgpt-harness-read-file-{}",
                uuid::Uuid::new_v4()
            ));
            let root = base.join("workspace");
            let outside = base.join("outside");
            fs::create_dir_all(&root).expect("create workspace");
            fs::create_dir_all(&outside).expect("create outside directory");
            Self {
                base,
                root,
                outside,
            }
        }

        fn write(&self, relative_path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
            let path = self.root.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, bytes).expect("write fixture");
        }

        fn guard(&self) -> WorkspaceGuard {
            WorkspaceGuard::new(&Workspace::new(&self.root).expect("create workspace"))
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn request(path: impl Into<PathBuf>) -> ReadFileRequest {
        ReadFileRequest::new(path)
    }

    #[test]
    fn reads_nested_utf8_content_and_returns_the_approved_path() {
        let workspace = TestWorkspace::new();
        workspace.write("nested/hello.txt", "hello, 世界\n");
        let guard = workspace.guard();

        let output = execute_read_file(&guard, 64, &request("nested/hello.txt")).unwrap();

        assert_eq!(output.content, "hello, 世界\n");
        assert_eq!(output.lines_read, 1);
        assert_eq!(output.bytes_read, "hello, 世界\n".len() as u64);
        assert!(!output.truncated);
        assert_eq!(output.next_offset, None);
        assert_eq!(
            output.normalized_path,
            fs::canonicalize(workspace.root.join("nested/hello.txt")).unwrap()
        );
    }

    #[test]
    fn reads_empty_and_crlf_files() {
        let workspace = TestWorkspace::new();
        workspace.write("empty.txt", b"");
        workspace.write("crlf.txt", b"one\r\ntwo\r\n");
        let guard = workspace.guard();

        let empty = execute_read_file(&guard, 32, &request("empty.txt")).unwrap();
        assert_eq!(empty.content, "");
        assert_eq!(empty.lines_read, 0);

        let crlf = execute_read_file(&guard, 32, &request("crlf.txt")).unwrap();
        assert_eq!(crlf.content, "one\r\ntwo\r\n");
        assert_eq!(crlf.lines_read, 2);
    }

    #[test]
    fn offset_limit_and_paging_are_line_based_without_gaps() {
        let workspace = TestWorkspace::new();
        workspace.write("lines.txt", "zero\none\ntwo\nthree\n");
        let guard = workspace.guard();

        let second_page = execute_read_file(
            &guard,
            128,
            &ReadFileRequest {
                path: PathBuf::from("lines.txt"),
                offset: 1,
                limit: Some(2),
            },
        )
        .unwrap();
        assert_eq!(second_page.content, "one\ntwo\n");
        assert_eq!(second_page.offset, 1);
        assert_eq!(second_page.lines_read, 2);
        assert!(second_page.truncated);
        assert_eq!(second_page.next_offset, Some(3));

        let final_page = execute_read_file(
            &guard,
            128,
            &ReadFileRequest {
                path: PathBuf::from("lines.txt"),
                offset: second_page.next_offset.unwrap(),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(final_page.content, "three\n");
        assert!(!final_page.truncated);
        assert_eq!(final_page.next_offset, None);
    }

    #[test]
    fn offset_past_eof_returns_an_empty_non_truncated_page() {
        let workspace = TestWorkspace::new();
        workspace.write("one-line.txt", "only\n");
        let guard = workspace.guard();

        let output = execute_read_file(
            &guard,
            64,
            &ReadFileRequest {
                path: PathBuf::from("one-line.txt"),
                offset: 9,
                limit: None,
            },
        )
        .unwrap();

        assert_eq!(output.content, "");
        assert_eq!(output.lines_read, 0);
        assert!(!output.truncated);
        assert_eq!(output.next_offset, None);
    }

    #[test]
    fn byte_limit_returns_complete_lines_without_splitting_utf8() {
        let workspace = TestWorkspace::new();
        workspace.write("unicode.txt", "é\nx\n");
        let guard = workspace.guard();

        let output = execute_read_file(&guard, 3, &request("unicode.txt")).unwrap();

        assert_eq!(output.content, "é\n");
        assert_eq!(output.bytes_read, 3);
        assert!(output.bytes_read <= 3);
        assert!(output.truncated);
        assert_eq!(output.next_offset, Some(1));
    }

    #[test]
    fn first_line_over_the_byte_limit_is_a_structured_error() {
        let workspace = TestWorkspace::new();
        workspace.write("long.txt", "12345\n");
        let guard = workspace.guard();

        assert!(matches!(
            execute_read_file(&guard, 4, &request("long.txt")),
            Err(ReadFileError::ReadLimitExceeded {
                max_read_bytes: 4,
                ..
            })
        ));
    }

    #[test]
    fn large_file_stops_at_the_configured_limit_without_returning_extra_content() {
        let workspace = TestWorkspace::new();
        let mut contents = String::from("a\n");
        contents.push_str(&"b".repeat(2 * 1024 * 1024));
        contents.push('\n');
        workspace.write("large.txt", contents);
        let guard = workspace.guard();

        let output = execute_read_file(&guard, 2, &request("large.txt")).unwrap();

        assert_eq!(output.content, "a\n");
        assert_eq!(output.bytes_read, 2);
        assert!(output.truncated);
        assert_eq!(output.next_offset, Some(1));
    }

    #[test]
    fn rejects_invalid_utf8_and_nul_content() {
        let workspace = TestWorkspace::new();
        workspace.write("invalid.txt", [0xff, b'\n']);
        workspace.write("contains-nul.txt", b"ok\0\n");
        let guard = workspace.guard();

        assert!(matches!(
            execute_read_file(&guard, 64, &request("invalid.txt")),
            Err(ReadFileError::UnsupportedContent {
                reason: UnsupportedContent::InvalidUtf8,
                ..
            })
        ));
        assert!(matches!(
            execute_read_file(&guard, 64, &request("contains-nul.txt")),
            Err(ReadFileError::UnsupportedContent {
                reason: UnsupportedContent::ContainsNul,
                ..
            })
        ));
    }

    #[test]
    fn workspace_guard_allows_an_absolute_path_inside_the_workspace() {
        let workspace = TestWorkspace::new();
        workspace.write("inside.txt", "inside\n");
        let guard = workspace.guard();

        let output =
            execute_read_file(&guard, 64, &request(workspace.root.join("inside.txt"))).unwrap();

        assert_eq!(output.content, "inside\n");
        assert!(!output.truncated);
    }
    #[test]
    fn rejects_directories_missing_files_and_workspace_escapes() {
        let workspace = TestWorkspace::new();
        fs::create_dir_all(workspace.root.join("directory")).unwrap();
        fs::write(workspace.outside.join("secret.txt"), "secret").unwrap();
        let guard = workspace.guard();

        assert!(matches!(
            execute_read_file(&guard, 64, &request("directory")),
            Err(ReadFileError::TargetNotFile { .. })
        ));
        assert!(matches!(
            execute_read_file(&guard, 64, &request("missing.txt")),
            Err(ReadFileError::Workspace(
                WorkspaceError::TargetNotFound { .. }
            ))
        ));
        assert!(matches!(
            execute_read_file(&guard, 64, &request("../outside/secret.txt")),
            Err(ReadFileError::Workspace(
                WorkspaceError::WorkspaceEscape { .. }
            ))
        ));
        assert!(matches!(
            execute_read_file(&guard, 64, &request(workspace.outside.join("secret.txt"))),
            Err(ReadFileError::Workspace(
                WorkspaceError::WorkspaceEscape { .. }
            ))
        ));
    }
}
