pub mod meta {
    pub static NAME: &str = env!("CARGO_PKG_NAME");
    pub static VERSION: &str = env!("CARGO_PKG_VERSION");
}

#[derive(Clone)]
pub struct Model {
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

pub struct Provider {
    pub name: String,
}

impl From<&str> for Provider {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
        }
    }
}

#[derive(Clone)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
    Diagnostic,
    Reasoning,
}

#[derive(Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Default)]
pub struct Session {
    pub messages: Vec<Message>,
}

pub struct ChatRequest {
    pub model: Model,
    pub messages: Vec<Message>,
}

pub struct ChatResponse {
    pub message: Message,
}

pub trait ProviderClient {
    type Error;

    fn send(&self, request: ChatRequest) -> Result<ChatResponse, Self::Error>;
}
