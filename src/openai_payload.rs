#[cfg(not(test))]
use waki::Response;

const DEFAULT_HOST: &str = "api.openai.com";
const ENDPOINT: &str = "/v1/chat/completions";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct Message {
    pub(crate) role: String,
    pub(crate) content: String,
}

impl Message {
    pub(crate) fn default_error_message() -> Self {
        Message { role: "system".into(), content: "An error occurred".into() }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct OpenAIPayload {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
}

impl OpenAIPayload {
    pub(crate) fn new(
        model: String,
        messages: Vec<Message>,
        max_completion_tokens: Option<u32>,
    ) -> Self {
        OpenAIPayload {
            model,
            messages,
            max_completion_tokens,
        }
    }

    pub(crate) fn generate_endpoint(&self, hostname: Option<String>) -> String {
        // use provided hostname or default to DEFAULT_HOST
        let hostname = hostname.unwrap_or(DEFAULT_HOST.to_string());
        // append the endpoint path
        let mut endpoint = format!("{hostname}{ENDPOINT}");
        // ensure the endpoint starts with "https://"
        if !endpoint.starts_with("https://") {
            endpoint = format!("https://{endpoint}");
        }
        endpoint
    }

    #[cfg(not(test))]
    pub(crate) fn send(
        &self,
        hostname: Option<String>,
        api_key: String,
    ) -> Result<Response, anyhow::Error> {
        let client = waki::Client::new();
        let response = client
            .post(&self.generate_endpoint(hostname))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {api_key}"))
            .body(serde_json::to_vec(self)?)
            .send()?;
        Ok(response)
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct OpenAIChoice {
    pub(crate) message: Message,
}

#[derive(serde::Deserialize)]
pub(crate) struct OpenAIResponse {
    pub(crate) choices: Vec<OpenAIChoice>,
}

/*
 * This struct is used to deserialize the OpenAI response
 * and only return back the first choice's message (ignoring all other response fields for now).
 */
impl OpenAIResponse {
    pub(crate) fn from_json_string(response_body: String) -> Result<Self, anyhow::Error> {
        // deserialize the body into OpenAIResponse struct
        let openai_response: OpenAIResponse = serde_json::from_str(&response_body)?;
        Ok(openai_response)
    }

    pub(crate) fn first_choice_to_json(&self) -> serde_json::Value {
        // convert the first choice's message content to a string
        if let Some(choice) = self.choices.first() {
            serde_json::json!(&choice.message)
        } else {
            // fallback message (this should not happen, but just in case)
            serde_json::json!(Message::default_error_message())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_struct() {
        let msg = Message {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello!");
    }

    #[test]
    fn test_openai_payload_new() {
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "Hi".to_string(),
            },
            Message {
                role: "assistant".to_string(),
                content: "Hello!".to_string(),
            },
        ];
        let payload = OpenAIPayload::new("gpt-3.5-turbo".to_string(), messages.clone(), Some(42));
        assert_eq!(payload.model, "gpt-3.5-turbo");
        assert_eq!(payload.messages.len(), 2);
        assert_eq!(payload.max_completion_tokens.unwrap(), 42);
    }

    #[test]
    fn test_openai_payload_serialization() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }];
        let payload = OpenAIPayload::new("gpt-4".to_string(), messages, None);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Test\""));
        assert!(!json.contains("max_completion_tokens"));
    }

    #[test]
    fn test_openai_response_from_json_string_success() {
        let json = r#"{
            "choices": [
                { "message": { "role": "assistant", "content": "Hi there!" } }
            ]
        }"#
        .to_string();
        let resp = OpenAIResponse::from_json_string(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.role, "assistant");
        assert_eq!(resp.choices[0].message.content, "Hi there!");
    }

    #[test]
    fn test_openai_response_from_json_string_invalid_json() {
        let invalid_json = r#"{"choices": [ { "message": { "role": "assistant" } } ]}"#.to_string();
        let result = OpenAIResponse::from_json_string(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_openai_response_from_json_string_empty_choices() {
        let json = r#"{"choices": []}"#.to_string();
        let resp = OpenAIResponse::from_json_string(json).unwrap();
        assert_eq!(resp.choices.len(), 0);
    }

    #[test]
    fn test_openai_response_to_response_with_choice() {
        let response = OpenAIResponse {
            choices: vec![OpenAIChoice {
                message: Message {
                    role: "assistant".to_string(),
                    content: "Hello from OpenAI!".to_string(),
                },
            }],
        };
        let result = response.first_choice_to_json();
        // Should be a JSON string containing the message
        assert_eq!(result.get("role").unwrap().as_str(), Some("assistant"));
        assert_eq!(result.get("content").unwrap().as_str(), Some("Hello from OpenAI!"));
    }

    #[test]
    fn test_openai_response_to_response_no_choices() {
        let response = OpenAIResponse { choices: vec![] };
        let result = response.first_choice_to_json();
        assert_eq!(serde_json::to_string(&result).unwrap(), r#"{"content":"An error occurred","role":"system"}"#);
    }

    #[test]
    fn test_generate_endpoint_with_default_hostname() {
        let payload = OpenAIPayload::new("gpt-3.5-turbo".to_string(), vec![], None);
        let endpoint = payload.generate_endpoint(None);
        assert_eq!(endpoint, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_generate_endpoint_with_custom_hostname() {
        let payload = OpenAIPayload::new("gpt-3.5-turbo".to_string(), vec![], None);
        let endpoint = payload.generate_endpoint(Some("custom.example.com".to_string()));
        assert_eq!(endpoint, "https://custom.example.com/v1/chat/completions");
    }

    #[test]
    fn test_generate_endpoint_with_https_in_hostname() {
        let payload = OpenAIPayload::new("gpt-3.5-turbo".to_string(), vec![], None);
        let endpoint = payload.generate_endpoint(Some("https://another.example.com".to_string()));
        assert_eq!(endpoint, "https://another.example.com/v1/chat/completions");
    }
}
