//! Conservative, quote-aware facts for Bash policy evaluation.
//!
//! This module deliberately does not produce an executable command.  The
//! caller keeps the original request for execution while this analyzer emits
//! only bounded logical facts used by Policy/Approval.  Anything outside the
//! small supported grammar is classified high risk and therefore receives at
//! least the ASK policy floor.

use harness_control_protocol::{
    normalize_bash_executable, BashCommandMatcherConfig, BashDestructiveReason,
};
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
    pub(crate) destructive: Vec<BashDestructiveReason>,
    pub(crate) redirects: Vec<BashRedirect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BashRedirect {
    pub(crate) fd: Option<String>,
    pub(crate) operator: String,
    pub(crate) target: String,
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

    pub(crate) fn is_destructive(&self) -> bool {
        self.segments
            .iter()
            .any(|segment| !segment.destructive.is_empty())
    }

    pub(crate) fn destructive_reasons(&self) -> Vec<BashDestructiveReason> {
        let mut reasons = self
            .segments
            .iter()
            .flat_map(|segment| segment.destructive.iter().copied())
            .collect::<Vec<_>>();
        reasons.sort();
        reasons.dedup();
        reasons
    }

    /// Returns a stable, compact representation of the exact normalized
    /// command shape.  Segments, operators, executable identity, logical argv
    /// and bounded redirect facts participate; risk metadata is excluded.
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
            redirects: &'a [BashRedirect],
        }

        let segments = self
            .segments
            .iter()
            .map(|segment| ScopeSegment {
                executable: &segment.executable,
                argv: &segment.argv,
                redirects: &segment.redirects,
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
            #[serde(skip_serializing_if = "Vec::is_empty")]
            redirects: Vec<DisplayRedirect>,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct DisplayRedirect {
            #[serde(skip_serializing_if = "Option::is_none")]
            fd: Option<String>,
            operator: String,
            target: String,
        }

        let segments = self
            .segments
            .iter()
            .map(|segment| DisplaySegment {
                executable: sanitize_executable(&segment.executable),
                argv: sanitize_argv(segment),
                redirects: segment
                    .redirects
                    .iter()
                    .map(|redirect| DisplayRedirect {
                        fd: redirect.fd.clone(),
                        operator: redirect.operator.clone(),
                        target: sanitize_redirect_target(&redirect.target),
                    })
                    .collect(),
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
                    && segment.redirects.iter().all(|redirect| {
                        sanitize_redirect_target(&redirect.target) == redirect.target
                    })
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
            for redirect in &segment.redirects {
                output.push(' ');
                if let Some(fd) = &redirect.fd {
                    output.push_str(fd);
                }
                output.push_str(&redirect.operator);
                output.push_str(&quote_display_token(&sanitize_redirect_target(
                    &redirect.target,
                )));
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
    let mut segment_destructive: Vec<BTreeSet<BashDestructiveReason>> = vec![BTreeSet::new()];
    let mut segment_redirects: Vec<Vec<BashRedirect>> = vec![Vec::new()];
    let mut operators = Vec::new();
    let mut current = String::new();
    let mut current_flags = BTreeSet::new();
    let mut current_quoted = false;
    let mut current_escaped = false;
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
                if current.is_empty() && !token_started {
                    current_quoted = false;
                    current_escaped = false;
                }
                if character.is_control() {
                    current_flags.insert(BashRiskFlag::UnsupportedGrammar);
                    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
                }
                if character == '\'' {
                    quote = Quote::Single;
                    current_quoted = true;
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character == '"' {
                    quote = Quote::Double;
                    current_quoted = true;
                    token_started = true;
                    index += 1;
                    continue;
                }
                if character == '\\' {
                    current_escaped = true;
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
                    let fd_prefix = if token_started
                        && current_flags.is_empty()
                        && !current_quoted
                        && !current_escaped
                        && !current.is_empty()
                        && current.chars().all(|character| character.is_ascii_digit())
                    {
                        token_started = false;
                        current_quoted = false;
                        current_escaped = false;
                        Some(std::mem::take(&mut current))
                    } else {
                        flush_token(
                            &mut current,
                            &mut current_flags,
                            token_started,
                            &mut segments,
                        );
                        token_started = false;
                        None
                    };
                    let parsed =
                        parse_redirection(&chars, index, fd_prefix.as_deref(), &mut global_flags);
                    if let Some(reason) = parsed.destructive {
                        segment_destructive
                            .last_mut()
                            .expect("segments and destructive facts stay aligned")
                            .insert(reason);
                    }
                    if let Some(redirect) = parsed.redirect {
                        segment_redirects
                            .last_mut()
                            .expect("segments and redirect facts stay aligned")
                            .push(redirect);
                    }
                    index = parsed.next;
                    continue;
                }
                if character == '&' {
                    if chars.get(index + 1) == Some(&'>') {
                        flush_token(
                            &mut current,
                            &mut current_flags,
                            token_started,
                            &mut segments,
                        );
                        token_started = false;
                        let parsed = parse_redirection(&chars, index, None, &mut global_flags);
                        if let Some(reason) = parsed.destructive {
                            segment_destructive
                                .last_mut()
                                .expect("segments and destructive facts stay aligned")
                                .insert(reason);
                        }
                        if let Some(redirect) = parsed.redirect {
                            segment_redirects
                                .last_mut()
                                .expect("segments and redirect facts stay aligned")
                                .push(redirect);
                        }
                        index = parsed.next;
                        continue;
                    }
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
                        segment_destructive.push(BTreeSet::new());
                        segment_redirects.push(Vec::new());
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
                        segment_destructive.push(BTreeSet::new());
                        segment_redirects.push(Vec::new());
                        index += 2;
                    } else {
                        operators.push(BashOperator::Pipe);
                        segments.push(Vec::new());
                        segment_destructive.push(BTreeSet::new());
                        segment_redirects.push(Vec::new());
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
                    segment_destructive.push(BTreeSet::new());
                    segment_redirects.push(Vec::new());
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
        .zip(segment_destructive)
        .zip(segment_redirects)
        .map(|((tokens, destructive), redirects)| {
            analyze_segment(tokens, destructive, redirects, &mut global_flags)
        })
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
            destructive: Vec::new(),
            redirects: Vec::new(),
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
    mut destructive: BTreeSet<BashDestructiveReason>,
    redirects: Vec<BashRedirect>,
    global_flags: &mut BTreeSet<BashRiskFlag>,
) -> BashCommandSegment {
    if tokens.is_empty() {
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
        return BashCommandSegment {
            executable: String::new(),
            argv: Vec::new(),
            risk_flags: Vec::new(),
            destructive: Vec::new(),
            redirects: Vec::new(),
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
    let (detected, wrapper_unsupported) = detect_destructive(executable_name, &argv);
    destructive.extend(detected);
    if wrapper_unsupported {
        risk_flags.insert(BashRiskFlag::UnsupportedGrammar);
    }
    BashCommandSegment {
        executable,
        argv,
        risk_flags: risk_flags.into_iter().collect(),
        destructive: destructive.into_iter().collect(),
        redirects,
    }
}

struct ParsedRedirection {
    next: usize,
    destructive: Option<BashDestructiveReason>,
    redirect: Option<BashRedirect>,
}

fn parse_redirection(
    chars: &[char],
    index: usize,
    fd_prefix: Option<&str>,
    global_flags: &mut BTreeSet<BashRiskFlag>,
) -> ParsedRedirection {
    let first = chars[index];
    let mut operator = String::new();
    let mut cursor = index;

    if first == '&' {
        operator.push('&');
        cursor += 1;
        if chars.get(cursor) == Some(&'>') {
            operator.push('>');
            cursor += 1;
            if chars.get(cursor) == Some(&'>') {
                operator.push('>');
                cursor += 1;
            }
        } else {
            mark_unsupported_redirection(global_flags);
            return ParsedRedirection {
                next: cursor,
                destructive: None,
                redirect: None,
            };
        }
    } else {
        operator.push(first);
        cursor += 1;
        if let Some(next) = chars.get(cursor).copied() {
            if next == first {
                operator.push(next);
                cursor += 1;
                if first == '<' && chars.get(cursor) == Some(&'<') {
                    operator.push('<');
                    cursor += 1;
                }
            } else if next == '&' {
                operator.push('&');
                cursor += 1;
            } else if first == '>' && next == '|' {
                operator.push('|');
                cursor += 1;
            } else if first == '<' && next == '>' {
                operator.push('>');
                cursor += 1;
            }
        }
    }

    if operator == "<<" || operator == "<<<" {
        mark_unsupported_redirection(global_flags);
        return ParsedRedirection {
            next: chars.len(),
            destructive: None,
            redirect: None,
        };
    }
    if operator.starts_with(">>>") {
        mark_unsupported_redirection(global_flags);
        return ParsedRedirection {
            next: cursor,
            destructive: None,
            redirect: None,
        };
    }
    if chars.get(cursor) == Some(&'(') {
        mark_unsupported_redirection(global_flags);
        global_flags.insert(BashRiskFlag::UnsupportedGrammar);
        return ParsedRedirection {
            next: cursor,
            destructive: None,
            redirect: None,
        };
    }

    if operator == "<&" || operator == ">&" {
        if let Some((target, next)) = parse_fd_target(chars, cursor) {
            return ParsedRedirection {
                next,
                destructive: None,
                redirect: Some(BashRedirect {
                    fd: fd_prefix.map(str::to_owned),
                    operator,
                    target,
                }),
            };
        }
        mark_unsupported_redirection(global_flags);
        return ParsedRedirection {
            next: cursor,
            destructive: None,
            redirect: None,
        };
    }

    let Some(target) = parse_redirect_target(chars, cursor) else {
        mark_unsupported_redirection(global_flags);
        return ParsedRedirection {
            next: cursor,
            destructive: None,
            redirect: None,
        };
    };
    if target.dynamic {
        mark_unsupported_redirection(global_flags);
        global_flags.insert(BashRiskFlag::DynamicExpansion);
        return ParsedRedirection {
            next: target.next,
            destructive: None,
            redirect: None,
        };
    }
    if target.malformed {
        mark_unsupported_redirection(global_flags);
        global_flags.insert(BashRiskFlag::MalformedQuoting);
        return ParsedRedirection {
            next: target.next,
            destructive: None,
            redirect: None,
        };
    }
    if operator.starts_with('<') && operator != "<>" || is_safe_redirect_target(&target.value) {
        return ParsedRedirection {
            next: target.next,
            destructive: None,
            redirect: Some(BashRedirect {
                fd: fd_prefix.map(str::to_owned),
                operator,
                target: target.value,
            }),
        };
    }
    ParsedRedirection {
        next: target.next,
        destructive: Some(BashDestructiveReason::Overwrite),
        redirect: Some(BashRedirect {
            fd: fd_prefix.map(str::to_owned),
            operator,
            target: target.value,
        }),
    }
}

fn mark_unsupported_redirection(global_flags: &mut BTreeSet<BashRiskFlag>) {
    global_flags.insert(BashRiskFlag::Redirection);
    global_flags.insert(BashRiskFlag::UnsupportedGrammar);
}

fn is_safe_redirect_target(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace('\\', "/")
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "/dev/null" | "nul" | "nul:" | "con" | "con:"
    )
}

struct RedirectTarget {
    value: String,
    next: usize,
    dynamic: bool,
    malformed: bool,
}

fn parse_redirect_target(chars: &[char], start: usize) -> Option<RedirectTarget> {
    let mut cursor = start;
    while chars
        .get(cursor)
        .is_some_and(|character| character.is_ascii_whitespace())
    {
        cursor += 1;
    }
    let mut value = String::new();
    let mut quote = Quote::None;
    let mut dynamic = false;
    let mut malformed = false;

    while let Some(&character) = chars.get(cursor) {
        match quote {
            Quote::Single => {
                if character == '\'' {
                    quote = Quote::None;
                } else {
                    value.push(character);
                }
                cursor += 1;
            }
            Quote::Double => {
                if character == '"' {
                    quote = Quote::None;
                    cursor += 1;
                    continue;
                }
                if character == '\\' {
                    if let Some(&next) = chars.get(cursor + 1) {
                        match next {
                            '"' | '\\' | '$' | '`' => {
                                value.push(next);
                                cursor += 2;
                                continue;
                            }
                            '\n' | '\r' => {
                                cursor += 2;
                                if next == '\r' && chars.get(cursor) == Some(&'\n') {
                                    cursor += 1;
                                }
                                continue;
                            }
                            _ => {}
                        }
                    } else {
                        malformed = true;
                        cursor += 1;
                        continue;
                    }
                }
                if character == '$' || character == '`' {
                    dynamic = true;
                }
                if character.is_control() {
                    malformed = true;
                }
                value.push(character);
                cursor += 1;
            }
            Quote::None => {
                if character.is_ascii_whitespace()
                    || matches!(character, '<' | '>' | '|' | '&' | ';')
                {
                    break;
                }
                if character == '\'' {
                    quote = Quote::Single;
                    cursor += 1;
                    continue;
                }
                if character == '"' {
                    quote = Quote::Double;
                    cursor += 1;
                    continue;
                }
                if character == '\\' {
                    if let Some(&next) = chars.get(cursor + 1) {
                        match next {
                            '\n' | '\r' => {
                                cursor += 2;
                                if next == '\r' && chars.get(cursor) == Some(&'\n') {
                                    cursor += 1;
                                }
                                continue;
                            }
                            _ => {
                                value.push(next);
                                cursor += 2;
                                continue;
                            }
                        }
                    }
                    malformed = true;
                    cursor += 1;
                    continue;
                }
                if character == '$' || character == '`' {
                    dynamic = true;
                } else if matches!(
                    character,
                    '*' | '?' | '[' | ']' | '~' | '!' | '(' | ')' | '{' | '}'
                ) {
                    dynamic = true;
                }
                if character.is_control() {
                    malformed = true;
                }
                value.push(character);
                cursor += 1;
            }
        }
    }

    if quote != Quote::None {
        malformed = true;
    }
    if value.is_empty() {
        return None;
    }
    Some(RedirectTarget {
        value,
        next: cursor,
        dynamic,
        malformed,
    })
}

fn parse_fd_target(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut cursor = start;
    while chars
        .get(cursor)
        .is_some_and(|character| character.is_ascii_whitespace())
    {
        cursor += 1;
    }
    let mut value = String::new();
    while let Some(&character) = chars.get(cursor) {
        if character.is_ascii_digit() || character == '-' {
            value.push(character);
            cursor += 1;
        } else {
            break;
        }
    }
    if value.is_empty() {
        return None;
    }
    if let Some(&next) = chars.get(cursor) {
        if !next.is_ascii_whitespace() && !matches!(next, '<' | '>' | '|' | '&' | ';') {
            return None;
        }
    }
    Some((value, cursor))
}

fn detect_destructive(
    executable_name: &str,
    argv: &[String],
) -> (BTreeSet<BashDestructiveReason>, bool) {
    let mut reasons = BTreeSet::new();
    let mut wrapper_unsupported = false;
    let name = executable_name
        .trim_end_matches(".exe")
        .to_ascii_lowercase();

    if matches!(
        name.as_str(),
        "rm" | "rmdir"
            | "unlink"
            | "shred"
            | "sdelete"
            | "del"
            | "erase"
            | "rd"
            | "remove-item"
            | "ri"
    ) {
        reasons.insert(BashDestructiveReason::Delete);
    }
    if name == "truncate" || name == "dd" {
        reasons.insert(BashDestructiveReason::Overwrite);
    }
    if name == "format"
        || name.starts_with("mkfs")
        || matches!(name.as_str(), "wipefs" | "diskpart" | "fdisk" | "parted")
    {
        reasons.insert(BashDestructiveReason::Disk);
    }
    if matches!(name.as_str(), "tee" | "set-content" | "clear-content") {
        reasons.insert(BashDestructiveReason::Overwrite);
    }
    if name == "sed"
        && argv
            .iter()
            .any(|argument| argument == "--in-place" || argument.starts_with("-i"))
    {
        reasons.insert(BashDestructiveReason::Overwrite);
    }
    if name == "perl"
        && argv
            .iter()
            .any(|argument| argument.starts_with('-') && argument.contains('i'))
    {
        reasons.insert(BashDestructiveReason::Overwrite);
    }
    if name == "git" {
        reasons.extend(git_destructive(argv));
    }
    if name == "find" {
        if argv.iter().any(|argument| argument == "-delete") {
            reasons.insert(BashDestructiveReason::Delete);
        }
        if argv
            .iter()
            .any(|argument| argument == "-exec" || argument == "-execdir")
        {
            wrapper_unsupported = true;
            if let Some(command) = find_exec_command(argv) {
                reasons.extend(destructive_for_wrapped_command(command, &[]));
            }
        }
    }
    if name == "xargs" {
        wrapper_unsupported = true;
        if let Some((command, args)) = wrapped_command(argv) {
            reasons.extend(destructive_for_wrapped_command(command, args));
        }
    }
    if matches!(
        name.as_str(),
        "sudo"
            | "doas"
            | "runas"
            | "su"
            | "chroot"
            | "nice"
            | "nohup"
            | "timeout"
            | "stdbuf"
            | "setsid"
            | "ionice"
            | "watch"
            | "parallel"
            | "busybox"
            | "toybox"
            | "wsl"
            | "ssh"
    ) {
        wrapper_unsupported = true;
    }

    (reasons, wrapper_unsupported)
}

fn git_destructive(argv: &[String]) -> BTreeSet<BashDestructiveReason> {
    let mut reasons = BTreeSet::new();
    let Some(subcommand) = argv.first().map(String::as_str) else {
        return reasons;
    };
    let rest = &argv[1..];
    match subcommand {
        "rm" => {
            reasons.insert(BashDestructiveReason::Delete);
        }
        "clean"
            if rest.iter().any(|argument| {
                argument == "--force" || argument.starts_with('-') && argument.contains('f')
            }) =>
        {
            reasons.insert(BashDestructiveReason::Delete);
        }
        "reset" if rest.iter().any(|argument| argument == "--hard") => {
            reasons.insert(BashDestructiveReason::Repository);
        }
        "checkout"
            if rest.iter().any(|argument| {
                argument == "-f" || argument == "--force" || argument == "--" || argument == "."
            }) =>
        {
            reasons.insert(BashDestructiveReason::Repository);
        }
        "restore" => {
            reasons.insert(BashDestructiveReason::Repository);
        }
        "stash"
            if rest
                .first()
                .is_some_and(|argument| argument == "drop" || argument == "clear") =>
        {
            reasons.insert(BashDestructiveReason::Delete);
        }
        "branch"
            if rest.iter().any(|argument| argument == "-D")
                || (rest.iter().any(|argument| argument == "--delete")
                    && rest
                        .iter()
                        .any(|argument| argument == "-f" || argument == "--force")) =>
        {
            reasons.insert(BashDestructiveReason::Delete);
        }
        "push"
            if rest.iter().any(|argument| {
                argument == "-f"
                    || argument == "--force"
                    || argument == "--delete"
                    || argument == "--force-with-lease"
                    || argument.starts_with("--force-with-lease=")
            }) =>
        {
            reasons.insert(BashDestructiveReason::Repository);
        }
        "filter-branch" => {
            reasons.insert(BashDestructiveReason::Repository);
        }
        "worktree" if rest.first().is_some_and(|argument| argument == "remove") => {
            reasons.insert(BashDestructiveReason::Delete);
        }
        _ => {}
    }
    reasons
}

fn wrapped_command(argv: &[String]) -> Option<(&str, &[String])> {
    for (index, argument) in argv.iter().enumerate() {
        if argument.starts_with('-') {
            continue;
        }
        return Some((argument.as_str(), &argv[index + 1..]));
    }
    None
}

fn find_exec_command(argv: &[String]) -> Option<&str> {
    let index = argv
        .iter()
        .position(|argument| argument == "-exec" || argument == "-execdir")?;
    argv.get(index + 1).map(String::as_str)
}

fn destructive_for_wrapped_command(
    command: &str,
    argv: &[String],
) -> BTreeSet<BashDestructiveReason> {
    let name = command
        .rsplit('/')
        .next()
        .unwrap_or(command)
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    let mut reasons = BTreeSet::new();
    if matches!(
        name.as_str(),
        "rm" | "rmdir" | "unlink" | "shred" | "sdelete" | "del" | "erase" | "rd"
    ) {
        reasons.insert(BashDestructiveReason::Delete);
    }
    if name == "git" {
        reasons.extend(git_destructive(argv));
    }
    reasons
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

fn sanitize_redirect_target(target: &str) -> String {
    if target.len() > MAX_DISPLAY_BYTES / 2
        || target.chars().any(|character| character.is_control())
        || contains_publicly_sensitive_shape(target)
        || is_secret_key(target)
    {
        REDACTED.into()
    } else {
        target.into()
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
            ("cat <<EOF\nhi\nEOF", BashRiskFlag::Redirection),
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
    fn safe_redirections_are_classified_and_not_destructive() {
        for command in [
            "rg -n foo 2>/dev/null",
            "rg -n foo > /dev/null",
            "rg -n foo 2>&1 | head -100",
            "rg -n foo >NUL",
            "rg -n foo </dev/null",
            "rg -n foo &>/dev/null",
        ] {
            let analysis = analyze(command);
            assert!(analysis.is_classified(), "{command}");
            assert!(!analysis.is_destructive(), "{command}");
        }
    }

    #[test]
    fn file_output_redirection_is_classified_destructive() {
        for command in [
            "echo hi > out.txt",
            "echo hi >> out.txt",
            "echo hi 2> out.txt",
            "echo hi &> out.txt",
        ] {
            let analysis = analyze(command);
            assert!(analysis.is_classified(), "{command}");
            assert!(analysis.is_destructive(), "{command}");
            assert!(
                analysis
                    .destructive_reasons()
                    .contains(&BashDestructiveReason::Overwrite),
                "{command}"
            );
        }
    }

    #[test]
    fn dynamic_or_heredoc_redirection_stays_unclassified() {
        for command in [
            "echo hi > $FILE",
            "echo hi > *.txt",
            "cat <<EOF\nhi\nEOF",
            "diff <(echo a) <(echo b)",
        ] {
            assert_eq!(
                analyze(command).classification,
                BashClassification::UnclassifiedHighRisk,
                "{command}"
            );
        }
    }

    #[test]
    fn destructive_commands_are_detected_across_segments() {
        for command in [
            "rm -rf build",
            "del foo.txt",
            "git clean -fd",
            "git reset --hard HEAD",
            "git push --force",
            "git push --delete old",
            "git restore .",
            "git checkout .",
            "git stash drop",
            "git branch -D old",
            "find . -delete",
        ] {
            let analysis = analyze(command);
            assert!(analysis.is_destructive(), "{command}");
        }
        for command in [
            "cd module-platform && rg foo | head -100",
            "git status",
            "git diff",
            "npm test",
            "cat < input.txt",
        ] {
            let analysis = analyze(command);
            assert!(analysis.is_classified(), "{command}");
            assert!(!analysis.is_destructive(), "{command}");
        }
    }

    #[test]
    fn wrappers_that_can_hide_arbitrary_execution_stay_unclassified() {
        for command in [
            "sudo rm -rf /",
            "wsl rm -rf /",
            "xargs rm -rf",
            "find . -exec sh -c 'rm -rf /' ;",
        ] {
            assert_eq!(
                analyze(command).classification,
                BashClassification::UnclassifiedHighRisk,
                "{command}"
            );
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
    fn normalized_scope_keeps_redirect_targets_exact() {
        assert_ne!(
            analyze("echo hi > a.txt").normalized_scope(),
            analyze("echo hi > b.txt").normalized_scope()
        );
        assert_ne!(
            analyze("echo hi").normalized_scope(),
            analyze("echo hi > a.txt").normalized_scope()
        );
        assert_eq!(
            analyze("echo hi > a.txt").normalized_scope(),
            analyze("echo hi > 'a.txt'").normalized_scope()
        );
        assert!(analyze("echo hi > a.txt")
            .normalized_scope()
            .contains("a.txt"));
    }

    #[test]
    fn quoted_digits_are_arguments_not_file_descriptors() {
        let analysis = analyze("echo '2'>out.txt");
        assert_eq!(segment(&analysis, 0).argv, ["2"]);
        assert_eq!(segment(&analysis, 0).redirects[0].fd, None);

        let escaped = analyze("echo 2\\3>out.txt");
        assert_eq!(segment(&escaped, 0).argv, ["23"]);
        assert_eq!(segment(&escaped, 0).redirects[0].fd, None);
    }

    #[test]
    fn sanitized_scope_redacts_secret_redirect_targets() {
        let analysis = analyze("echo hi > token=PRIVATE_REDIRECT_SECRET");
        assert!(!analysis
            .sanitized_scope()
            .contains("PRIVATE_REDIRECT_SECRET"));
        assert!(!analysis
            .sanitized_preview()
            .contains("PRIVATE_REDIRECT_SECRET"));
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
