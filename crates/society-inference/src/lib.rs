//! OpenAI-compatible inference providers, defaulting to a local LM Studio server.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

/// Default OpenAI-compatible base URL used by LM Studio.
pub const DEFAULT_BASE_URL: &str = "http://localhost:1234/v1";

/// Connection and routing settings for an OpenAI-compatible provider.
#[derive(Clone, Debug)]
pub struct ProviderConfig {
    /// API base URL, such as `http://localhost:1234/v1` or a remote HTTPS URL.
    pub base_url: String,
    /// Model identifier; if omitted, the provider selects the first ID from `/models`.
    pub model: Option<String>,
    /// Optional bearer token, normally read from an environment variable.
    pub api_key: Option<String>,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: None,
            api_key: None,
            timeout: Duration::from_secs(120),
        }
    }
}

/// A message in an OpenAI-compatible chat completion request.
#[derive(Clone, Debug, Serialize)]
pub struct ChatMessage {
    /// Message role (`system`, `user`, or `assistant`).
    pub role: String,
    /// Message text.
    pub content: String,
}

impl ChatMessage {
    /// Creates a chat message.
    #[must_use]
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

/// Chat generation parameters.
#[derive(Clone, Debug)]
pub struct ChatRequest {
    /// Conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Maximum output tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Optional OpenAI-compatible response-format request, such as JSON mode.
    pub response_format: Option<serde_json::Value>,
    /// Optional reasoning budget supported by some compatible servers (for example LM Studio).
    pub reasoning_effort: Option<String>,
}

/// Text and usage information returned by an OpenAI-compatible chat endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct ChatResponse {
    /// Assistant response text.
    pub content: String,
    /// Provider/model identifier reported by the server.
    pub model: Option<String>,
    /// Prompt token count when the endpoint reports it.
    pub prompt_tokens: Option<u64>,
    /// Completion token count when the endpoint reports it.
    pub completion_tokens: Option<u64>,
    /// Reason the provider stopped generation, when it reports one.
    pub finish_reason: Option<String>,
}

/// A model reported by the provider's `/models` endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ModelInfo {
    /// Provider-specific model identifier.
    pub id: String,
}

/// Common operations supported by OpenAI-compatible inference providers.
#[async_trait]
pub trait InferenceProvider: Send + Sync {
    /// Lists models visible to the endpoint.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, InferenceError>;

    /// Generates one chat completion.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}

/// OpenAI-compatible provider using `/models` and `/chat/completions`.
pub struct OpenAiCompatibleProvider {
    base_url: Url,
    api_key: Option<String>,
    client: Client,
    resolved_model: OnceCell<String>,
}

impl OpenAiCompatibleProvider {
    /// Creates a provider and validates its URL and timeout settings.
    pub fn new(config: ProviderConfig) -> Result<Self, InferenceError> {
        let mut base_url = Url::parse(config.base_url.trim())
            .map_err(|error| InferenceError::Configuration(error.to_string()))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(InferenceError::Configuration(
                "base URL must use HTTP or HTTPS".to_owned(),
            ));
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(InferenceError::Configuration(
                "credentials must be supplied through the API key environment variable, not the URL".to_owned(),
            ));
        }
        if base_url.scheme() == "http" && !is_loopback_host(base_url.host_str()) {
            return Err(InferenceError::Configuration(
                "non-loopback inference endpoints must use HTTPS".to_owned(),
            ));
        }
        if base_url.query().is_some() || base_url.fragment().is_some() {
            return Err(InferenceError::Configuration(
                "base URL must not contain a query or fragment".to_owned(),
            ));
        }
        let path = base_url.path().trim_end_matches('/');
        let path = if path.is_empty() { "/" } else { path };
        base_url.set_path(&format!("{path}/"));
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| InferenceError::Configuration(error.to_string()))?;
        let resolved_model = OnceCell::new();
        if let Some(model) = config
            .model
            .as_ref()
            .filter(|model| !model.trim().is_empty())
        {
            let _ = resolved_model.set(model.clone());
        }
        Ok(Self {
            base_url,
            api_key: config.api_key.filter(|key| !key.trim().is_empty()),
            client,
            resolved_model,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, InferenceError> {
        self.base_url
            .join(path)
            .map_err(|error| InferenceError::Configuration(error.to_string()))
    }

    fn with_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(api_key) = &self.api_key {
            request.bearer_auth(api_key)
        } else {
            request
        }
    }

    async fn selected_model(&self) -> Result<&str, InferenceError> {
        if let Some(model) = self.resolved_model.get() {
            return Ok(model);
        }
        let model = self
            .resolved_model
            .get_or_try_init(|| async {
                self.list_models()
                    .await?
                    .into_iter()
                    .next()
                    .map(|model| model.id)
                    .ok_or_else(|| {
                        InferenceError::InvalidResponse(
                            "provider returned no models; pass --model explicitly".to_owned(),
                        )
                    })
            })
            .await?;
        Ok(model)
    }
}

fn is_loopback_host(host: Option<&str>) -> bool {
    match host {
        Some("localhost") => true,
        Some(host) => host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    }
}

#[async_trait]
impl InferenceProvider for OpenAiCompatibleProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>, InferenceError> {
        let url = self.endpoint("models")?;
        let response = self.with_auth(self.client.get(url)).send().await?;
        let response = checked_response(response).await?;
        let response: ModelsEnvelope = response.json().await?;
        Ok(response.data)
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        let model = self.selected_model().await?.to_owned();
        let mut body = CompletionRequest {
            model: model.clone(),
            messages: request.messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            response_format: request.response_format,
            reasoning_effort: request.reasoning_effort,
        };
        const MAX_ADAPTIVE_OUTPUT_TOKENS: u32 = 8_192;
        loop {
            let mut response = self
                .with_auth(
                    self.client
                        .post(self.endpoint("chat/completions")?)
                        .json(&body),
                )
                .send()
                .await?;
            if response.status() == StatusCode::BAD_REQUEST && body.response_format.is_some() {
                let _ = response.bytes().await;
                body.response_format = None;
                response = self
                    .with_auth(
                        self.client
                            .post(self.endpoint("chat/completions")?)
                            .json(&body),
                    )
                    .send()
                    .await?;
            }
            let response = checked_response(response).await?;
            let response: CompletionEnvelope = response.json().await?;
            let choice = response.choices.into_iter().next().ok_or_else(|| {
                InferenceError::InvalidResponse("completion has no choices".to_owned())
            })?;
            if choice.finish_reason.as_deref() == Some("length")
                && body.max_tokens < MAX_ADAPTIVE_OUTPUT_TOKENS
            {
                body.max_tokens = body
                    .max_tokens
                    .saturating_mul(2)
                    .min(MAX_ADAPTIVE_OUTPUT_TOKENS);
                continue;
            }
            if choice.message.content.trim().is_empty() {
                let finish_reason = choice.finish_reason.as_deref().unwrap_or("not reported");
                return Err(InferenceError::InvalidResponse(format!(
                    "model `{}` returned empty assistant content after an output budget of {} tokens (finish reason: {finish_reason}); check the model's LM Studio load configuration",
                    model, body.max_tokens
                )));
            }
            return Ok(ChatResponse {
                content: choice.message.content,
                model: response.model,
                prompt_tokens: response
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.prompt_tokens),
                completion_tokens: response.usage.and_then(|usage| usage.completion_tokens),
                finish_reason: choice.finish_reason,
            });
        }
    }
}

async fn checked_response(
    response: reqwest::Response,
) -> Result<reqwest::Response, InferenceError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    let body: String = body.chars().take(2_000).collect();
    Err(InferenceError::HttpStatus { status, body })
}

#[derive(Clone, Serialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Deserialize)]
struct ModelsEnvelope {
    #[serde(default)]
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct CompletionEnvelope {
    model: Option<String>,
    choices: Vec<CompletionChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CompletionMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

/// Errors returned while configuring or calling an inference provider.
#[derive(Debug)]
pub enum InferenceError {
    /// URL or request settings are invalid.
    Configuration(String),
    /// The HTTP request failed before a response was received.
    Transport(reqwest::Error),
    /// The provider returned a non-success status.
    HttpStatus {
        /// HTTP status code.
        status: StatusCode,
        /// Truncated response body.
        body: String,
    },
    /// The provider returned malformed or incomplete JSON.
    InvalidResponse(String),
}

impl From<reqwest::Error> for InferenceError {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error)
    }
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) => write!(formatter, "inference configuration: {message}"),
            Self::Transport(error) => write!(formatter, "inference connection failed: {error}"),
            Self::HttpStatus { status, body } => {
                write!(formatter, "inference server returned {status}: {body}")
            }
            Self::InvalidResponse(message) => {
                write!(formatter, "invalid inference response: {message}")
            }
        }
    }
}

impl std::error::Error for InferenceError {}
