//! Image generation: Gemini ("Nano Banana") or OpenAI.

use base64::Engine;
use serde_json::{json, Value};

use crate::models::{post_json, ImageModel};

pub struct Gemini {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

pub struct OpenaiImages {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ImageModel for Gemini {
    /// Uses Google's Interactions API, falling back to the older
    /// generateContent API, which some image models still require.
    fn generate(&self, prompt: &str) -> Result<Vec<u8>, String> {
        let headers = [("x-goog-api-key", self.api_key.as_str())];
        let interactions = json!({
            "model": self.model,
            "input": [{ "type": "text", "text": prompt }],
            "response_format": { "type": "image" },
        });
        let first_error =
            match post_json(&format!("{}/v1beta/interactions", self.base_url), &headers, &interactions) {
                Ok(response) => match find_image_data(&response) {
                    Some(data) => return decode(&data),
                    None => "the response had no image in it".to_string(),
                },
                Err(error) => error,
            };

        let generate_content = json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": { "responseModalities": ["IMAGE"] },
        });
        let url = format!("{}/v1beta/models/{}:generateContent", self.base_url, self.model);
        let response = post_json(&url, &headers, &generate_content)
            .map_err(|second_error| format!("{first_error}\nAlso tried generateContent: {second_error}"))?;
        let data = find_image_data(&response).ok_or("Gemini didn't return an image")?;
        decode(&data)
    }
}

impl ImageModel for OpenaiImages {
    fn generate(&self, prompt: &str) -> Result<Vec<u8>, String> {
        let authorization = format!("Bearer {}", self.api_key);
        let body = json!({ "model": self.model, "prompt": prompt, "size": "1024x1024" });
        let url = format!("{}/images/generations", self.base_url);
        let response = post_json(&url, &[("authorization", authorization.as_str())], &body)?;
        let data = response["data"][0]["b64_json"].as_str().ok_or("OpenAI didn't return an image")?;
        decode(data)
    }
}

/// Finds base64 image data anywhere in a response. Google has shipped a few
/// shapes for this (`{"type": "image", "data"}` and `inlineData`), so look
/// for all of them rather than depend on one exact path.
fn find_image_data(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("image") {
                if let Some(data) = object.get("data").and_then(Value::as_str) {
                    return Some(data.to_string());
                }
            }
            for key in ["inlineData", "inline_data"] {
                if let Some(data) = object.get(key).and_then(|inline| inline["data"].as_str()) {
                    return Some(data.to_string());
                }
            }
            object.values().find_map(find_image_data)
        }
        Value::Array(items) => items.iter().find_map(find_image_data),
        _ => None,
    }
}

fn decode(data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| format!("Bad image data: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_images_in_both_gemini_response_shapes() {
        let interactions = json!({ "steps": [{ "type": "model_output",
            "content": [{ "type": "text", "text": "here" }, { "type": "image", "data": "aGk=" }] }] });
        let generate_content = json!({ "candidates": [{ "content": { "parts": [
            { "text": "here" }, { "inlineData": { "mimeType": "image/png", "data": "aGk=" } }] } }] });
        assert_eq!(find_image_data(&interactions).as_deref(), Some("aGk="));
        assert_eq!(find_image_data(&generate_content).as_deref(), Some("aGk="));
        assert_eq!(decode("aGk=").unwrap(), b"hi");
    }
}
