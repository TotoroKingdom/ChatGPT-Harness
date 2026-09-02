use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A documented empty object used by RPCs with no parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct EmptyParams {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Deny,
    Ask,
    Allow,
}
