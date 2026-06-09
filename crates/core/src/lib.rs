//! Shared domain types for Switchyard clients and provider integrations.

/// Package metadata for the core crate.
pub mod meta {
    /// Cargo package name for this crate.
    pub static NAME: &str = env!("CARGO_PKG_NAME");
    /// Cargo package version for this crate.
    pub static VERSION: &str = env!("CARGO_PKG_VERSION");
}

/// A model identifier understood by a provider.
#[derive(Clone)]
pub struct Model {
    /// Provider-specific model name.
    pub name: String,
}

impl From<&str> for Model {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
        }
    }
}

impl From<String> for Model {
    fn from(value: String) -> Self {
        Self { name: value }
    }
}

/// A provider identifier.
pub struct Provider {
    /// Human-readable provider name.
    pub name: String,
}

impl From<&str> for Provider {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
        }
    }
}

/// Role associated with a chat message.
#[derive(Clone)]
pub enum MessageRole {
    /// System instructions that shape the assistant's behavior.
    System,
    /// Message authored by the user.
    User,
    /// Message authored by the assistant.
    Assistant,
    /// Message produced by an external tool.
    Tool,
    /// Diagnostic output intended for debugging or status reporting.
    Diagnostic,
    /// Reasoning content emitted separately from the assistant response.
    Reasoning,
}

/// A single chat transcript entry.
#[derive(Clone)]
pub struct Message {
    /// The author or semantic role of the message.
    pub role: MessageRole,
    /// Message text.
    pub content: String,
}

/// In-memory chat session state.
#[derive(Default)]
pub struct Session {
    /// Messages accumulated in the current session.
    pub messages: Vec<Message>,
}

/// Request sent to a chat provider.
pub struct ChatRequest {
    /// Model to use for the request.
    pub model: Model,
    /// Conversation context sent to the provider.
    pub messages: Vec<Message>,
}

/// Response returned by a chat provider.
pub struct ChatResponse {
    /// Provider's response message.
    pub message: Message,
}

/// Synchronous client interface for chat providers.
pub trait ProviderClient {
    /// Error type returned by the provider implementation.
    type Error;

    /// Sends a chat request and returns the provider response.
    fn send(&self, request: ChatRequest) -> Result<ChatResponse, Self::Error>;
}
