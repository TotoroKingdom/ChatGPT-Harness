use super::config::RuntimeConfigSnapshot;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

/// Only non-security-critical optional behavior may appear here.
pub type Capabilities = BTreeMap<String, bool>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: ClientInfo,
    pub protocol_version: u32,
    pub capabilities: Capabilities,
    pub runtime_config: RuntimeConfigSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub host_context: Option<super::history::RuntimeHostContext>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub server_info: ServerInfo,
    pub protocol_version: u32,
    pub runtime_id: String,
    pub capabilities: Capabilities,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InitializedParams {}
