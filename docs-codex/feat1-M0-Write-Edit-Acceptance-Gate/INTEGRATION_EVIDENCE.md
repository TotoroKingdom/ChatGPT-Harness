# M0 Integration Evidence — Write/Edit Capability

Date: 2026-09-29 (Asia/Shanghai)

Scope: local Runtime + MCP integration only. This record is not External ChatGPT E2E evidence and does not freeze M0.

## Entry criteria

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Command exited 0 |
| `cargo check --workspace` | PASS | Command exited 0; existing dead-code/unused-import warnings only |
| `cargo test --workspace` | PASS | Runtime 183 passed, UI 57 passed, Protocol 9 passed; 0 failed |
| `git diff --check` | PASS | Command exited 0; Git reported only the repository line-ending normalization warning |

## Case evidence

| Case | Result | Test command / test name | Evidence summary |
| --- | --- | --- | --- |
| INT-01 | PASS | `cargo test -p chatgpt-harness-runtime mcp::service::tests::ready_catalog_exposes_read_write_tools_and_excludes_unsupported_tools -- --exact --nocapture`; `mcp::service::tests::write_and_edit_builders_expose_the_declared_schema_and_mutation_annotations`; `mcp::http::tests::http_transport_exposes_available_tools_and_preserves_workspace_security`; `tools::registry::tests::normal_mode_descriptors_report_read_and_write_tools_as_ready` | Ready catalog order is `read_file`, `glob`, `write`, `edit`; write/edit metadata and annotations match the plan; unsupported tools are absent. |
| INT-02 | PASS | `mcp::service::tests::write_is_not_executable_before_runtime_acknowledgement`; `mcp::service::tests::tools_are_not_executable_before_runtime_acknowledgement`; `state::tests::effective_tools_are_available_only_after_runtime_acknowledgement` | NotReady write returns `RUNTIME_NOT_READY`, creates no file, and has no `ExecutionStarted`/`ExecutionCompleted`; effective catalog remains unavailable before acknowledgement. |
| INT-03 | PASS | `mcp::service::tests::write_and_edit_route_through_structured_success_results`; `mcp::service::tests::write_and_edit_use_existing_audit_lifecycle_and_boundary_rejection_has_no_start` | MCP write creates the workspace file with exact content and relative structured result; success lifecycle includes request, workspace, policy, start, complete. |
| INT-04 | PASS | `mcp::service::tests::write_overwrite_routes_through_mcp_and_reports_structured_flags`; `tools::write::tests::overwrites_existing_regular_file_and_reports_flags` | Existing regular file is fully overwritten; `created=false`, `overwritten=true`, exact bytes and `bytesWritten` asserted. |
| INT-05 | PASS | `mcp::service::tests::write_and_edit_route_through_structured_success_results`; `tools::edit::tests::replaces_exactly_one_match_and_reports_full_rewrite_bytes` | Exact single replacement succeeds, reports `replacements=1`, and filesystem content is the expected replacement. |
| INT-06 | PASS | `mcp::service::tests::write_and_edit_failures_are_sanitized_and_deterministic`; `tools::edit::tests::zero_and_multiple_matches_fail_without_modifying_the_file` | Zero match returns `EDIT_MATCH_NOT_FOUND`; bytes/content remain unchanged and no successful execution is asserted. |
| INT-07 | PASS | `mcp::service::tests::write_and_edit_failures_are_sanitized_and_deterministic`; `tools::edit::tests::zero_and_multiple_matches_fail_without_modifying_the_file` | Multiple match returns `EDIT_MATCH_NOT_UNIQUE`; no first-match guess and file remains unchanged. |
| INT-08 | PASS | `mcp::service::tests::mcp_write_boundary_rejections_preserve_filesystem_and_audit_boundary`; `tools::write::tests::relative_and_absolute_outside_targets_are_rejected`; `workspace::guard::tests::write_target_rejects_relative_absolute_and_sibling_prefix_escapes` | Relative and absolute outside writes return `WORKSPACE_VIOLATION`, sanitize outside paths, preserve outside bytes, and emit only request + failed workspace validation with no execution start/complete. |
| INT-09 | PASS | `mcp::service::tests::mcp_write_boundary_rejections_preserve_filesystem_and_audit_boundary`; `tools::write::tests::missing_parent_and_non_file_targets_fail_without_mutation`; `workspace::guard::tests::existing_write_target_is_approved_and_missing_parent_is_rejected` | Missing parent returns `TARGET_NOT_FOUND`; no directory or partial file is created. |
| INT-10 | PASS | `workspace::guard::tests::existing_approval_remains_missing_target_fail_closed`; `tools::edit::tests::missing_outside_and_directory_targets_fail`; `mcp::service::tests::glob_security_and_malformed_arguments_return_tool_errors`; `state::tests::write_and_edit_use_the_same_initialized_pipeline_as_mcp` | Existing-path approval still rejects missing targets; edit missing target fails; glob existing-base behavior remains guarded in the same Runtime snapshot. |
| INT-11 | PASS | `mcp::http::tests::http_transport_exposes_available_tools_and_preserves_workspace_security`; `mcp::service::tests::read_file_security_and_content_failures_are_sanitized_tool_errors`; `mcp::service::tests::glob_security_and_malformed_arguments_return_tool_errors` | Read/glob succeed inside the workspace and outside read/glob paths remain rejected and sanitized. |
| INT-12 | PASS | `mcp::service::tests::mcp_malformed_mutations_and_unsupported_edit_content_do_not_mutate`; `mcp::http::tests::malformed_requests_wrong_routes_and_oversized_bodies_do_not_stop_endpoint` | Missing fields, non-string fields, empty `oldText`, and unknown fields return `INVALID_ARGUMENTS`; baseline bytes remain unchanged. |
| INT-13 | PASS | `mcp::service::tests::mcp_malformed_mutations_and_unsupported_edit_content_do_not_mutate`; `tools::edit::tests::invalid_utf8_and_empty_old_text_fail_without_modifying_the_file` | Invalid UTF-8 edit returns `UNSUPPORTED_CONTENT`, sanitizes the result, and preserves original bytes. |
| INT-14 | PASS | `mcp::service::tests::mutation_audit_projection_excludes_raw_write_and_edit_payloads`; `mcp::service::tests::write_and_edit_use_existing_audit_lifecycle_and_boundary_rejection_has_no_start`; `activity::tests::runtime_and_mcp_share_the_sink_with_identical_audit_ids_and_no_payload_leaks`; `audit::redaction::tests::sensitive_argument_values_are_redacted_recursively` | Request/tool/workspace/policy/outcome evidence is present; raw write/edit markers, absolute workspace paths, and payload values are absent from Activity/Audit projections. |

## Windows boundary evidence

- Windows special path forms and root-relative path tests: PASS.
- Windows junction inside/outside boundary test: PASS; both junctions were created and checked.
- Directory symlink fixture: `ENVIRONMENT_SKIP` because the current test account could not create the symlink. This is not counted as a security PASS.
- File symlink revalidation fixture: `ENVIRONMENT_SKIP` because file symlink creation was unavailable. This is not counted as a security PASS.

## Integration decision

`INT-01` through `INT-14`: PASS. The explicitly identified Windows symlink fixture limitations are recorded as `ENVIRONMENT_SKIP` per the plan. Local Runtime/MCP integration is verified; External ChatGPT + Tunnel acceptance remains pending.
