use serde::Deserialize;

pub(crate) const MCP_PROTOCOL_REVISION: &str = "2025-03-26";
pub(crate) const RMCP_VERSION: &str = "3.2.0";
pub(crate) const OFFICIAL_RUNNER_REPOSITORY: &str = "modelcontextprotocol/conformance";
pub(crate) const OFFICIAL_RUNNER_COMMIT: &str = "c37eec888e1c6ff140af79987a40008548b7cc5f";
pub(crate) const OFFICIAL_RUNNER_PACKAGE: &str = "0.2.0-alpha.12";
pub(crate) const OFFICIAL_RUNNER_SDK: &str = "1.29.0";
pub(crate) const STANDALONE_SDK_PACKAGE: &str = "@modelcontextprotocol/sdk";
pub(crate) const STANDALONE_SDK_VERSION: &str = "1.29.0";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompatibilityManifest {
    manifest_version: u32,
    protocol_revision: String,
    transport: String,
    runtime_sdk: String,
    official_runner: OfficialRunner,
    standalone_sdk: StandaloneSdk,
    required_coverage: Vec<String>,
    result_classification: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfficialRunner {
    repository: String,
    commit: String,
    package_version: String,
    sdk_version: String,
    requirements_set: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StandaloneSdk {
    package: String,
    version: String,
    package_lock_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_identity_matches_source_manifest_and_lock_contract() {
        let manifest: CompatibilityManifest = serde_json::from_str(include_str!(
            "../../../fixtures/v4/m10-mcp/compatibility-v1.json"
        ))
        .expect("M10 compatibility manifest is valid JSON");

        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.protocol_revision, MCP_PROTOCOL_REVISION);
        assert_eq!(manifest.transport, "streamable_http");
        assert_eq!(manifest.runtime_sdk, "rmcp 3.2.0");
        assert_eq!(
            manifest.official_runner.repository,
            OFFICIAL_RUNNER_REPOSITORY
        );
        assert_eq!(manifest.official_runner.commit, OFFICIAL_RUNNER_COMMIT);
        assert_eq!(
            manifest.official_runner.package_version,
            OFFICIAL_RUNNER_PACKAGE
        );
        assert_eq!(manifest.official_runner.sdk_version, OFFICIAL_RUNNER_SDK);
        assert!(manifest.official_runner.requirements_set.is_none());
        assert_eq!(manifest.standalone_sdk.package, STANDALONE_SDK_PACKAGE);
        assert_eq!(manifest.standalone_sdk.version, STANDALONE_SDK_VERSION);
        assert_eq!(
            manifest.standalone_sdk.package_lock_sha256,
            "C18EDC415FFBF3ABC171F5E4DB39DF26141FF1CE610CD728C425172FBCBBF34B"
        );
        assert!(manifest.required_coverage.contains(&"cancellation".into()));
        assert!(manifest
            .result_classification
            .contains(&"NOT_VERIFIED".into()));

        let cargo = include_str!("../../Cargo.toml");
        assert!(cargo.contains("rmcp = { version = \"3.2.0\""));

        let lock = include_str!("../../../Cargo.lock");
        let rmcp = lock
            .split("[[package]]")
            .find(|package| package.contains("name = \"rmcp\""))
            .expect("Cargo.lock contains rmcp");
        assert!(rmcp.contains("version = \"3.2.0\""));
    }
}
