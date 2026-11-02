use super::adapter::{
    AdapterClass, AdapterError, AdapterErrorKind, ConversationItem, ModelAdapter, ModelRequest,
    ModelToolCall, ModelTurn, ProviderUsage,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

pub(crate) const API_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub(crate) const API_KEY_ENV: &str = "M04_EVAL_OPENAI_API_KEY";

#[derive(Clone, Debug)]
pub(crate) struct TransportResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TransportError {
    pub(crate) message: String,
    pub(crate) retryable_before_output: bool,
}

impl TransportError {
    pub(crate) fn new(message: impl Into<String>, retryable_before_output: bool) -> Self {
        Self {
            message: message.into(),
            retryable_before_output,
        }
    }
}

#[async_trait]
pub(crate) trait ResponsesTransport: Send + Sync {
    async fn send(&self, api_key: &str, body: &Value) -> Result<TransportResponse, TransportError>;
}

pub(crate) struct HttpResponsesTransport {
    client: reqwest::Client,
    endpoint: String,
}

impl HttpResponsesTransport {
    pub(crate) fn new() -> Result<Self, AdapterError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| {
                AdapterError::new(
                    AdapterErrorKind::Configuration,
                    format!("could not create provider HTTP client: {error}"),
                    false,
                )
            })?;
        Ok(Self {
            client,
            endpoint: API_ENDPOINT.into(),
        })
    }
}

#[async_trait]
impl ResponsesTransport for HttpResponsesTransport {
    async fn send(&self, api_key: &str, body: &Value) -> Result<TransportResponse, TransportError> {
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .json(body)
            .send()
            .await
            .map_err(|error| {
                TransportError::new(
                    format!("provider transport failed: {error}"),
                    error.is_timeout() || error.is_connect() || error.is_request(),
                )
            })?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|error| {
            TransportError::new(format!("provider response body failed: {error}"), false)
        })?;
        Ok(TransportResponse { status, body })
    }
}

pub(crate) struct OpenAiResponsesAdapter {
    transport: Arc<dyn ResponsesTransport>,
    api_key: String,
    requested_model: String,
}

impl OpenAiResponsesAdapter {
    pub(crate) fn from_env() -> Result<Self, AdapterError> {
        let api_key = std::env::var(API_KEY_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AdapterError::new(
                    AdapterErrorKind::Configuration,
                    format!("{API_KEY_ENV} is required for the real adapter"),
                    false,
                )
            })?;
        Ok(Self {
            transport: Arc::new(HttpResponsesTransport::new()?),
            api_key,
            requested_model: super::manifest::REQUESTED_MODEL.into(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_transport(
        api_key: impl Into<String>,
        transport: Arc<dyn ResponsesTransport>,
    ) -> Self {
        Self {
            transport,
            api_key: api_key.into(),
            requested_model: super::manifest::REQUESTED_MODEL.into(),
        }
    }

    fn request_body(&self, request: &ModelRequest) -> Value {
        let input = request
            .items
            .iter()
            .flat_map(conversation_item_to_input)
            .collect::<Vec<_>>();
        let tools = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                    "strict": false,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "model": self.requested_model,
            "instructions": request.system_prompt,
            "input": input,
            "tools": tools,
            "tool_choice": "auto",
            "max_output_tokens": request.max_output_tokens,
            "store": false,
        })
    }
}

fn conversation_item_to_input(item: &ConversationItem) -> Vec<Value> {
    match item {
        ConversationItem::User { text } => vec![json!({
            "role": "user",
            "content": [{ "type": "input_text", "text": text }]
        })],
        ConversationItem::AssistantToolCalls { tool_calls } => tool_calls
            .iter()
            .map(|call| {
                json!({
                    "type": "function_call",
                    "call_id": call.id,
                    "name": call.name,
                    "arguments": call.arguments,
                })
            })
            .collect(),
        ConversationItem::ToolResult {
            call_id, content, ..
        } => vec![json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": content,
        })],
    }
}

#[async_trait]
impl ModelAdapter for OpenAiResponsesAdapter {
    fn adapter_id(&self) -> &str {
        super::manifest::ADAPTER_ID
    }

    fn adapter_version(&self) -> &str {
        super::manifest::ADAPTER_VERSION
    }

    fn requested_model(&self) -> &str {
        &self.requested_model
    }

    fn class(&self) -> AdapterClass {
        AdapterClass::Formal
    }

    async fn respond(&mut self, request: &ModelRequest) -> Result<ModelTurn, AdapterError> {
        let body = self.request_body(request);
        let mut attempts = 0;
        loop {
            attempts += 1;
            let response = match self.transport.send(&self.api_key, &body).await {
                Ok(response) => response,
                Err(error) => {
                    if attempts == 1 && error.retryable_before_output {
                        continue;
                    }
                    return Err(AdapterError::new(
                        AdapterErrorKind::ProviderUnavailable,
                        error.message,
                        error.retryable_before_output,
                    ));
                }
            };
            if (response.status == 429 || (500..=599).contains(&response.status)) && attempts == 1 {
                continue;
            }
            return parse_response(response, &self.requested_model, attempts);
        }
    }
}

fn parse_response(
    response: TransportResponse,
    requested_model: &str,
    attempts: u32,
) -> Result<ModelTurn, AdapterError> {
    let value: Value = serde_json::from_str(&response.body).map_err(|_| {
        AdapterError::new(
            if response.status == 429 {
                AdapterErrorKind::RateLimited
            } else if (500..=599).contains(&response.status) {
                AdapterErrorKind::ServerError
            } else {
                AdapterErrorKind::InvalidResponse
            },
            format!(
                "provider returned non-JSON response with HTTP {}",
                response.status
            ),
            false,
        )
    })?;
    if response.status == 429 {
        return Err(AdapterError::new(
            AdapterErrorKind::RateLimited,
            "provider rate limited the request after one retry",
            false,
        ));
    }
    if !(200..300).contains(&response.status) {
        let message = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("provider request failed");
        return Err(AdapterError::new(
            if (500..=599).contains(&response.status) {
                AdapterErrorKind::ServerError
            } else {
                AdapterErrorKind::ProviderUnavailable
            },
            message,
            false,
        ));
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    if status == "failed" {
        return Err(AdapterError::new(
            AdapterErrorKind::ProviderUnavailable,
            "provider reported a failed response",
            false,
        ));
    }
    let resolved_model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(requested_model)
        .to_owned();
    let mut tool_calls = Vec::new();
    let mut text_parts = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            match item.get("type").and_then(Value::as_str) {
                Some("function_call") => {
                    let call_id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let arguments = match item.get("arguments") {
                        Some(Value::String(value)) => value.clone(),
                        Some(value) => value.to_string(),
                        None => String::new(),
                    };
                    tool_calls.push(ModelToolCall {
                        id: call_id,
                        name,
                        arguments,
                    });
                }
                Some("message") => {
                    if let Some(content) = item.get("content").and_then(Value::as_array) {
                        for block in content {
                            if block.get("type").and_then(Value::as_str) == Some("output_text") {
                                if let Some(text) = block.get("text").and_then(Value::as_str) {
                                    text_parts.push(text.to_owned());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if text_parts.is_empty() {
        if let Some(text) = value.get("output_text").and_then(Value::as_str) {
            text_parts.push(text.to_owned());
        }
    }
    let usage = parse_usage(value.get("usage"));
    Ok(ModelTurn {
        resolved_model,
        api_surface: Some("responses-v1".into()),
        tool_calls,
        final_text: if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        },
        usage,
        stop_reason: value
            .get("incomplete_details")
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or(status)
            .to_owned(),
        attempts,
    })
}

fn parse_usage(value: Option<&Value>) -> ProviderUsage {
    let Some(value) = value else {
        return ProviderUsage::default();
    };
    ProviderUsage {
        input_tokens: value.get("input_tokens").and_then(Value::as_u64),
        output_tokens: value.get("output_tokens").and_then(Value::as_u64),
        total_tokens: value.get("total_tokens").and_then(Value::as_u64),
        cost_usd: value
            .get("cost")
            .or_else(|| value.get("total_cost"))
            .and_then(Value::as_f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::adapter::ModelToolSpec;
    use std::sync::Mutex;

    struct FakeTransport {
        responses: Mutex<Vec<Result<TransportResponse, TransportError>>>,
        requests: Mutex<Vec<Value>>,
    }

    #[async_trait]
    impl ResponsesTransport for FakeTransport {
        async fn send(
            &self,
            _api_key: &str,
            body: &Value,
        ) -> Result<TransportResponse, TransportError> {
            self.requests.lock().unwrap().push(body.clone());
            self.responses.lock().unwrap().remove(0)
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            system_prompt: "system".into(),
            items: vec![ConversationItem::User {
                text: "task".into(),
            }],
            tools: vec![ModelToolSpec {
                name: "read_file".into(),
                description: "read".into(),
                input_schema: json!({"type": "object"}),
            }],
            max_output_tokens: 2048,
        }
    }

    #[tokio::test]
    async fn parses_tool_calls_and_preserves_raw_arguments() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(vec![Ok(TransportResponse {
                status: 200,
                body: json!({
                    "model": "gpt-6.1-terra-2026-01-01",
                    "status": "completed",
                    "output": [
                        {
                            "type": "function_call",
                            "call_id": "call-1",
                            "name": "read_file",
                            "arguments": "{\"path\":\"a.txt\"}"
                        },
                        {
                            "type": "message",
                            "content": [{"type": "output_text", "text": "PORT=1"}]
                        }
                    ],
                    "usage": {"input_tokens": 10, "output_tokens": 4, "total_tokens": 14}
                })
                .to_string(),
            })]),
            requests: Mutex::new(Vec::new()),
        });
        let mut adapter = OpenAiResponsesAdapter::with_transport("test-key", transport.clone());
        let turn = adapter.respond(&request()).await.unwrap();
        assert_eq!(turn.resolved_model, "gpt-6.1-terra-2026-01-01");
        assert_eq!(turn.tool_calls[0].arguments, "{\"path\":\"a.txt\"}");
        assert_eq!(turn.final_text.as_deref(), Some("PORT=1"));
        assert_eq!(turn.usage.total_tokens, Some(14));
        assert_eq!(turn.stop_reason, "completed");
        let body = &transport.requests.lock().unwrap()[0];
        assert!(body.get("temperature").is_none());
        assert!(body.get("seed").is_none());
        assert_eq!(body["store"], false);
    }

    #[tokio::test]
    async fn retries_once_for_429_and_then_succeeds() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(vec![
                Ok(TransportResponse {
                    status: 429,
                    body: "{}".into(),
                }),
                Ok(TransportResponse {
                    status: 200,
                    body: json!({
                        "model": "gpt-6.1-terra",
                        "status": "completed",
                        "output": [{"type": "message", "content": [{"type": "output_text", "text": "DONE"}]}]
                    })
                    .to_string(),
                }),
            ]),
            requests: Mutex::new(Vec::new()),
        });
        let mut adapter = OpenAiResponsesAdapter::with_transport("test-key", transport);
        let turn = adapter.respond(&request()).await.unwrap();
        assert_eq!(turn.attempts, 2);
        assert_eq!(turn.final_text.as_deref(), Some("DONE"));
    }

    #[tokio::test]
    async fn provider_error_does_not_echo_credentials() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(vec![Ok(TransportResponse {
                status: 401,
                body: json!({"error": {"message": "invalid authentication"}}).to_string(),
            })]),
            requests: Mutex::new(Vec::new()),
        });
        let mut adapter = OpenAiResponsesAdapter::with_transport("sk-secret", transport);
        let error = adapter.respond(&request()).await.unwrap_err();
        assert!(!error.message.contains("sk-secret"));
    }

    #[tokio::test]
    async fn continuation_and_multiple_calls_are_encoded_deterministically() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(vec![Ok(TransportResponse {
                status: 200,
                body: json!({
                    "model": "gpt-6.1-terra",
                    "status": "completed",
                    "output": [
                        {"type": "function_call", "call_id": "c1", "name": "read_file", "arguments": "{\"path\":\"a\"}"},
                        {"type": "function_call", "call_id": "c2", "name": "glob", "arguments": "{\"pattern\":\"**/*\"}"}
                    ]
                })
                .to_string(),
            })]),
            requests: Mutex::new(Vec::new()),
        });
        let mut adapter = OpenAiResponsesAdapter::with_transport("key", transport.clone());
        let mut request = request();
        request.items.push(ConversationItem::AssistantToolCalls {
            tool_calls: vec![ModelToolCall {
                id: "c0".into(),
                name: "read_file".into(),
                arguments: "{\"path\":\"x\"}".into(),
            }],
        });
        request.items.push(ConversationItem::ToolResult {
            call_id: "c0".into(),
            name: "read_file".into(),
            content: "{\"content\":\"x\"}".into(),
            is_error: false,
        });
        let turn = adapter.respond(&request).await.unwrap();
        assert_eq!(turn.tool_calls.len(), 2);
        let body = &transport.requests.lock().unwrap()[0];
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(body["max_output_tokens"], 2048);
    }
}
