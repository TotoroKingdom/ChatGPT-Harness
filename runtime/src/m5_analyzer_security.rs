//! Independent checks for analyzer uncertainties that must never authorize
//! automatic execution or widen a reusable grant.
use crate::bash::analyzer::analyze;

#[test]
fn shell_semantics_outside_static_argv_never_become_classified() {
    for command in [
        "echo $$",
        "echo $?",
        "echo $@",
        "echo $*",
        "echo $0",
        "echo $!",
        "echo $-",
        "git $'status'",
        "git $\"status\"",
        "echo ~",
        "echo ~/private",
        "MODE=unsafe git status",
        "env git status",
        "command git status",
        "builtin eval 'git push'",
        "source ./script",
        "exec git push",
        "trap 'git push' EXIT; git status",
        "alias git='git push'; git status",
        "hash -p ./other-program git; git status",
        "enable -f ./extension.so custom",
        "declare -i value='1+2'",
        "let 'value=1+2'",
        "! git push",
        "time git push",
        "if true; then git push; fi",
        "for x in a; do git push; done",
        "node -eprocess.exit(0)",
        "node --eval=process.exit(0)",
        "perl -eprint",
        "php -recho",
        "python3 -cprint(0)",
        "bash ./script",
        "pwsh -Command Write-Output",
        "cmd /c echo hello",
    ] {
        let facts = analyze(command);
        assert!(!facts.is_classified(), "unsafe classification: {command:?}");
        assert!(
            !facts.allow_for_session(),
            "unsafe reusable grant: {command:?}"
        );
    }
}

#[test]
fn newline_cannot_be_authorized_as_extra_arguments_to_the_first_executable() {
    for command in ["git status\nshutdown", "git status\r\nshutdown"] {
        let facts = analyze(command);
        if facts.is_classified() {
            assert_eq!(facts.segments.len(), 2);
            assert_eq!(facts.segments[0].executable, "git");
            assert_eq!(facts.segments[0].argv, ["status"]);
            assert_eq!(facts.segments[1].executable, "shutdown");
            assert_eq!(facts.operators.len(), 1);
        } else {
            assert!(!facts.allow_for_session());
        }
    }
}

#[test]
fn line_continuation_is_decoded_like_bash_and_literal_quotes_remain_static() {
    assert_eq!(
        analyze("git sta\\\ntus").normalized_scope(),
        analyze("git status").normalized_scope()
    );
    assert_eq!(
        analyze("git \"sta\\\ntus\"").normalized_scope(),
        analyze("git status").normalized_scope()
    );
    for command in [
        "echo '$@'",
        "echo '$?'",
        "echo '&&'",
        "echo eval",
        "git status --short",
    ] {
        assert!(
            analyze(command).is_classified(),
            "literal misclassified: {command:?}"
        );
    }
}

#[test]
fn grant_shape_preserves_paths_arguments_and_operators() {
    let normal = analyze("git status --short");
    assert!(normal.is_classified());
    assert_ne!(
        normal.normalized_scope(),
        analyze("./git status --short").normalized_scope()
    );
    for other in [
        "git status",
        "git status --branch",
        "git push",
        "git status --short && git status",
    ] {
        assert_ne!(
            normal.normalized_scope(),
            analyze(other).normalized_scope(),
            "{other}"
        );
    }
    assert_eq!(
        normal.normalized_scope(),
        analyze("GIT.EXE 'status' --short").normalized_scope()
    );
}

#[test]
fn display_preview_keeps_literal_operators_quoted() {
    for token in [
        "&&",
        "||",
        ";",
        "|",
        "$(shutdown)",
        "$HOME",
        "<input",
        "a'b",
    ] {
        let quoted = token.replace('\'', "'\\''");
        let facts = analyze(&format!("echo '{quoted}'"));
        assert!(facts.is_classified());
        assert!(
            facts.sanitized_preview().contains('\''),
            "literal operator lost display quotes: {token}"
        );
    }
}

#[test]
fn opaque_lowercase_hex_credentials_stay_only_in_private_grant_scope() {
    let secret = "0123456789abcdef0123456789abcdef";
    let facts = analyze(&format!("echo {secret}"));
    assert!(facts.is_classified());
    assert!(facts.normalized_scope().contains(secret));
    assert!(!facts.sanitized_preview().contains(secret));
    assert!(!facts.sanitized_scope().contains(secret));
    assert!(!facts.sanitized_scope_is_complete());
}

#[test]
fn public_preview_is_bounded_utf8_and_secret_flags_are_redacted() {
    for command in [
        format!("{} argument", "界".repeat(1500)),
        format!("printf{}", " 界".repeat(500)),
    ] {
        let facts = analyze(&command);
        let preview = facts.sanitized_preview();
        assert!(preview.len() <= 1100);
    }
    for command in [
        "git --password PRIVATE_PASSWORD_VALUE status",
        "git --token=PRIVATE_TOKEN_VALUE status",
        "git --api-key PRIVATE_API_VALUE status",
        "curl --user alice:PRIVATE_CREDENTIAL_VALUE example.invalid",
        "curl -H 'Authorization: Bearer PRIVATE_BEARER_VALUE' example.invalid",
    ] {
        let facts = analyze(command);
        let display = format!("{} {}", facts.sanitized_preview(), facts.sanitized_scope());
        assert!(!display.contains("PRIVATE_"), "secret in public facts");
    }
}
