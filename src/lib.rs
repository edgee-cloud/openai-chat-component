mod openai_payload;

use std::collections::HashMap;

use bindings::wasi::http::types::{IncomingRequest, ResponseOutparam};
use openai_payload::{OpenAIPayload, OpenAIResponse, Message};

mod bindings {
    wit_bindgen::generate!({
        path: ".edgee/wit",
        world: "edge-function",
        generate_all,
        pub_export_macro: true,
        default_bindings_module: "$crate::bindings",
    });
}
mod helpers;

struct Component;
bindings::export!(Component);

impl bindings::exports::wasi::http::incoming_handler::Guest for Component {
    fn handle(req: IncomingRequest, resp: ResponseOutparam) {
        helpers::run_json(req, resp, Self::handle_json_request);
    }
}

impl Component {
    fn handle_json_request(
        req: http::Request<serde_json::Value>,
    ) -> Result<http::Response<serde_json::Value>, anyhow::Error> {
        let settings = Settings::from_req(&req)?;

        let request_body = req.body();

        // extract messages from request body
        let mut messages: Vec<Message> = match request_body.get("messages") {
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
            None => return Err(anyhow::anyhow!("Missing 'messages' field in request body")),
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

        let openai_response = openai_payload.send(settings.api_hostname, settings.api_key).expect("Failed to send OpenAI request");

        let response_status = openai_response.status_code();
        let response_body =
            String::from_utf8_lossy(&openai_response.body()?).to_string();

        let component_response = match OpenAIResponse::from_json_string(response_body) {
            Ok(response) => response,
            Err(e) => return Err(anyhow::anyhow!("Could not parse OpenAI response: {e}")),
        };

        Ok(http::Response::builder()
            .status(response_status)
            .body(component_response.first_choice_to_json())?)

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
    pub fn new(headers: &http::header::HeaderMap) -> anyhow::Result<Self> {
        let value = headers
            .get("x-edgee-component-settings")
            .ok_or_else(|| anyhow::anyhow!("Missing 'x-edgee-component-settings' header"))
            .and_then(|value| value.to_str().map_err(Into::into))?;
        let data: HashMap<String, String> = serde_json::from_str(value)?;

        Ok(Self {
            api_key: data
                .get("api_key")
                .ok_or_else(|| anyhow::anyhow!("Missing api_key setting"))?
                .to_string(),
            model: data
                .get("model")
                .ok_or_else(|| anyhow::anyhow!("Missing model setting"))?
                .to_string(),
            max_completion_tokens: data
                .get("max_completion_tokens")
                .and_then(|v| v.parse().ok()),
            default_role: data
                .get("default_role")
                .map(String::to_string)
                .unwrap_or("user".to_string()),
            default_system_prompt: data
                .get("default_system_prompt")
                .cloned()
                .filter(|s| !s.is_empty()),
            api_hostname: data
                .get("api_hostname")
                .cloned()
                .filter(|s| !s.is_empty()),
        })
    }

    pub fn from_req<B>(req: &http::Request<B>) -> anyhow::Result<Self> {
        Self::new(req.headers())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static;
    use serde_json::json;
    use std::sync::Mutex;
    use http::{HeaderValue, Request};


    // Patch SlackMessagePayload::send for this test
    lazy_static::lazy_static! {
        static ref SEND_CALLED: Mutex<bool> = Mutex::new(false);
    }

    // Mock SlackMessagePayload::send to avoid real HTTP call
    pub struct MockResponse;
    impl MockResponse {
        pub fn status_code(&self) -> u16 {
            200
        }
        pub fn body(&self) -> anyhow::Result<Vec<u8>> {
            Ok(r#"{"choices": [{"message": {"role": "system", "content": "ok"}}]}"#.into())
        }
    }

    impl OpenAIPayload {
        pub fn send(&self, _hostname: Option<String>, _apikey: String) -> anyhow::Result<MockResponse> {
            *SEND_CALLED.lock().unwrap() = true;
            Ok(MockResponse)
        }
    }

    #[test]
    fn test_settings_new() {
        let mut headers = http::header::HeaderMap::new();
        headers.insert(
            "x-edgee-component-settings",
            HeaderValue::from_static(r#"{"api_key": "sk-XYZ", "model": "gpt-3.5-turbo"}"#),
        );

        let settings = Settings::new(&headers).unwrap();
        assert_eq!(settings.api_key, "sk-XYZ");
        assert_eq!(settings.model, "gpt-3.5-turbo");
    }

    #[test]
    fn test_settings_new_missing_header() {
        let headers = http::header::HeaderMap::new();
        let result = Settings::new(&headers);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing 'x-edgee-component-settings' header"
        );
    }

    #[test]
    fn test_settings_new_invalid_json() {
        let mut headers = http::header::HeaderMap::new();
        headers.insert(
            "x-edgee-component-settings",
            HeaderValue::from_static(r#"not a json"#),
        );
        let result = Settings::new(&headers);
        assert!(result.is_err());
    }


    #[test]
    fn test_handle_json_request_success() {
        // Prepare request with headers and body
        let body = json!({ "messages": [{
                "role": "user",
                "content": "Hello! Please say \"ok\" if this API call is working."
            }]});
        let req = Request::builder()
            .header(
                "x-edgee-component-settings",
                r#"{"api_key": "sk-XYZ", "model": "gpt-3.5-turbo"}"#,
            )
            .body(body)
            .unwrap();

        // Call the handler
        let result = Component::handle_json_request(req);

        // Assert
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.body().to_string(), r#"{"content":"ok","role":"system"}"#);
        assert!(*SEND_CALLED.lock().unwrap());
    }

    #[test]
    fn test_handle_json_request_missing_messages() {
        let body = json!({}); // empty
        let req = Request::builder()
            .header(
                "x-edgee-component-settings",
                r#"{"api_key": "sk-XYZ", "model": "gpt-3.5-turbo"}"#,
            )
            .body(body)
            .unwrap();

        let result = Component::handle_json_request(req);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing 'messages' field in request body"
        );
    }
    
}
