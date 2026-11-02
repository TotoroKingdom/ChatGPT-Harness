use super::EvalResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdapterClass {
    Formal,
    NonFormal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModelToolSpec {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) input_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConversationItem {
    User {
        text: String,
    },
    AssistantToolCalls {
        tool_calls: Vec<ModelToolCall>,
    },
    ToolResult {
        call_id: String,
        name: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The exact model-emitted argument string, preserved even when it is not
    /// valid JSON. Runner never repairs an emitted call.
    pub(crate) arguments: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModelToolCallArguments {
    pub(crate) raw: String,
    pub(crate) parsed: Option<Value>,
}

impl ModelToolCall {
    pub(crate) fn parsed_arguments(&self) -> ModelToolCallArguments {
        ModelToolCallArguments {
            raw: self.arguments.clone(),
            parsed: serde_json::from_str(&self.arguments).ok(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) total_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cost_usd: Option<f64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ModelRequest {
    pub(crate) system_prompt: String,
    pub(crate) items: Vec<ConversationItem>,
    pub(crate) tools: Vec<ModelToolSpec>,
    pub(crate) max_output_tokens: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ModelTurn {
    pub(crate) resolved_model: String,
    #[allow(dead_code)]
    pub(crate) api_surface: Option<String>,
    pub(crate) tool_calls: Vec<ModelToolCall>,
    pub(crate) final_text: Option<String>,
    pub(crate) usage: ProviderUsage,
    #[allow(dead_code)]
    pub(crate) stop_reason: String,
    pub(crate) attempts: u32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterErrorKind {
    ProviderUnavailable,
    RateLimited,
    ServerError,
    TransportReset,
    InvalidResponse,
    Cancelled,
    Configuration,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct AdapterError {
    pub(crate) kind: AdapterErrorKind,
    pub(crate) message: String,
    pub(crate) retryable_before_output: bool,
}

impl AdapterError {
    pub(crate) fn new(
        kind: AdapterErrorKind,
        message: impl Into<String>,
        retryable_before_output: bool,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable_before_output,
        }
    }
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AdapterError {}

#[async_trait]
pub(crate) trait ModelAdapter: Send {
    fn adapter_id(&self) -> &str;
    fn adapter_version(&self) -> &str;
    fn requested_model(&self) -> &str;
    fn class(&self) -> AdapterClass;
    async fn respond(&mut self, request: &ModelRequest) -> Result<ModelTurn, AdapterError>;
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ScriptedTurn {
    pub(crate) tool_calls: Vec<ModelToolCall>,
    pub(crate) final_text: Option<String>,
}

/// Deterministic mechanics adapter. It never contacts a provider and must be
/// labelled NON_FORMAL in every result.
pub(crate) struct ScriptedAdapter {
    turns: VecDeque<ScriptedTurn>,
    resolved_model: String,
}

impl ScriptedAdapter {
    pub(crate) fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self {
            turns: turns.into(),
            resolved_model: "scripted-local".into(),
        }
    }
}

#[async_trait]
impl ModelAdapter for ScriptedAdapter {
    fn adapter_id(&self) -> &str {
        super::manifest::SCRIPTED_ADAPTER_ID
    }

    fn adapter_version(&self) -> &str {
        "1"
    }

    fn requested_model(&self) -> &str {
        "scripted-local"
    }

    fn class(&self) -> AdapterClass {
        AdapterClass::NonFormal
    }

    async fn respond(&mut self, _request: &ModelRequest) -> Result<ModelTurn, AdapterError> {
        let turn = self.turns.pop_front().ok_or_else(|| {
            AdapterError::new(
                AdapterErrorKind::InvalidResponse,
                "scripted adapter exhausted its turns",
                false,
            )
        })?;
        Ok(ModelTurn {
            resolved_model: self.resolved_model.clone(),
            api_surface: Some("scripted".into()),
            tool_calls: turn.tool_calls,
            final_text: turn.final_text,
            usage: ProviderUsage::default(),
            stop_reason: "scripted".into(),
            attempts: 1,
        })
    }
}

pub(crate) fn scripted_call(id: impl Into<String>, name: &str, arguments: Value) -> ModelToolCall {
    ModelToolCall {
        id: id.into(),
        name: name.into(),
        arguments: arguments.to_string(),
    }
}

pub(crate) fn model_request(
    system_prompt: impl Into<String>,
    user_text: impl Into<String>,
    tools: Vec<ModelToolSpec>,
) -> ModelRequest {
    ModelRequest {
        system_prompt: system_prompt.into(),
        items: vec![ConversationItem::User {
            text: user_text.into(),
        }],
        tools,
        max_output_tokens: super::MAX_OUTPUT_TOKENS_PER_TURN,
    }
}

pub(crate) fn ensure_formal_adapter(adapter: &dyn ModelAdapter) -> EvalResult<()> {
    if adapter.class() != AdapterClass::Formal {
        return Err(super::EvalError::new(
            "NON_FORMAL",
            "scripted/mechanics adapter cannot satisfy formal baseline execution",
        ));
    }
    Ok(())
}
