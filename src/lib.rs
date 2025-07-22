mod helpers;
mod openai_payload;
mod world;

use openai_payload::{Message, OpenAIPayload, OpenAIResponse};
use std::collections::HashMap;

use world::bindings::exports::wasi::http::incoming_handler::Guest;
use world::bindings::wasi::http::types::IncomingRequest;
use world::bindings::wasi::http::types::ResponseOutparam;
use world::bindings::Component;

impl Guest for Component {
    fn handle(req: IncomingRequest, resp: ResponseOutparam) {
        let settings = match Settings::from_req(&req) {
            Ok(settings) => settings,
            Err(e) => {
                let response = helpers::build_response_json_error(
                    &format!("Failed to parse component settings: {e}"),
                    500,
                );
                response.send(resp);
                return;
            }
        };

        // read request body
        let request_body = match helpers::parse_body(req) {
            Ok(body) => body,
            Err(e) => {
                let response = helpers::build_response_json_error(&e, 400);
                response.send(resp);
                return;
            }
        };

        // parse body to JSON
        let body_json: serde_json::Value = match serde_json::from_slice(&request_body) {
            Ok(json) => json,
            Err(_) => {
                let response =
                    helpers::build_response_json_error("Invalid JSON in request body", 400);
                response.send(resp);
                return;
            }
        };

        // extract messages from request body
        let mut messages: Vec<Message> = match body_json.get("messages") {
            Some(value) => value
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|v| {
                    let content = v
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let role = v
                        .get("role")
                        .and_then(|r| r.as_str())
                        .unwrap_or(settings.default_role.as_str())
                        .to_string();
                    Message { role, content }
                })
                .collect::<Vec<Message>>(),
            None => {
                let response = helpers::build_response_json_error(
                    "Missing 'messages' field in request body",
                    400,
                );
                response.send(resp);
                return;
            }
        };

        // use system prompt if provided (or default)
        let default_system_prompt = settings
            .default_system_prompt
            .clone()
            .unwrap_or_else(|| "You are a helpful assistant.".into());

        // always prepend a system message
        messages.insert(
            0,
            Message {
                role: "system".to_string(),
                content: default_system_prompt,
            },
        );

        let openai_payload =
            OpenAIPayload::new(settings.model, messages, settings.max_completion_tokens);

        let openai_response = openai_payload.send(settings.api_hostname, settings.api_key);

        // handle error in case request couldn't be sent
        if let Err(e) = openai_response {
            let response = helpers::build_response_json_error(&e.to_string(), 500);
            response.send(resp);
            return;
        }

        let openai_response = openai_response.unwrap();
        let response_status = openai_response.status_code();

        let response_body =
            String::from_utf8_lossy(&openai_response.body().unwrap_or_default()).to_string();

        let function_response = match OpenAIResponse::from_json_string(response_body) {
            Ok(response) => response,
            Err(e) => {
                let response = helpers::build_response_json_error(
                    &format!("Could not parse OpenAI response: {e}"),
                    500,
                );
                response.send(resp);
                return;
            }
        };

        let response = helpers::build_response_json(
            &function_response.first_choice_to_json(),
            response_status,
        );
        response.send(resp);
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Settings {
    pub api_key: String,
    pub model: String,
    pub max_completion_tokens: Option<u32>,
    pub default_role: String,
    pub default_system_prompt: Option<String>,
    pub api_hostname: Option<String>,
}

impl Settings {
    pub fn from_req(req: &IncomingRequest) -> anyhow::Result<Self> {
        let map = helpers::parse_headers(&IncomingRequest::headers(req));
        Self::new(&map)
    }

    pub fn new(headers: &HashMap<String, Vec<String>>) -> anyhow::Result<Self> {
        let settings = headers
            .get("x-edgee-component-settings")
            .ok_or_else(|| anyhow::anyhow!("Missing 'x-edgee-component-settings' header"))?;

        if settings.len() != 1 {
            return Err(anyhow::anyhow!(
                "Expected exactly one 'x-edgee-component-settings' header, found {}",
                settings.len()
            ));
        }
        let setting = settings[0].clone();
        let setting: HashMap<String, String> = serde_json::from_str(&setting)?;

        let api_key = setting
            .get("api_key")
            .map(String::to_string)
            .unwrap_or_default();

        let model = setting
            .get("model")
            .map(String::to_string)
            .unwrap_or_default();

        let max_completion_tokens: Option<u32> = setting
            .get("max_completion_tokens")
            .and_then(|v| v.parse().ok());

        let default_role = setting
            .get("default_role")
            .map(String::to_string)
            .unwrap_or("user".to_string());

        let default_system_prompt: Option<String> = setting.get("default_system_prompt").cloned();

        let api_hostname: Option<String> = setting
            .get("api_hostname")
            .cloned()
            .filter(|s| !s.is_empty());

        Ok(Self {
            api_key,
            model,
            max_completion_tokens,
            default_role,
            default_system_prompt,
            api_hostname,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_new() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-edgee-component-settings".to_string(),
            vec![r#"{"api_key": "test_value"}"#.to_string()],
        );

        let settings = Settings::new(&headers).unwrap();
        assert_eq!(settings.api_key, "test_value");
    }

    #[test]
    fn test_settings_new_missing_header() {
        let headers = HashMap::new();
        let result = Settings::new(&headers);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing 'x-edgee-component-settings' header"
        );
    }

    #[test]
    fn test_settings_new_multiple_headers() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-edgee-component-settings".to_string(),
            vec![
                r#"{"api_key": "test_value"}"#.to_string(),
                r#"{"api_key": "another_value"}"#.to_string(),
            ],
        );
        let result = Settings::new(&headers);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected exactly one 'x-edgee-component-settings' header"));
    }

    #[test]
    fn test_settings_new_invalid_json() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-edgee-component-settings".to_string(),
            vec!["not a json".to_string()],
        );
        let result = Settings::new(&headers);
        assert!(result.is_err());
    }
}
