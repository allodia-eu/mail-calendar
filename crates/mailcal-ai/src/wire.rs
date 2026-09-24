//! The OpenAI-compatible chat-completions shapes this crate sends and reads.
//!
//! Only the subset the two features use: text messages, one forced tool, no streaming. A field a
//! server adds that is not modelled here is ignored on read.
//!
//! **Privacy.** Every type that can hold a prompt or an answer prints lengths, never text, from
//! its `Debug`: a message body and a draft are user content (`docs/logging.md`).

use std::{fmt, time::Duration};

use serde::{Deserialize, Serialize};

/// What a request is for. The relay names it to the gateway, which picks the model for it; an own
/// endpoint ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    /// Learning a writing style from sent mail.
    Style,
    /// Drafting a reply.
    Draft,
}

impl Purpose {
    /// What the log calls a request for this purpose.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Style => "learning request",
            Self::Draft => "draft request",
        }
    }

    /// How long a request for this purpose may take before it is given up on.
    ///
    /// Learning reads up to a context window of mail and writes a long structured answer, which
    /// an EU-hosted model can take minutes over; a draft is a paragraph or two.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        match self {
            Self::Style => Duration::from_secs(300),
            Self::Draft => Duration::from_secs(120),
        }
    }
}

/// One chat-completions request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatRequest {
    /// What the request is for. Not an OpenAI field: a backend decides whether to send it.
    #[serde(skip)]
    pub purpose: Purpose,
    /// The conversation, system prompt first.
    pub messages: Vec<ChatMessage>,
    /// The tools offered. Empty for a plain-text answer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// The tool the model must call, when one must be.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// The answer's length cap, in tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Who a message is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The instructions.
    System,
    /// The input.
    User,
    /// A model's earlier answer.
    Assistant,
}

/// One text message in a conversation.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    /// Who it is from.
    pub role: Role,
    /// The text.
    pub content: String,
}

impl ChatMessage {
    /// A system message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// A user message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
}

impl fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatMessage")
            .field("role", &self.role)
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// A function the model may call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    /// Always `"function"`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The function.
    pub function: FunctionSpec,
}

/// A function's name, purpose and parameters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionSpec {
    /// The name the model calls it by.
    pub name: &'static str,
    /// What it is for.
    pub description: &'static str,
    /// Its arguments, as a JSON Schema.
    pub parameters: serde_json::Value,
}

/// Forces one named function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolChoice {
    /// Always `"function"`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The function.
    pub function: ToolChoiceFunction,
}

/// The function a [`ToolChoice`] names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolChoiceFunction {
    /// Its name.
    pub name: &'static str,
}

/// One chat-completions answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChatResponse {
    /// The candidates; this crate reads the first.
    pub choices: Vec<Choice>,
    /// What the request cost in tokens, when the server says.
    #[serde(default)]
    pub usage: Option<Usage>,
    /// What the relay charged and what is left. Absent from an own endpoint's answer.
    #[serde(default)]
    pub allodia: Option<Metering>,
}

/// One candidate answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Choice {
    /// The answer.
    pub message: AssistantMessage,
    /// Why generation stopped (`stop`, `length`, `tool_calls`), when the server says.
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// The model's answer: text, tool calls, or both.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct AssistantMessage {
    /// The text, if any.
    #[serde(default)]
    pub content: Option<String>,
    /// The calls, if any.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

impl fmt::Debug for AssistantMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssistantMessage")
            .field("content_len", &self.content.as_ref().map(String::len))
            .field("tool_calls", &self.tool_calls)
            .finish()
    }
}

/// One function call.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolCall {
    /// The call.
    pub function: FunctionCall,
}

/// A called function's name and arguments.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct FunctionCall {
    /// The function called.
    pub name: String,
    /// Its arguments: a JSON document, as a string.
    #[serde(default)]
    pub arguments: String,
}

impl fmt::Debug for FunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionCall")
            .field("name", &self.name)
            .field("arguments_len", &self.arguments.len())
            .finish()
    }
}

/// Token counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Usage {
    /// Tokens read.
    #[serde(default)]
    pub prompt_tokens: u64,
    /// Tokens written.
    #[serde(default)]
    pub completion_tokens: u64,
}

/// What a relay request cost, in credits, and the balance after it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct Metering {
    /// Credits this request took.
    pub credits_charged: f64,
    /// Credits left.
    pub balance_credits: f64,
}

impl Metering {
    /// This charge added to `earlier`, carrying the newer balance.
    #[must_use]
    pub fn after(self, earlier: Option<Self>) -> Self {
        Self {
            credits_charged: earlier.map_or(0.0, |earlier| earlier.credits_charged)
                + self.credits_charged,
            balance_credits: self.balance_credits,
        }
    }
}

impl ChatResponse {
    /// The first candidate's answer.
    #[must_use]
    pub fn answer(&self) -> Option<&AssistantMessage> {
        self.choices.first().map(|choice| &choice.message)
    }
}
