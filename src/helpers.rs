use crate::world::bindings::wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse,
};

use crate::world::bindings::wasi::io::streams::StreamError;

use crate::world::bindings::exports::wasi::http::incoming_handler::ResponseOutparam;
use std::collections::HashMap;

pub struct ResponseBuilder {
    headers: Fields,
    status_code: u16,
    body_content: Option<String>,
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        ResponseBuilder::new()
    }
}

impl ResponseBuilder {
    pub fn new() -> Self {
        ResponseBuilder {
            headers: Fields::new(),
            status_code: 200,
            body_content: None,
        }
    }

    pub fn set_header(&mut self, key: &str, value: &str) -> &mut Self {
        let _ = self
            .headers
            .set(key, vec![value.as_bytes().to_vec()].as_slice());
        self
    }

    pub fn set_status_code(&mut self, status_code: u16) -> &mut Self {
        self.status_code = status_code;
        self
    }

    pub fn set_body(&mut self, body: &str) -> &mut Self {
        self.body_content = Some(body.to_string());
        self
    }

    pub fn send(self, resp: ResponseOutparam) {
        let resp_tx = OutgoingResponse::new(self.headers);
        let _ = resp_tx.set_status_code(self.status_code);

        let body = resp_tx.body().unwrap();
        ResponseOutparam::set(resp, Ok(resp_tx));
        let stream = body.write().unwrap();
        if let Some(body_content) = self.body_content {
            let _ = stream.write(body_content.as_bytes());
        }
        drop(stream);
        let _ = OutgoingBody::finish(body, None);
    }
}

pub fn parse_headers(headers: &Fields) -> HashMap<String, Vec<String>> {
    let mut output: HashMap<String, Vec<String>> = HashMap::new();
    for (header_name, header_value) in headers.entries() {
        let header_name = header_name.to_string();
        let header_value = String::from_utf8_lossy(&header_value).to_string();
        output
            .entry(header_name.clone())
            .or_default()
            .push(header_value);
    }

    output
}

pub fn parse_body(req: IncomingRequest) -> Result<Vec<u8>, String> {
    let mut request_body = Vec::new();
    let stream = match req.consume() {
        Ok(stream) => stream,
        Err(_e) => {
            return Err("Failed to consume request stream".to_string());
        }
    };
    let stream = match stream.stream() {
        Ok(stream) => stream,
        Err(_e) => {
            return Err("Failed to get request stream: ".to_string());
        }
    };

    loop {
        match stream.read(4096) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                request_body.extend_from_slice(&chunk);
            }
            Err(StreamError::Closed) => {
                // Stream is closed, we can stop reading
                break;
            }
            Err(e) => {
                return Err(format!("Failed to read from request stream: {e}"));
            }
        }
    }
    Ok(request_body)
}

pub fn build_response(body: &str, status_code: u16, content_type: &str) -> ResponseBuilder {
    let mut builder = ResponseBuilder::new();
    builder
        .set_header("content-type", content_type)
        .set_status_code(status_code)
        .set_body(body);
    builder
}

#[allow(dead_code)]
pub fn build_response_html(body: &str, status_code: u16) -> ResponseBuilder {
    build_response(body, status_code, "text/html; charset=utf-8")
}

#[allow(dead_code)]
pub fn build_response_json(body: &str, status_code: u16) -> ResponseBuilder {
    build_response(body, status_code, "application/json")
}

#[allow(dead_code)]
pub fn build_response_json_error(message: &str, status_code: u16) -> ResponseBuilder {
    let body = format!("{{\"error\": \"{message}\"}}");
    build_response_json(&body, status_code)
}

#[allow(dead_code)]
pub fn build_response_redirect(url: &str) -> ResponseBuilder {
    let mut builder = ResponseBuilder::new();
    builder
        .set_header("content-type", "text/plain")
        .set_header("Location", url)
        .set_status_code(302)
        .set_body("");
    builder
}

//#[cfg(test)]
//mod tests {
//    use super::*;
//
//    // Test ResponseBuilder header and status
//    #[test]
//    fn test_response_builder_setters() {
//        let mut builder = ResponseBuilder::new();
//        builder //.set_header("foo", "bar")
//            .set_status_code(404)
//            .set_body("hello world");
//        assert_eq!(builder.status_code, 404);
//        assert_eq!(builder.body_content.as_deref(), Some("hello world"));
//        let headers_map = parse_headers(&builder.headers);
//        assert!(headers_map.contains_key("foo"));
//        assert_eq!(headers_map.get("foo").unwrap(), &vec!["bar".to_string()]);
//    }
//
//    #[test]
//    fn test_build_response_plain_text() {
//        let response = build_response("abc", 200, "text/plain");
//        assert_eq!(response.status_code, 200);
//    }
//
//    #[test]
//    fn test_build_response_html() {
//        let response = build_response_html("abc", 201);
//        assert_eq!(response.status_code, 201);
//    }
//
//    #[test]
//    fn test_build_response_json() {
//        let response = build_response_json("{\"a\":1}", 202);
//        assert_eq!(response.status_code, 202);
//    }
//
//    #[test]
//    fn test_send_error_json() {
//        let response = build_response_json_error("fail", 500);
//        assert_eq!(response.status_code, 500);
//    }
//
//    #[test]
//    fn test_response_builder_default() {
//        let builder = ResponseBuilder::default();
//        assert_eq!(builder.status_code, 200);
//        assert!(builder.body_content.is_none());
//    }
//}
