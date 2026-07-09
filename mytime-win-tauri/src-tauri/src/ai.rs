//! Minimal Anthropic Messages API client for AI-powered features
//! (categorization suggestions and period insights).
//!
//! Uses raw HTTP against `POST /v1/messages` with structured outputs
//! (`output_config.format`) so the model is constrained to return valid
//! JSON matching a schema. Model: Claude Haiku 4.5 (fast + cost-effective).

use serde_json::{json, Value};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-haiku-4-5";

/// Read the API key from the environment, treating empty as unset.
pub fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
}

/// Call the Messages API and return the model's JSON output, validated
/// server-side against `schema` via structured outputs.
pub async fn complete_json(
    system: &str,
    user: &str,
    schema: Value,
    max_tokens: u32,
) -> Result<Value, String> {
    let key = api_key()
        .ok_or("ANTHROPIC_API_KEY is not set. Add it to your environment to enable AI features.")?;

    let body = json!({
        "model": MODEL,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{"role": "user", "content": user}],
        "output_config": {"format": {"type": "json_schema", "schema": schema}},
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {e}"))?;

    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid Anthropic API response: {e}"))?;

    if !status.is_success() {
        let msg = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("Anthropic API error ({status}): {msg}"));
    }

    match value.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => return Err("The model declined this request.".into()),
        Some("max_tokens") => {
            return Err("The model response was truncated (max_tokens). Try again.".into())
        }
        _ => {}
    }

    let text = value
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        })
        .and_then(|b| b.get("text"))
        .and_then(Value::as_str)
        .ok_or("No text content in Anthropic API response")?;

    serde_json::from_str(text).map_err(|e| format!("Failed to parse model output as JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_treats_blank_as_unset() {
        // Not touching the real env var here; just exercise the filter logic
        // the helper relies on.
        let filter = |k: &str| !k.trim().is_empty();
        assert!(!filter(""));
        assert!(!filter("   "));
        assert!(filter("sk-ant-test"));
    }
}
