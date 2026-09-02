use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use ts_rs::TS;

/// A non-negative 64-bit integer represented as a decimal string on the JSON wire.
///
/// Keeping revisions and cursors as strings avoids JSON number precision loss and
/// makes the representation identical across Tauri and future JSONL transports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema, TS)]
#[schemars(with = "String")]
#[ts(type = "string")]
pub struct DecimalU64(u64);

impl DecimalU64 {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_add_one(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl From<u64> for DecimalU64 {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DecimalU64> for u64 {
    fn from(value: DecimalU64) -> Self {
        value.0
    }
}

impl FromStr for DecimalU64 {
    type Err = DecimalU64ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DecimalU64ParseError);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| DecimalU64ParseError)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalU64ParseError;

impl fmt::Display for DecimalU64ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a non-negative decimal u64 string")
    }
}

impl std::error::Error for DecimalU64ParseError {}

impl Serialize for DecimalU64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DecimalU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DecimalU64Visitor;

        impl<'de> de::Visitor<'de> for DecimalU64Visitor {
            type Value = DecimalU64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-negative decimal u64 string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .parse()
                    .map_err(|error: DecimalU64ParseError| E::custom(error.to_string()))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(value)
            }
        }

        deserializer.deserialize_str(DecimalU64Visitor)
    }
}

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
