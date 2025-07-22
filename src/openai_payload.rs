use waki::Response;

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct Message {
    pub(crate) role: String,
    pub(crate) content: String,
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

    pub(crate) fn send(&self, api_key: String) -> Result<Response, anyhow::Error> {
        let client = waki::Client::new();
        let response = client
            .post(ENDPOINT)
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

impl OpenAIResponse {
    pub(crate) fn from_json_string(response_body: String) -> Result<Self, anyhow::Error> {
        // deserialize the body into OpenAIResponse struct
        let openai_response: OpenAIResponse = serde_json::from_str(&response_body)?;
        Ok(openai_response)
    }

    pub(crate) fn first_choice_to_json(&self) -> String {
        // convert the first choice's message content to a string
        if let Some(choice) = self.choices.first() {
            serde_json::to_string(&choice.message).unwrap_or_else(|_| "".to_string())
        } else {
            "No response from OpenAI".to_string() // fallback message (this should not happen, but just in case)
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
        }"#.to_string();
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
            choices: vec![
                OpenAIChoice {
                    message: Message {
                        role: "assistant".to_string(),
                        content: "Hello from OpenAI!".to_string(),
                    },
                },
            ],
        };
        let result = response.first_choice_to_json();
        // Should be a JSON string containing the message
        assert!(result.contains("\"role\":\"assistant\""));
        assert!(result.contains("\"content\":\"Hello from OpenAI!\""));
    }

    #[test]
    fn test_openai_response_to_response_no_choices() {
        let response = OpenAIResponse { choices: vec![] };
        let result = response.first_choice_to_json();
        assert_eq!(result, "No response from OpenAI");
    }

}
