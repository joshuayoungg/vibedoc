//! Any service that speaks OpenAI's Chat Completions API: OpenAI, Gemini,
//! DeepSeek, OpenRouter, Groq, Ollama, LM Studio, and many more.

use serde_json::{json, Value};

use crate::models::{post_json, ChatModel, Message, Reply, ToolCall, ToolSpec};

pub struct OpenaiCompatible {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ChatModel for OpenaiCompatible {
    fn reply(&self, system: &str, messages: &[Message], tools: &[ToolSpec]) -> Result<Reply, String> {
        let mut json_messages = vec![json!({ "role": "system", "content": system })];
        for message in messages {
            json_messages.extend(to_json(message));
        }
        let body = json!({
            "model": self.model,
            "messages": json_messages,
            "tools": tools.iter().map(|t| json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.input_schema },
            })).collect::<Vec<_>>(),
        });
        let authorization = format!("Bearer {}", self.api_key);
        let headers = [("authorization", authorization.as_str())];
        let response = post_json(&format!("{}/chat/completions", self.base_url), &headers, &body)?;
        parse_reply(&response)
    }
}

/// One of our messages can become several of theirs (one per tool result).
fn to_json(message: &Message) -> Vec<Value> {
    match message {
        Message::User(text) => vec![json!({ "role": "user", "content": text })],
        Message::Assistant { text, tool_calls } => {
            let mut assistant = json!({ "role": "assistant", "content": text });
            if !tool_calls.is_empty() {
                assistant["tool_calls"] = tool_calls
                    .iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "type": "function",
                            "function": { "name": call.name, "arguments": call.input.to_string() },
                        })
                    })
                    .collect();
            }
            vec![assistant]
        }
        Message::ToolResults(results) => results
            .iter()
            .map(|r| json!({ "role": "tool", "tool_call_id": r.call_id, "content": r.content }))
            .collect(),
    }
}

fn parse_reply(response: &Value) -> Result<Reply, String> {
    let message = &response["choices"][0]["message"];
    if message.is_null() {
        return Err(format!("Unexpected response: {response}"));
    }
    let mut reply =
        Reply { text: message["content"].as_str().unwrap_or_default().to_string(), ..Default::default() };
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
        reply.tool_calls.push(ToolCall {
            id: call["id"].as_str().unwrap_or_default().to_string(),
            name: call["function"]["name"].as_str().unwrap_or_default().to_string(),
            input: serde_json::from_str(arguments).unwrap_or(Value::Null),
        });
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_calls_with_string_arguments() {
        let response = json!({ "choices": [{ "message": {
            "content": null,
            "tool_calls": [{ "id": "c1", "type": "function",
                "function": { "name": "delete_paragraphs", "arguments": "{\"first_paragraph\":2}" } }],
        }}]});
        let reply = parse_reply(&response).unwrap();
        assert_eq!(reply.tool_calls[0].input["first_paragraph"], 2);
    }

    #[test]
    fn each_tool_result_becomes_its_own_message() {
        let results = Message::ToolResults(vec![
            crate::models::ToolResult { call_id: "a".into(), content: "ok".into() },
            crate::models::ToolResult { call_id: "b".into(), content: "ok".into() },
        ]);
        assert_eq!(to_json(&results).len(), 2);
    }
}
