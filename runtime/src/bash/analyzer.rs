//! Conservative, quote-aware facts for Bash policy evaluation.
//!
//! This module deliberately does not produce an executable command.  The
//! caller keeps the original request for execution while this analyzer emits
//! only bounded logical facts used by Policy/Approval.  Anything outside the
//! small supported grammar is classified high risk and therefore receives at
//! least the ASK policy floor.

use harness_control_protocol::{normalize_bash_executable, BashCommandMatcherConfig};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BashOperator {
    AndIf,
    OrIf,
    Sequence,
    Pipe,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BashRiskFlag {
    CommandSubstitution,
    Backticks,
    Eval,
    NestedShell,
    InterpreterInlineCode,
    Redirection,
    DynamicExpansion,
    MalformedQuoting,
    UnsupportedGrammar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BashClassification {
    Classified,
    UnclassifiedHighRisk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashCommandSegment {
    /// Normalized executable identity.  Unsupported executable syntax uses a
    /// fixed placeholder, so display/evidence never needs to echo a path.
    pub(crate) executable: String,
    /// Logical argv after quote/escape decoding.  This is an internal fact;
    /// callers must use the sanitized projections for public evidence.
    pub(crate) argv: Vec<String>,
    pub(crate) risk_flags: Vec<BashRiskFlag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashCommandAnalysis {
    pub(crate) segments: Vec<BashCommandSegment>,
    pub(crate) operators: Vec<BashOperator>,
    pub(crate) classification: BashClassification,
}

impl BashCommandAnalysis {
    pub(crate) fn is_classified(&self) -> bool {
        matches!(self.classification, BashClassification::Classified)
    }

    pub(crate) fn allow_for_session(&self) -> bool {
        self.is_classified() && !self.segments.is_empty()
    }

    /// Returns a stable, compact representation of the exact normalized
    /// command shape.  Only segments, operators, executable identity and
    /// logical argv participate; risk metadata is intentionally excluded.
    pub(crate) fn normalized_scope(&self) -> String {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Scope<'a> {
            version: u8,
            segments: Vec<ScopeSegment<'a>>,
            operators: &'a [BashOperator],
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ScopeSegment<'a> {
            executable: &'a str,
            argv: &'a [String],
        }

        let segments = self
            .segments
            .iter()
            .map(|segment| ScopeSegment {
                executable: &segment.executable,
                argv: &segment.argv,
            })
            .collect();
        serde_json::to_string(&Scope {
            version: 1,
            segments,
            operators: &self.operators,
        })
        .unwrap_or_else(|_| "{\"version\":1,\"segments\":[],\"operators\":[]}".into())
    }

    /// Sanitized scope suitable for approval/activity display.  This is
    /// intentionally separate from `normalized_scope`, which remains an
    /// internal exact-match key and must never be sent over the control API.
    pub(crate) fn sanitized_scope(&self) -> String {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Scope<'a> {
            version: u8,
            segments: Vec<DisplaySegment>,
            operators: &'a [BashOperator],
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct DisplaySegment {
            executable: String,
            argv: Vec<String>,
        }

        let segments = self
            .segments
            .iter()
            .map(|segment| DisplaySegment {
                executable: sanitize_executable(&segment.executable),
                argv: sanitize_argv(segment),
            })
            .collect();
        let serialized = serde_json::to_string(&Scope {
            version: 1,
            segments,
            operators: &self.operators,
        })
        .unwrap_or_else(|_| "{\"version\":1,\"segments\":[],\"operators\":[]}".into());
        if serialized.len() <= MAX_DISPLAY_BYTES {
            serialized
        } else {
            "{\"version\":1,\"redacted\":true}".into()
        }
    }

    /// A sanitized scope containing `<redacted>` or an overflow marker must
    /// never be fed back into a persistent BashCommand matcher draft.
    pub(crate) fn sanitized_scope_is_complete(&self) -> bool {
        let sanitized = self.sanitized_scope();
        sanitized != "{\"version\":1,\"redacted\":true}"
            && sanitized.len() <= MAX_DISPLAY_BYTES
            && self.segments.iter().all(|segment| {
                sanitize_executable(&segment.executable) == segment.executable
                    && sanitize_argv(segment) == segment.argv
            })
    }

    /// A bounded shell-like preview for approval cards.  It is derived from
    /// logical facts and never from the raw shell source.
    pub(crate) fn sanitized_preview(&self) -> String {
        let mut output = String::new();
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                if let Some(operator) = self.operators.get(index - 1) {
                    output.push(' ');
                    output.push_str(operator_text(operator));
                    output.push(' ');
                }
            }
            output.push_str(&sanitize_executable(&segment.executable));
            for argument in sanitize_argv(segment) {
                output.push(' ');
                output.push_str(&quote_display_token(&argument));
            }
        }
        if output.len() > MAX_DISPLAY_BYTES {
            let mut end = MAX_DISPLAY_BYTES;
            while end > 0 && !output.is_char_boundary(end) {
                end -= 1;
            }
            output.truncate(end);
            output.push('…');
        }
        output
    }

    pub(crate) fn risk_flags(&self) -> Vec<BashRiskFlag> {
        let mut flags = BTreeSet::new();
        for segment in &self.segments {
            flags.extend(segment.risk_flags.iter().cloned());
        }
        flags.into_iter().collect()
    }

    pub(crate) fn matches_bash_command(&self, matcher: &BashCommandMatcherConfig) -> bool {
        let Some(executable) = normalize_bash_executable(&matcher.executable) else {
            return false;
        };
        self.segments.iter().any(|segment| {
            segment.executable == executable
                && matcher.argv_prefix.len() <= segment.argv.len()
                && matcher
                    .argv_prefix
                    .iter()
                    .zip(&segment.argv)
                    .all(|(expected, actual)| expected == actual)
        })
    }
}

const MAX_DISPLAY_BYTES: usize = 1024;
const REDACTED: &str = "<redacted>";
const UNSUPPORTED_EXECUTABLE: &str = "<unrecognized>";

#[derive(Clone, Debug)]
struct Token {
    value: String,
    risk_flags: BTreeSet<BashRiskFlag>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

/// Analyze only the small M5 grammar.  Errors are represented as risk facts
/// rather than returned as a parser error so every uncertain command reaches
/// the ASK floor and can still produce a useful sanitized approval preview.
pub(crate) fn analyze(command: &str) -> BashCommandAnalysis {
    let mut segments: Vec<Vec<Token>> = vec![Vec::new()];
    let mut operators = Vec::new();
    let mut current = String::new();
    let mut current_flags = BTreeSet::new();
    let mut quote = Quote::None;
    let mut token_started = false;
    let mut global_flags = BTreeSet::new();
    let chars = command.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];
        match quote {
            Quote::Single => {
                if character == '\'' {
                    quote = Quote::None;
                } else {
                    current.push(character);
                    token_started = true;
                }
                index += 1;
            }
            Quote::Double => {
                if character == '"' {
                    quote = Quote::None;
                    index += 1;
                    continue;
                }
                if character == '\\' {
                    if let Some(next) = chars.get(index + 1).copied() {
                        if matches!(next, '"' | '\\' | '$' | '`') {
                            current.push(next);
                            token_started = true;
                            index += 2;
                            continue;
                        }
                        if matches!(next, '\n' | '\r') {
                            index += 2;
                            if next == '\r' && chars.get(index).copied() == Some('\n') {
                                index += 1;
                            }
                            continue;
                        }
                    } else {
                        current_flags.insert(BashRiskFlag::MalformedQuoting);
                        global_flags.insert(BashRiskFlag::MalformedQuoting);
                        index += 1;
                        continue;
                    }
                }
                if character == '$' {
                    mark_expansion(&chars, index, &mut current_flags, &mut global_flags);
                } else if character == '`' {
                    current_flags.insert(BashRiskFlag::Backticks);
                    global_flags.insert(BashRiskFlag::Backticks);
                }
                if character.is_control() {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                }
                current.push(character);
                token_started = true;
                index += 1;
            }
            Quote::None => {
                if character.is_ascii_whitespace() {
                    if matches!(character, '\n' | '\r') {
                        current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    }
                    flush_token(
                        &mut current,
                        &mut current_flags,
                        token_started,
                        &mut segments,
                    );
                    token_started = false;
                    index += 1;
                    continue;
                }
                if character.is_control() {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                }
                if character == '\'' {
                    quote = Quote::Single;
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character == '"' {
                    quote = Quote::Double;
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character == '\\' {
                    if let Some(next) = chars.get(index + 1).copied() {
                        if matches!(next, '\n' | '\r') {
                            index += 2;
                            if next == '\r' && chars.get(index).copied() == Some('\n') {
                                index += 1;
                            }
                        } else {
                            current.push(next);
                            token_started = true;
                            index += 2;
                        }
                    } else {
                        current_flags.insert(BashRiskFlag::MalformedQuoting);
                        global_flags.insert(BashRiskFlag::MalformedQuoting);
                        index += 1;
                    }
                    continue;
                }
                if character == '$' {
                    mark_expansion(&chars, index, &mut current_flags, &mut global_flags);
                    current.push(character);
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character == '`' {
                    current_flags.insert(BashRiskFlag::Backticks);
                    global_flags.insert(BashRiskFlag::Backticks);
                    current.push(character);
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character.is_control() {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                }
                if character == '#' {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    current.push(character);
                    token_started = true;
                    index += 1;
                    continue;
                }
                if matches!(character, '<' | '>') {
                    flush_token(
                        &mut current,
                        &mut current_flags,
                        token_started,
                        &mut segments,
                    );
                    token_started = false;
                    current_flags.insert(BashRiskFlag::Redirection);
                    global_flags.insert(BashRiskFlag::Redirection);
                    if chars.get(index + 1) == Some(&character) {
                        index += 2;
                    } else {
                        index += 1;
                    }
                    continue;
                }
                if character == '&' {
                    flush_token(
                        &mut current,
                        &mut current_flags,
                        token_started,
                        &mut segments,
                    );
                    token_started = false;
                    if chars.get(index + 1) == Some(&'&') {
                        operators.push(BashOperator::AndIf);
                        segments.push(Vec::new());
                        index += 2;
                    } else {
                        current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                        index += 1;
                    }
                    continue;
                }
                if character == '|' {
                    flush_token(
                        &mut current,
                        &mut current_flags,
                        token_started,
                        &mut segments,
                    );
                    token_started = false;
                    if chars.get(index + 1) == Some(&'|') {
                        operators.push(BashOperator::OrIf);
                        segments.push(Vec::new());
                        index += 2;
                    } else {
                        operators.push(BashOperator::Pipe);
                        segments.push(Vec::new());
                        index += 1;
                    }
                    continue;
                }
                if character == ';' {
                    flush_token(
                        &mut current,
                        &mut current_flags,
                        token_started,
                        &mut segments,
                    );
                    token_started = false;
                    operators.push(BashOperator::Sequence);
                    segments.push(Vec::new());
                    index += 1;
                    continue;
                }
                if matches!(character, '(' | ')' | '{' | '}') {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                }
                if matches!(character, '*' | '?' | '[' | ']') {
                    current_flags.insert(BashRiskFlag::DynamicExpansion);
                    global_flags.insert(BashRiskFlag::DynamicExpansion);
                }
                if matches!(character, '~' | '!') {
                    current_flags.insert(BashRiskFlag::DynamicExpansion);
                    global_flags.insert(BashRiskFlag::DynamicExpansion);
                }
                current.push(character);
                token_started = true;
                index += 1;
            }
        }
    }

    if quote != Quote::None {
        current_flags.insert(BashRiskFlag::MalformedQuoting);
        global_flags.insert(BashRiskFlag::MalformedQuoting);
    }
    flush_token(
        &mut current,
        &mut current_flags,
        token_started,
        &mut segments,
    );

    let mut analyzed = segments
        .into_iter()
        .map(|tokens| analyze_segment(tokens, &mut global_flags))
        .collect::<Vec<_>>();
    if analyzed.iter().all(|segment| segment.executable.is_empty()) {
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
        analyzed.clear();
    }
    if analyzed.len() != operators.len() + 1 {
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
    }
    if analyzed.is_empty() {
        analyzed.push(BashCommandSegment {
            executable: UNSUPPORTED_EXECUTABLE.into(),
            argv: Vec::new(),
            risk_flags: Vec::new(),
        });
    }
    if !global_flags.is_empty() {
        let first = analyzed
            .first_mut()
            .expect("analyzer always creates a bounded placeholder segment");
        first.risk_flags.extend(global_flags);
        first.risk_flags.sort();
        first.risk_flags.dedup();
    }
    let classification = if analyzed
        .iter()
        .any(|segment| !segment.risk_flags.is_empty())
    {
        BashClassification::UnclassifiedHighRisk
    } else {
        BashClassification::Classified
    };
    BashCommandAnalysis {
        segments: analyzed,
        operators,
        classification,
    }
}

fn flush_token(
    current: &mut String,
    flags: &mut BTreeSet<BashRiskFlag>,
    started: bool,
    segments: &mut [Vec<Token>],
) {
    if !started && current.is_empty() && flags.is_empty() {
        return;
    }
    let token = Token {
        value: std::mem::take(current),
        risk_flags: std::mem::take(flags),
    };
    if let Some(segment) = segments.last_mut() {
        segment.push(token);
    }
}

fn analyze_segment(
    tokens: Vec<Token>,
    global_flags: &mut BTreeSet<BashRiskFlag>,
) -> BashCommandSegment {
    if tokens.is_empty() {
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
        return BashCommandSegment {
            executable: String::new(),
            argv: Vec::new(),
            risk_flags: Vec::new(),
        };
    }
    let executable_token = &tokens[0].value;
    let executable = normalize_bash_executable(executable_token).unwrap_or_else(|| {
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
        UNSUPPORTED_EXECUTABLE.into()
    });
    let mut risk_flags = BTreeSet::new();
    for token in &tokens {
        risk_flags.extend(token.risk_flags.iter().cloned());
    }
    let argv = tokens
        .iter()
        .skip(1)
        .map(|token| token.value.clone())
        .collect::<Vec<_>>();
    let executable_name = executable.rsplit('/').next().unwrap_or(&executable);
    if executable_name == "eval" {
        risk_flags.insert(BashRiskFlag::Eval);
    }
    if matches!(
        executable_name,
        "env"
            | "command"
            | "builtin"
            | "source"
            | "."
            | "exec"
            | "function"
            | "export"
            | "unset"
            | "set"
            | "trap"
            | "alias"
            | "unalias"
            | "enable"
            | "hash"
            | "shopt"
            | "declare"
            | "typeset"
            | "readonly"
            | "local"
            | "let"
            | "select"
            | "coproc"
            | "time"
            | "!"
            | "if"
            | "then"
            | "else"
            | "fi"
            | "for"
            | "while"
            | "until"
            | "case"
            | "esac"
            | "do"
            | "done"
    ) {
        risk_flags.insert(BashRiskFlag::UnsupportedGrammar);
    }
    if is_nested_shell(&executable, &argv) {
        risk_flags.insert(BashRiskFlag::NestedShell);
    }
    if is_interpreter_inline_code(&executable, &argv) {
        risk_flags.insert(BashRiskFlag::InterpreterInlineCode);
    }
    BashCommandSegment {
        executable,
        argv,
        risk_flags: risk_flags.into_iter().collect(),
    }
}

fn mark_expansion(
    chars: &[char],
    index: usize,
    current_flags: &mut BTreeSet<BashRiskFlag>,
    global_flags: &mut BTreeSet<BashRiskFlag>,
) {
    match chars.get(index + 1).copied() {
        Some('(') => {
            current_flags.insert(BashRiskFlag::CommandSubstitution);
            global_flags.insert(BashRiskFlag::CommandSubstitution);
        }
        Some(character) if character.is_ascii_alphanumeric() || matches!(character, '_' | '{') => {
            current_flags.insert(BashRiskFlag::DynamicExpansion);
            global_flags.insert(BashRiskFlag::DynamicExpansion);
        }
        _ => {
            // Any unescaped `$` outside single quotes may alter argv or
            // command shape (`$$`, `$?`, `$@`, `$*`, `$!`, `$-`, etc.).
            current_flags.insert(BashRiskFlag::DynamicExpansion);
            global_flags.insert(BashRiskFlag::DynamicExpansion);
        }
    }
}

fn is_nested_shell(executable: &str, argv: &[String]) -> bool {
    let executable = executable.rsplit('/').next().unwrap_or(executable);
    if matches!(
        executable,
        "bash" | "sh" | "dash" | "zsh" | "ksh" | "cmd" | "powershell" | "pwsh"
    ) {
        return true;
    }
    if !matches!(executable, "bash" | "sh" | "dash" | "zsh" | "ksh") {
        return false;
    }
    argv.iter()
        .any(|argument| has_attached_flag(argument, &["-c", "--command"]))
}

fn is_interpreter_inline_code(executable: &str, argv: &[String]) -> bool {
    let executable = executable.rsplit('/').next().unwrap_or(executable);
    let is_python = executable == "python"
        || executable == "python2"
        || executable == "python3"
        || (executable.starts_with("python")
            && executable[6..]
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.'));
    let inline_flags: &[&str] = match (is_python, executable) {
        (true, _) | (false, "node") | (false, "nodejs") | (false, "perl") | (false, "ruby") => {
            &["-c", "--command", "-e", "--eval"]
        }
        (false, "php") => &["-r", "--run"],
        _ => return false,
    };
    argv.iter()
        .any(|argument| has_attached_flag(argument, inline_flags))
}

fn has_attached_flag(argument: &str, flags: &[&str]) -> bool {
    flags
        .iter()
        .any(|flag| argument == *flag || argument.starts_with(flag) && argument.len() > flag.len())
}

fn sanitize_argv(segment: &BashCommandSegment) -> Vec<String> {
    let mut sanitized = Vec::with_capacity(segment.argv.len());
    let mut redact_next = false;
    for argument in &segment.argv {
        if redact_next {
            sanitized.push(REDACTED.into());
            redact_next = false;
            continue;
        }
        let lower = argument.to_ascii_lowercase();
        if is_secret_key(&lower) {
            if !argument.contains('=') {
                redact_next = true;
            }
            sanitized.push(
                argument
                    .split_once('=')
                    .map(|(key, _)| format!("{key}={REDACTED}"))
                    .unwrap_or_else(|| REDACTED.into()),
            );
        } else if contains_publicly_sensitive_shape(argument)
            || segment.risk_flags.iter().any(|flag| {
                matches!(
                    flag,
                    BashRiskFlag::CommandSubstitution
                        | BashRiskFlag::Backticks
                        | BashRiskFlag::DynamicExpansion
                )
            }) && argument
                .chars()
                .any(|character| matches!(character, '$' | '`'))
        {
            sanitized.push(REDACTED.into());
        } else if argument.len() > MAX_DISPLAY_BYTES / 2
            || argument.chars().any(|character| character.is_control())
        {
            sanitized.push(REDACTED.into());
        } else {
            sanitized.push(argument.clone());
        }
    }
    sanitized
}

fn is_secret_key(argument: &str) -> bool {
    let key = argument
        .split_once('=')
        .map(|(key, _)| key)
        .unwrap_or(argument);
    let normalized = argument.trim_start_matches('-').to_ascii_lowercase();
    if normalized.starts_with("authorization:")
        || normalized.starts_with("proxy-authorization:")
        || normalized.starts_with("cookie:")
        || normalized.contains("bearer ")
    {
        return true;
    }
    let key = key.trim_start_matches('-').to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "api-key",
        "apikey",
        "access_key",
        "access-key",
        "auth_token",
        "auth-token",
        "client_secret",
        "client-secret",
        "private_key",
        "private-key",
        "authorization",
        "key",
        "cookie",
        "credential",
        "user",
        "username",
        "header",
        "h",
        "u",
        "p",
    ]
    .iter()
    .any(|needle| {
        key == *needle
            || key.ends_with(&format!("_{needle}"))
            || key.ends_with(&format!("-{needle}"))
    })
}

fn contains_publicly_sensitive_shape(argument: &str) -> bool {
    if argument.contains('=')
        || argument.contains("://")
        || argument.contains('@')
        || argument.contains('?')
        || argument.starts_with('/')
        || argument.starts_with("./")
        || argument.starts_with("../")
        || argument.starts_with(".\\")
        || argument.starts_with("..\\")
        || (argument.len() >= 3
            && argument.as_bytes()[0].is_ascii_alphabetic()
            && argument.as_bytes()[1] == b':'
            && matches!(argument.as_bytes()[2], b'/' | b'\\'))
    {
        return true;
    }
    // Opaque credentials often contain only lowercase hex or URL-safe text,
    // so requiring three character classes would expose common API keys.
    if argument.len() >= 24
        && argument.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return true;
    }
    false
}

fn sanitize_executable(executable: &str) -> String {
    if executable == UNSUPPORTED_EXECUTABLE {
        return executable.into();
    }
    let basename = executable.rsplit('/').next().unwrap_or(executable);
    if basename.is_empty()
        || basename.len() > MAX_DISPLAY_BYTES / 4
        || contains_publicly_sensitive_shape(basename)
        || is_secret_key(basename)
    {
        REDACTED.into()
    } else {
        basename.into()
    }
}

fn quote_display_token(token: &str) -> String {
    // The preview is never executable authority, but quoting shell syntax
    // keeps literal argv such as `&&` and `$(...)` from looking like syntax.
    if token.is_empty()
        || !token.chars().all(|character| {
            character.is_alphanumeric()
                || matches!(
                    character,
                    '-' | '_' | '.' | '/' | ':' | '+' | ',' | '%' | '='
                )
        })
    {
        format!("'{}'", token.replace('\'', "'\\''"))
    } else {
        token.to_owned()
    }
}

fn operator_text(operator: &BashOperator) -> &'static str {
    match operator {
        BashOperator::AndIf => "&&",
        BashOperator::OrIf => "||",
        BashOperator::Sequence => ";",
        BashOperator::Pipe => "|",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(analysis: &BashCommandAnalysis, index: usize) -> &BashCommandSegment {
        &analysis.segments[index]
    }

    #[test]
    fn simple_command_decodes_logical_argv_and_normalizes_executable() {
        let analysis = analyze("GIT.EXE status --short");
        assert_eq!(analysis.classification, BashClassification::Classified);
        assert_eq!(segment(&analysis, 0).executable, "git");
        assert_eq!(segment(&analysis, 0).argv, ["status", "--short"]);
        assert_eq!(analysis.operators, []);
    }

    #[test]
    fn quoted_arguments_are_decoded_without_rewriting_execution() {
        let analysis = analyze("git \"status report\" 'two words'");
        assert_eq!(segment(&analysis, 0).argv, ["status report", "two words"]);
        assert!(analysis.is_classified());
    }

    #[test]
    fn quoted_operators_remain_arguments() {
        let analysis = analyze("echo \"&&\" '|' ");
        assert_eq!(analysis.operators, []);
        assert_eq!(segment(&analysis, 0).argv, ["&&", "|"]);
    }

    #[test]
    fn supported_compound_operators_split_segments_deterministically() {
        let analysis = analyze("git status && git push || git log; git diff | cat");
        assert_eq!(
            analysis.operators,
            [
                BashOperator::AndIf,
                BashOperator::OrIf,
                BashOperator::Sequence,
                BashOperator::Pipe
            ]
        );
        assert_eq!(analysis.segments.len(), 5);
        assert_eq!(segment(&analysis, 1).executable, "git");
        assert_eq!(segment(&analysis, 1).argv, ["push"]);
        assert!(analysis.is_classified());
    }

    #[test]
    fn malformed_quotes_are_unclassified_high_risk() {
        let analysis = analyze("git status 'unfinished");
        assert_eq!(
            analysis.classification,
            BashClassification::UnclassifiedHighRisk
        );
        assert!(analysis
            .risk_flags()
            .contains(&BashRiskFlag::MalformedQuoting));
    }

    #[test]
    fn dynamic_and_high_risk_shapes_are_flagged_without_regex_blacklist_authorization() {
        for (command, flag) in [
            ("echo $(whoami)", BashRiskFlag::CommandSubstitution),
            ("echo `whoami`", BashRiskFlag::Backticks),
            ("eval \"echo hi\"", BashRiskFlag::Eval),
            ("bash -c \"echo hi\"", BashRiskFlag::NestedShell),
            (
                "python -c \"print(1)\"",
                BashRiskFlag::InterpreterInlineCode,
            ),
            ("cat < input.txt", BashRiskFlag::Redirection),
            ("echo $NAME", BashRiskFlag::DynamicExpansion),
        ] {
            let analysis = analyze(command);
            assert_eq!(
                analysis.classification,
                BashClassification::UnclassifiedHighRisk,
                "{command}"
            );
            assert!(analysis.risk_flags().contains(&flag), "{command}");
        }
    }

    #[test]
    fn normalized_scope_is_exact_and_operator_sensitive() {
        let first = analyze("git status");
        let second = analyze("git status --short");
        let compound = analyze("git status && git status");
        assert_ne!(first.normalized_scope(), second.normalized_scope());
        assert_ne!(first.normalized_scope(), compound.normalized_scope());
        assert_eq!(
            first.normalized_scope(),
            analyze("git 'status'").normalized_scope()
        );
    }

    #[test]
    fn bash_command_matcher_uses_exact_executable_and_argv_prefix() {
        let analysis = analyze("git status --short");
        let matcher = BashCommandMatcherConfig::new("git.exe", vec!["status".into()]);
        assert!(analysis.matches_bash_command(&matcher));
        assert!(
            !analysis.matches_bash_command(&BashCommandMatcherConfig::new(
                "git",
                vec!["statusx".into()]
            ))
        );
        assert!(!analyze("git stash").matches_bash_command(&matcher));
    }

    #[test]
    fn sanitized_projection_redacts_secret_values_and_bounds_display() {
        let analysis = analyze("git push --token=very-secret-value");
        assert!(!analysis.sanitized_preview().contains("very-secret-value"));
        assert!(!analysis.sanitized_scope().contains("very-secret-value"));
        assert!(analysis.sanitized_preview().len() <= MAX_DISPLAY_BYTES + 3);
        for command in [
            "curl --api-key PRIVATE_API_KEY example.invalid",
            "curl -u PRIVATE_USER:PRIVATE_PASSWORD example.invalid",
            "curl -H 'Authorization: Bearer PRIVATE_BEARER' example.invalid",
        ] {
            let display = analyze(command).sanitized_preview();
            assert!(!display.contains("PRIVATE_"), "secret in {command}");
        }
    }

    #[test]
    fn empty_or_unsupported_grammar_is_fail_closed() {
        for command in ["", "   ", "git &&", "| git", "(git status)"] {
            let analysis = analyze(command);
            assert_eq!(
                analysis.classification,
                BashClassification::UnclassifiedHighRisk,
                "{command:?}"
            );
        }
    }
}
