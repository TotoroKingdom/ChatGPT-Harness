use std::path::Path;

pub(crate) fn sanitize_bounded_text(
    value: &str,
    workspace_root: Option<&Path>,
    max_chars: usize,
) -> String {
    let mut text = value.to_owned();
    if let Some(root) = workspace_root {
        let root = root.to_string_lossy();
        if !root.is_empty() {
            text = text.replace(root.as_ref(), "<workspace>");
            text = text.replace(&root.replace('\\', "/"), "<workspace>");
        }
    }
    text = redact_sk_tokens(&text);
    text = redact_url_credentials(&text);
    text = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(max_chars)
        .collect();
    text
}

fn redact_sk_tokens(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(index) = remaining.find("sk-") {
        output.push_str(&remaining[..index]);
        output.push_str("sk-[REDACTED]");
        let token = &remaining[index + 3..];
        let end = token
            .find(|character: char| {
                character.is_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | ')' | ']' | '}')
            })
            .unwrap_or(token.len());
        remaining = &token[end..];
    }
    output.push_str(remaining);
    output
}

fn redact_url_credentials(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(scheme) = remaining.find("://") {
        output.push_str(&remaining[..scheme + 3]);
        let after_scheme = &remaining[scheme + 3..];
        let authority_end = after_scheme
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '"' | '\'' | ')' | ']')
            })
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if let Some(at) = authority.rfind('@') {
            output.push_str("[REDACTED]@");
            output.push_str(&authority[at + 1..]);
        } else {
            output.push_str(authority);
        }
        remaining = &after_scheme[authority_end..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_paths_keys_and_url_credentials() {
        let root = Path::new("C:\\Temp\\m04-workspace");
        let value = format!(
            "{} sk-live-secret https://user:pass@example.test/path",
            root.display()
        );
        let sanitized = sanitize_bounded_text(&value, Some(root), 128);
        assert!(!sanitized.contains("C:\\Temp"));
        assert!(!sanitized.contains("live-secret"));
        assert!(!sanitized.contains("user:pass"));
        assert!(sanitized.contains("<workspace>"));
    }
}
