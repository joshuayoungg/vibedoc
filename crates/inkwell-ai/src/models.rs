//! A small, provider-neutral interface to AI models. Each provider file
//! translates between these types and that service's JSON.

use std::time::Duration;

use serde_json::Value;
use ureq::tls::{RootCerts, TlsConfig};

use crate::config::{AgentConfig, AgentProvider, ImageConfig, ImageProvider};
use crate::{anthropic, images, openai};

pub enum Message {
    User(String),
    Assistant { text: String, tool_calls: Vec<ToolCall> },
    ToolResults(Vec<ToolResult>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

pub struct ToolResult {
    pub call_id: String,
    pub content: String,
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema for the tool's input.
    pub input_schema: Value,
}

#[derive(Debug, Default)]
pub struct Reply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

/// A model that can chat and call tools.
pub trait ChatModel: Send {
    fn reply(&self, system: &str, messages: &[Message], tools: &[ToolSpec]) -> Result<Reply, String>;
}

/// A model that turns a prompt into an encoded image (PNG or JPEG bytes).
pub trait ImageModel: Send {
    fn generate(&self, prompt: &str) -> Result<Vec<u8>, String>;
}

pub fn chat_model(config: &AgentConfig) -> Result<Box<dyn ChatModel>, String> {
    let api_key = config.api_key()?;
    let base_url = config.base_url.trim_end_matches('/').to_string();
    let model = config.model.clone();
    Ok(match config.provider {
        AgentProvider::Anthropic => Box::new(anthropic::Anthropic { api_key, base_url, model }),
        AgentProvider::OpenaiCompatible => Box::new(openai::OpenaiCompatible { api_key, base_url, model }),
    })
}

pub fn image_model(config: &ImageConfig) -> Result<Box<dyn ImageModel>, String> {
    let api_key = config.api_key()?;
    let base_url = config.base_url.trim_end_matches('/').to_string();
    let model = config.model.clone();
    Ok(match config.provider {
        ImageProvider::Gemini => Box::new(images::Gemini { api_key, base_url, model }),
        ImageProvider::Openai => Box::new(images::OpenaiImages { api_key, base_url, model }),
    })
}

/// POSTs JSON and returns the JSON response. Error responses become readable
/// messages that include what the service said, so setup problems are clear.
pub(crate) fn post_json(url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(180)))
        // Trust the computer's own certificates, so corporate proxies work too.
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .build()
        .into();
    let mut request = agent.post(url).header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request.send_json(body).map_err(|e| format!("Couldn't reach {url}: {e}"))?;
    let status = response.status();
    let text = response
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Couldn't read the response from {url}: {e}"))?;
    if !status.is_success() {
        let detail: String = text.chars().take(400).collect();
        return Err(format!("{url} returned {status}: {detail}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("{url} sent something that isn't JSON: {e}"))
}
