use super::manifest::canonical_hash;
use super::mcp_client::exposed_tool_names;
use super::{EvalError, EvalResult, BASELINE_TOOLS};
use serde_json::{json, Value};

pub(crate) const M05_CANDIDATE_PROFILE_ID: &str = "m05-grep-candidate-v1";
pub(crate) const M05_PAIRED_CONTROL_PROFILE_ID: &str = "m05-paired-control-v1";
pub(crate) const M05_CANDIDATE_TOOLS: [&str; 5] = ["read_file", "glob", "grep", "write", "edit"];
/// The quality control catalog carried forward from M05.  This is deliberately
/// separate from `BASELINE_TOOLS`, which remains the frozen four-tool M04
/// catalog used by the older evaluator.
pub(crate) const M09_CONTROL_PROFILE_ID: &str = "m09-five-tool-control-v1";
pub(crate) const M09_FINAL_PROFILE_ID: &str = "m09-final-candidate-v1";
pub(crate) const M09_CONTROL_TOOLS: [&str; 5] = ["read_file", "glob", "grep", "write", "edit"];

pub(crate) const M09_CANDIDATE_PROFILE_IDS: [&str; 6] = [
    "m09-mkdir-candidate-v1",
    "m09-list-directory-candidate-v1",
    "m09-copy-candidate-v1",
    "m09-move-candidate-v1",
    "m09-rename-candidate-v1",
    "m09-delete-candidate-v1",
];

pub(crate) const M09_CANDIDATE_NAMES: [&str; 6] = [
    "mkdir",
    "list_directory",
    "copy",
    "move",
    "rename",
    "delete",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvalProfile {
    M04Baseline,
    M05GrepCandidate,
    M05PairedControl,
    M09FiveToolControl,
    M09MkdirCandidate,
    M09ListDirectoryCandidate,
    M09CopyCandidate,
    M09MoveCandidate,
    M09RenameCandidate,
    M09DeleteCandidate,
    M09FinalCandidate,
}

impl EvalProfile {
    pub(crate) fn parse(value: &str) -> EvalResult<Self> {
        match value {
            "m04-baseline" => Ok(Self::M04Baseline),
            M05_CANDIDATE_PROFILE_ID => Ok(Self::M05GrepCandidate),
            M05_PAIRED_CONTROL_PROFILE_ID => Ok(Self::M05PairedControl),
            M09_CONTROL_PROFILE_ID => Ok(Self::M09FiveToolControl),
            "m09-mkdir-candidate-v1" => Ok(Self::M09MkdirCandidate),
            "m09-list-directory-candidate-v1" => Ok(Self::M09ListDirectoryCandidate),
            "m09-copy-candidate-v1" => Ok(Self::M09CopyCandidate),
            "m09-move-candidate-v1" => Ok(Self::M09MoveCandidate),
            "m09-rename-candidate-v1" => Ok(Self::M09RenameCandidate),
            "m09-delete-candidate-v1" => Ok(Self::M09DeleteCandidate),
            M09_FINAL_PROFILE_ID => Ok(Self::M09FinalCandidate),
            _ => Err(EvalError::new(
                "INVALID_ARGUMENT",
                format!("unknown eval profile '{value}'"),
            )),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::M04Baseline => "m04-baseline",
            Self::M05GrepCandidate => M05_CANDIDATE_PROFILE_ID,
            Self::M05PairedControl => M05_PAIRED_CONTROL_PROFILE_ID,
            Self::M09FiveToolControl => M09_CONTROL_PROFILE_ID,
            Self::M09MkdirCandidate => M09_CANDIDATE_PROFILE_IDS[0],
            Self::M09ListDirectoryCandidate => M09_CANDIDATE_PROFILE_IDS[1],
            Self::M09CopyCandidate => M09_CANDIDATE_PROFILE_IDS[2],
            Self::M09MoveCandidate => M09_CANDIDATE_PROFILE_IDS[3],
            Self::M09RenameCandidate => M09_CANDIDATE_PROFILE_IDS[4],
            Self::M09DeleteCandidate => M09_CANDIDATE_PROFILE_IDS[5],
            Self::M09FinalCandidate => M09_FINAL_PROFILE_ID,
        }
    }

    pub(crate) fn tool_names(self) -> &'static [&'static str] {
        match self {
            Self::M04Baseline | Self::M05PairedControl => &BASELINE_TOOLS,
            Self::M05GrepCandidate => &M05_CANDIDATE_TOOLS,
            Self::M09FiveToolControl | Self::M09FinalCandidate => &M09_CONTROL_TOOLS,
            Self::M09MkdirCandidate => &M09_MKDIR_TOOLS,
            Self::M09ListDirectoryCandidate => &M09_LIST_DIRECTORY_TOOLS,
            Self::M09CopyCandidate => &M09_COPY_TOOLS,
            Self::M09MoveCandidate => &M09_MOVE_TOOLS,
            Self::M09RenameCandidate => &M09_RENAME_TOOLS,
            Self::M09DeleteCandidate => &M09_DELETE_TOOLS,
        }
    }

    pub(crate) fn grep_enabled(self) -> bool {
        self.tool_names().contains(&"grep")
    }

    pub(crate) fn m09_candidate(self) -> Option<&'static str> {
        match self {
            Self::M09MkdirCandidate => Some("mkdir"),
            Self::M09ListDirectoryCandidate => Some("list_directory"),
            Self::M09CopyCandidate => Some("copy"),
            Self::M09MoveCandidate => Some("move"),
            Self::M09RenameCandidate => Some("rename"),
            Self::M09DeleteCandidate => Some("delete"),
            _ => None,
        }
    }

    pub(crate) fn is_m09(self) -> bool {
        matches!(
            self,
            Self::M09FiveToolControl
                | Self::M09MkdirCandidate
                | Self::M09ListDirectoryCandidate
                | Self::M09CopyCandidate
                | Self::M09MoveCandidate
                | Self::M09RenameCandidate
                | Self::M09DeleteCandidate
                | Self::M09FinalCandidate
        )
    }

    /// Hash of the exact candidate schema used by the admission profile.  The
    /// schema is evaluation-only metadata; it does not add a Runtime tool.
    pub(crate) fn candidate_schema_hash(self) -> EvalResult<Option<String>> {
        self.m09_candidate()
            .map(|name| canonical_hash(&candidate_schema(name)))
            .transpose()
    }

    pub(crate) fn candidate_schema(self) -> Option<Value> {
        self.m09_candidate().map(candidate_schema)
    }

    pub(crate) fn validate_catalog(self, tools_result: &serde_json::Value) -> EvalResult<()> {
        if self == Self::M09FinalCandidate {
            return Err(EvalError::new(
                "BLOCKED",
                "m09-final-candidate-v1 requires an evidence-backed decision-set-v1 catalog",
            ));
        }
        let names = exposed_tool_names(tools_result)?;
        let expected = self
            .tool_names()
            .iter()
            .map(|tool| (*tool).to_owned())
            .collect::<Vec<_>>();
        if names != expected {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!(
                    "{} tools/list catalog drifted: expected {expected:?}, observed {names:?}",
                    self.id()
                ),
            ));
        }
        if names.iter().any(|name| name == "bash") {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("bash is forbidden in {}", self.id()),
            ));
        }
        if !self.grep_enabled() && names.iter().any(|name| name == "grep") {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("grep is forbidden in {}", self.id()),
            ));
        }
        Ok(())
    }

    /// Construct the exact final catalog only after a caller has loaded and
    /// validated decision-set-v1.  The decision artifact is deliberately not
    /// read from user-controlled Runtime configuration here.
    pub(crate) fn final_catalog(admitted: &[&str]) -> EvalResult<Vec<&'static str>> {
        let mut result = M09_CONTROL_TOOLS.to_vec();
        for candidate in M09_CANDIDATE_NAMES {
            if admitted.contains(&candidate) {
                result.push(candidate);
            }
        }
        if admitted
            .iter()
            .any(|name| !M09_CANDIDATE_NAMES.contains(name))
        {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "final catalog contains a non-M09 candidate",
            ));
        }
        Ok(result)
    }

    pub(crate) fn validate_final_catalog(observed: &[&str], admitted: &[&str]) -> EvalResult<()> {
        let expected = Self::final_catalog(admitted)?;
        if observed != expected.as_slice() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("final catalog drifted: expected {expected:?}, observed {observed:?}"),
            ));
        }
        Ok(())
    }
}

const M09_MKDIR_TOOLS: [&str; 6] = ["read_file", "glob", "grep", "write", "edit", "mkdir"];
const M09_LIST_DIRECTORY_TOOLS: [&str; 6] = [
    "read_file",
    "glob",
    "grep",
    "write",
    "edit",
    "list_directory",
];
const M09_COPY_TOOLS: [&str; 6] = ["read_file", "glob", "grep", "write", "edit", "copy"];
const M09_MOVE_TOOLS: [&str; 6] = ["read_file", "glob", "grep", "write", "edit", "move"];
const M09_RENAME_TOOLS: [&str; 6] = ["read_file", "glob", "grep", "write", "edit", "rename"];
const M09_DELETE_TOOLS: [&str; 6] = ["read_file", "glob", "grep", "write", "edit", "delete"];

fn candidate_schema(candidate: &str) -> Value {
    let (properties, required, capability, description) = match candidate {
        "mkdir" => (
            json!({
                "path": {"type": "string", "minLength": 1, "maxLength": 1024},
                "recursive": {"type": "boolean", "default": false}
            }),
            json!(["path"]),
            "workspace_write",
            "Create one workspace-relative directory without shell fallback.",
        ),
        "list_directory" => (
            json!({
                "path": {"type": "string", "minLength": 1, "maxLength": 1024, "default": "."},
                "maxEntries": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 200}
            }),
            json!([]),
            "workspace_read",
            "List one bounded, non-recursive workspace directory level.",
        ),
        "copy" => (
            json!({
                "sourcePath": {"type": "string", "minLength": 1, "maxLength": 1024},
                "destinationPath": {"type": "string", "minLength": 1, "maxLength": 1024}
            }),
            json!(["sourcePath", "destinationPath"]),
            "workspace_write",
            "Copy one regular file atomically without overwrite.",
        ),
        "move" => (
            json!({
                "sourcePath": {"type": "string", "minLength": 1, "maxLength": 1024},
                "destinationPath": {"type": "string", "minLength": 1, "maxLength": 1024}
            }),
            json!(["sourcePath", "destinationPath"]),
            "workspace_write",
            "Move one file or ordinary directory on the same volume.",
        ),
        "rename" => (
            json!({
                "path": {"type": "string", "minLength": 1, "maxLength": 1024},
                "newName": {"type": "string", "minLength": 1, "maxLength": 255}
            }),
            json!(["path", "newName"]),
            "workspace_write",
            "Rename one workspace entry within its existing parent.",
        ),
        "delete" => (
            json!({"path": {"type": "string", "minLength": 1, "maxLength": 1024}}),
            json!(["path"]),
            "workspace_write",
            "Delete one regular file or empty directory; recursive deletion is unsupported.",
        ),
        _ => unreachable!("unknown M09 candidate"),
    };
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
        "x-harness-capability": capability,
        "description": description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(names: &[&str]) -> serde_json::Value {
        json!({
            "tools": names.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()
        })
    }

    #[test]
    fn baseline_and_paired_control_keep_the_frozen_four_tool_catalog() {
        let baseline = catalog(&BASELINE_TOOLS);
        assert!(EvalProfile::M04Baseline.validate_catalog(&baseline).is_ok());
        assert!(EvalProfile::M05PairedControl
            .validate_catalog(&baseline)
            .is_ok());
        let with_grep = catalog(&M05_CANDIDATE_TOOLS);
        assert_eq!(
            EvalProfile::M04Baseline
                .validate_catalog(&with_grep)
                .unwrap_err()
                .code,
            "HARNESS_CONTRACT_ERROR"
        );
        assert_eq!(
            EvalProfile::M05PairedControl
                .validate_catalog(&with_grep)
                .unwrap_err()
                .code,
            "HARNESS_CONTRACT_ERROR"
        );
    }

    #[test]
    fn candidate_requires_exact_five_tool_catalog_without_bash() {
        assert!(EvalProfile::M05GrepCandidate
            .validate_catalog(&catalog(&M05_CANDIDATE_TOOLS))
            .is_ok());
        for drift in [
            catalog(&BASELINE_TOOLS),
            catalog(&["read_file", "glob", "grep", "write", "edit", "bash"]),
            catalog(&["read_file", "glob", "grep", "write"]),
        ] {
            assert_eq!(
                EvalProfile::M05GrepCandidate
                    .validate_catalog(&drift)
                    .unwrap_err()
                    .code,
                "HARNESS_CONTRACT_ERROR"
            );
        }
    }

    #[test]
    fn final_profile_is_blocked_without_decision_truth_and_catalog_is_explicit() {
        let error = EvalProfile::M09FinalCandidate
            .validate_catalog(&catalog(&M09_CONTROL_TOOLS))
            .unwrap_err();
        assert_eq!(error.code, "BLOCKED");
        assert_eq!(
            EvalProfile::final_catalog(&["copy", "mkdir"]).unwrap(),
            vec![
                "read_file",
                "glob",
                "grep",
                "write",
                "edit",
                "mkdir",
                "copy"
            ]
        );
        assert!(EvalProfile::validate_final_catalog(
            &[
                "read_file",
                "glob",
                "grep",
                "write",
                "edit",
                "mkdir",
                "copy"
            ],
            &["mkdir", "copy"]
        )
        .is_ok());
    }

    #[test]
    fn profile_ids_are_versioned_and_parse_strictly() {
        for profile in [
            EvalProfile::M04Baseline,
            EvalProfile::M05GrepCandidate,
            EvalProfile::M05PairedControl,
            EvalProfile::M09FiveToolControl,
            EvalProfile::M09MkdirCandidate,
            EvalProfile::M09ListDirectoryCandidate,
            EvalProfile::M09CopyCandidate,
            EvalProfile::M09MoveCandidate,
            EvalProfile::M09RenameCandidate,
            EvalProfile::M09DeleteCandidate,
            EvalProfile::M09FinalCandidate,
        ] {
            assert_eq!(EvalProfile::parse(profile.id()).unwrap(), profile);
        }
        assert!(EvalProfile::parse("m05-latest").is_err());
    }
}
