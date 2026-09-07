use serde_json::{Map, Value};

/// Redact a future argument projection before it can cross the audit boundary.
/// The current AuditEvent does not accept arguments at all; this helper makes
/// that constraint explicit for later callers.
#[allow(dead_code)]
pub(crate) fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value = if is_sensitive_key(key) {
                        Value::String("[REDACTED]".into())
                    } else {
                        redact_value(value)
                    };
                    (key.clone(), value)
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "secret",
        "apikey",
        "credential",
        "password",
        "token",
        "authorization",
    ]
    .contains(&normalized.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sensitive_argument_values_are_redacted_recursively() {
        let input = json!({
            "secret": "SECRET-VALUE",
            "api_key": "API-KEY-VALUE",
            "credential": "CREDENTIAL-VALUE",
            "nested": [{"password": "PASSWORD-VALUE"}],
            "safe": "visible"
        });
        let redacted = redact_value(&input).to_string();
        for secret in [
            "SECRET-VALUE",
            "API-KEY-VALUE",
            "CREDENTIAL-VALUE",
            "PASSWORD-VALUE",
        ] {
            assert!(!redacted.contains(secret));
        }
        assert!(redacted.contains("visible"));
    }
}
