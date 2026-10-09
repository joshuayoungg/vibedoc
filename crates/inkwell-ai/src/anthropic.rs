//! Claude, through Anthropic's Messages API.

use serde_json::{json, Value};

use crate::models::{post_json, ChatModel, Message, Reply, ToolCall, ToolSpec};

pub struct Anthropic {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ChatModel for Anthropic {
    fn reply(&self, system: &str, messages: &[Message], tools: &[ToolSpec]) -> Result<Reply, String> {
        let body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "system": system,
            "tools": tools.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })).collect::<Vec<_>>(),
            "messages": messages.iter().map(to_json).collect::<Vec<_>>(),
        });
        let headers = [("x-api-key", self.api_key.as_str()), ("anthropic-version", "2023-06-01")];
        let response = post_json(&format!("{}/v1/messages", self.base_url), &headers, &body)?;
        Ok(parse_reply(&response))
    }
}

fn to_json(message: &Message) -> Value {
    match message {
        Message::User(text) => json!({ "role": "user", "content": text }),
        Message::Assistant { text, tool_calls } => {
            let mut content = Vec::new();
            if !text.is_empty() {
                content.push(json!({ "type": "text", "text": text }));
            }
            for call in tool_calls {
                content.push(
                    json!({ "type": "tool_use", "id": call.id, "name": call.name, "input": call.input }),
                );
            }
            json!({ "role": "assistant", "content": content })
        }
        Message::ToolResults(results) => json!({
            "role": "user",
            "content": results.iter().map(|r| json!({
                "type": "tool_result",
                "tool_use_id": r.call_id,
                "content": r.content,
            })).collect::<Vec<_>>(),
        }),
    }
}

fn parse_reply(response: &Value) -> Reply {
    let mut reply = Reply::default();
    for block in response["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => reply.text.push_str(block["text"].as_str().unwrap_or_default()),
            Some("tool_use") => reply.tool_calls.push(ToolCall {
                id: block["id"].as_str().unwrap_or_default().to_string(),
                name: block["name"].as_str().unwrap_or_default().to_string(),
                input: block["input"].clone(),
            }),
            _ => {}
        }
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_and_tool_calls() {
        let response = json!({ "content": [
            { "type": "text", "text": "Done." },
            { "type": "tool_use", "id": "t1", "name": "read_document", "input": {} },
        ]});
        let reply = parse_reply(&response);
        assert_eq!(reply.text, "Done.");
        assert_eq!(reply.tool_calls[0].name, "read_document");
    }
}
