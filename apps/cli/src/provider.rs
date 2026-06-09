use std::{
    env,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use switchyard_core::{ChatRequest, ChatResponse, Message, MessageRole, Model, ProviderClient};

const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";
const DEFAULT_LLAMA_CPP_BASE_URL: &str = "http://localhost:8080";
const DEFAULT_LLAMA_CPP_MODEL: &str = "local-model";

pub(crate) enum LocalProvider {
    Ollama(OllamaProvider),
    LlamaCpp(LlamaCppProvider),
}

pub(crate) struct OllamaProvider {
    base_url: String,
    client: Client,
}

pub(crate) struct LlamaCppProvider {
    base_url: String,
    client: Client,
}

pub(crate) enum StreamEvent {
    InitialResponse {
        status: u16,
        initial_response_time: Duration,
    },
    ReasoningChunk(String),
    Chunk(String),
    Complete {
        total_time: Duration,
    },
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaStreamResponse {
    message: Option<OllamaResponseMessage>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    #[serde(default)]
    content: String,
    thinking: Option<String>,
}

#[derive(Serialize)]
struct LlamaCppChatRequest {
    model: String,
    messages: Vec<LlamaCppMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct LlamaCppMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct LlamaCppStreamResponse {
    choices: Vec<LlamaCppChoice>,
}

#[derive(Deserialize)]
struct LlamaCppChoice {
    delta: LlamaCppDelta,
}

#[derive(Deserialize)]
struct LlamaCppDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

impl LocalProvider {
    pub(crate) fn from_env() -> Self {
        Self::from_name(
            env::var("SWITCHYARD_PROVIDER")
                .unwrap_or_else(|_| "ollama".to_string())
                .as_str(),
        )
    }

    pub(crate) fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "llama.cpp" | "llamacpp" | "llama-cpp" => Self::LlamaCpp(LlamaCppProvider::from_env()),
            _ => Self::Ollama(OllamaProvider::from_env()),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Ollama(_) => "Ollama",
            Self::LlamaCpp(_) => "llama.cpp",
        }
    }

    pub(crate) fn model_from_env() -> Model {
        Self::default_model_for(
            env::var("SWITCHYARD_PROVIDER")
                .unwrap_or_else(|_| "ollama".to_string())
                .as_str(),
        )
    }

    pub(crate) fn default_model_for(provider_name: &str) -> Model {
        match provider_name.to_ascii_lowercase().as_str() {
            "llama.cpp" | "llamacpp" | "llama-cpp" => llama_cpp_model_from_env(),
            _ => ollama_model_from_env(),
        }
    }

    pub(crate) fn chat_url(&self) -> String {
        match self {
            Self::Ollama(provider) => provider.chat_url(),
            Self::LlamaCpp(provider) => provider.chat_url(),
        }
    }

    pub(crate) async fn stream_async(
        &self,
        request: ChatRequest,
        on_event: impl FnMut(StreamEvent),
    ) -> Result<()> {
        match self {
            Self::Ollama(provider) => provider.stream_async(request, on_event).await,
            Self::LlamaCpp(provider) => provider.stream_async(request, on_event).await,
        }
    }
}

impl OllamaProvider {
    pub(crate) fn from_env() -> Self {
        Self {
            base_url: env_with_fallback(
                "SWITCHYARD_OLLAMA_BASE_URL",
                "OLLAMA_BASE_URL",
                DEFAULT_OLLAMA_BASE_URL,
            )
            .trim_end_matches('/')
            .to_string(),
            client: Client::new(),
        }
    }

    pub(crate) fn chat_url(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }

    pub(crate) async fn send_async(&self, request: ChatRequest) -> Result<ChatResponse> {
        let mut content = String::new();
        self.stream_async(request, |event| {
            if let StreamEvent::Chunk(chunk) = event {
                content.push_str(chunk.as_str());
            }
        })
        .await?;

        Ok(ChatResponse {
            message: Message {
                role: MessageRole::Assistant,
                content,
            },
        })
    }

    pub(crate) async fn stream_async(
        &self,
        request: ChatRequest,
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<()> {
        let started = Instant::now();
        let url = self.chat_url();
        let mut response = self
            .client
            .post(url.as_str())
            .json(&OllamaChatRequest {
                model: request.model.name,
                messages: request
                    .messages
                    .into_iter()
                    .filter_map(ollama_message_from_message)
                    .collect(),
                stream: true,
            })
            .send()
            .await
            .with_context(|| format!("failed to send Ollama chat request to {url}"))?;

        let initial_response_time = started.elapsed();
        let status = response.status();
        on_event(StreamEvent::InitialResponse {
            status: status.as_u16(),
            initial_response_time,
        });

        if !status.is_success() {
            let body = response
                .text()
                .await
                .context("failed to read Ollama error response")?;
            bail!("Ollama chat request failed: HTTP status {status}: {body}");
        }

        let mut buffered = String::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("failed to read Ollama chat response chunk")?
        {
            buffered.push_str(
                std::str::from_utf8(chunk.as_ref())
                    .context("Ollama stream response was not valid UTF-8")?,
            );

            while let Some(newline) = buffered.find('\n') {
                let line = buffered[..newline].trim().to_string();
                buffered.drain(..=newline);
                emit_ollama_stream_line(&line, &mut on_event)?;
            }
        }

        emit_ollama_stream_line(buffered.trim(), &mut on_event)?;
        on_event(StreamEvent::Complete {
            total_time: started.elapsed(),
        });

        Ok(())
    }
}

impl LlamaCppProvider {
    pub(crate) fn from_env() -> Self {
        Self {
            base_url: env_with_fallback(
                "SWITCHYARD_LLAMA_CPP_BASE_URL",
                "LLAMA_CPP_BASE_URL",
                DEFAULT_LLAMA_CPP_BASE_URL,
            )
            .trim_end_matches('/')
            .to_string(),
            client: Client::new(),
        }
    }

    pub(crate) fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    pub(crate) async fn stream_async(
        &self,
        request: ChatRequest,
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<()> {
        let started = Instant::now();
        let url = self.chat_url();
        let mut response = self
            .client
            .post(url.as_str())
            .json(&LlamaCppChatRequest {
                model: request.model.name,
                messages: request
                    .messages
                    .into_iter()
                    .filter_map(llama_cpp_message_from_message)
                    .collect(),
                stream: true,
            })
            .send()
            .await
            .with_context(|| format!("failed to send llama.cpp chat request to {url}"))?;

        let initial_response_time = started.elapsed();
        let status = response.status();
        on_event(StreamEvent::InitialResponse {
            status: status.as_u16(),
            initial_response_time,
        });

        if !status.is_success() {
            let body = response
                .text()
                .await
                .context("failed to read llama.cpp error response")?;
            bail!("llama.cpp chat request failed: HTTP status {status}: {body}");
        }

        let mut buffered = String::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("failed to read llama.cpp chat response chunk")?
        {
            buffered.push_str(
                std::str::from_utf8(chunk.as_ref())
                    .context("llama.cpp stream response was not valid UTF-8")?,
            );

            while let Some(newline) = buffered.find('\n') {
                let line = buffered[..newline].trim().to_string();
                buffered.drain(..=newline);
                emit_llama_cpp_stream_line(&line, &mut on_event)?;
            }
        }

        emit_llama_cpp_stream_line(buffered.trim(), &mut on_event)?;
        on_event(StreamEvent::Complete {
            total_time: started.elapsed(),
        });

        Ok(())
    }
}

impl ProviderClient for OllamaProvider {
    type Error = anyhow::Error;

    fn send(&self, request: ChatRequest) -> Result<ChatResponse, Self::Error> {
        tokio::runtime::Runtime::new()
            .context("failed to create Tokio runtime for Ollama request")?
            .block_on(self.send_async(request))
    }
}

pub(crate) fn ollama_model_from_env() -> Model {
    env_with_fallback(
        "SWITCHYARD_OLLAMA_MODEL",
        "OLLAMA_MODEL",
        DEFAULT_OLLAMA_MODEL,
    )
    .into()
}

fn llama_cpp_model_from_env() -> Model {
    env_with_fallback(
        "SWITCHYARD_LLAMA_CPP_MODEL",
        "LLAMA_CPP_MODEL",
        DEFAULT_LLAMA_CPP_MODEL,
    )
    .into()
}

fn env_with_fallback(project_key: &str, fallback_key: &str, default: &str) -> String {
    env::var(project_key)
        .or_else(|_| env::var(fallback_key))
        .unwrap_or_else(|_| default.to_string())
}

fn ollama_message_from_message(message: Message) -> Option<OllamaMessage> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool | MessageRole::Diagnostic | MessageRole::Reasoning => return None,
    };

    Some(OllamaMessage {
        role: role.to_string(),
        content: message.content,
    })
}

fn llama_cpp_message_from_message(message: Message) -> Option<LlamaCppMessage> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool | MessageRole::Diagnostic | MessageRole::Reasoning => return None,
    };

    Some(LlamaCppMessage {
        role: role.to_string(),
        content: message.content,
    })
}

fn emit_ollama_stream_line(line: &str, on_event: &mut impl FnMut(StreamEvent)) -> Result<()> {
    if line.is_empty() {
        return Ok(());
    }

    let response: OllamaStreamResponse =
        serde_json::from_str(line).context("failed to decode Ollama stream response")?;
    if let Some(message) = response.message {
        if let Some(thinking) = message.thinking
            && !thinking.is_empty()
        {
            on_event(StreamEvent::ReasoningChunk(thinking));
        }

        if !message.content.is_empty() {
            on_event(StreamEvent::Chunk(message.content));
        }
    }

    Ok(())
}

fn emit_llama_cpp_stream_line(line: &str, on_event: &mut impl FnMut(StreamEvent)) -> Result<()> {
    if line.is_empty() {
        return Ok(());
    }

    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(());
    };
    if data == "[DONE]" {
        return Ok(());
    }

    let response: LlamaCppStreamResponse =
        serde_json::from_str(data).context("failed to decode llama.cpp stream response")?;
    for choice in response.choices {
        if let Some(reasoning) = choice.delta.reasoning_content
            && !reasoning.is_empty()
        {
            on_event(StreamEvent::ReasoningChunk(reasoning));
        }

        if let Some(content) = choice.delta.content
            && !content.is_empty()
        {
            on_event(StreamEvent::Chunk(content));
        }
    }

    Ok(())
}
