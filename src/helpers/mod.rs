#![allow(dead_code)]
pub mod body;
mod extensions;

use anyhow::Result;
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

use crate::bindings::wasi::http::types::{IncomingBody, IncomingRequest, ResponseOutparam};

const ERROR_PAGE: Bytes = Bytes::from_static(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/public/error.html"
)));

// Request handling helpers

/// This helper allow to run a standard HTTP request handler using the crate `http`
///
/// It takes an `http::Request` in input and returns a Result of an `http::Response` value
pub fn run<F, B>(req: IncomingRequest, response_out: ResponseOutparam, handler: F)
where
    F: FnOnce(http::Request<IncomingBody>) -> Result<http::Response<B>, anyhow::Error>,
    Bytes: From<B>,
{
    let req = req.try_into().unwrap();

    // Run handler
    let Ok(res) = handler(req) else {
        let error_res = http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(ERROR_PAGE)
            .unwrap();
        response_out
            .send(error_res)
            .expect("Could not send error response");

        return;
    };

    // Send final response
    response_out
        .send(res.map(Into::into))
        .expect("Could not send response");
}

/// This helper allow to run a standard HTTP JSON request handler using the crate `http`
///
/// It takes an `http::Request` in input and returns a Result of an `http::Response` value
pub fn run_json<F, I: DeserializeOwned, O: Serialize>(
    req: IncomingRequest,
    response_out: ResponseOutparam,
    handler: F,
) where
    F: FnOnce(http::Request<I>) -> Result<http::Response<O>, anyhow::Error>,
{
    let req: http::Request<IncomingBody> = req.try_into().unwrap();

    // Decode request body
    let (parts, body) = req.into_parts();
    let Ok(value) = body.read_json() else {
        handle_json_error(response_out, "Invalid JSON request");
        return;
    };
    let req = http::Request::from_parts(parts, value);

    // Run handler
    let Ok(res) = handler(req) else {
        handle_json_error(response_out, "Error during request handling");
        return;
    };

    // Encode response body
    let (mut parts, value) = res.into_parts();
    let Ok(body) = body::json(value) else {
        handle_json_error(response_out, "Invalid JSON response");
        return;
    };

    // Set content-type header
    parts.headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );

    let res = http::Response::from_parts(parts, body);

    // Send final response
    response_out.send(res).expect("Could not send response");
}

fn handle_json_error(response_out: ResponseOutparam, msg: impl Into<String>) {
    let error_res = http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(
            body::json(serde_json::json!({
                "error": "Internal server error",
                "message": msg.into(),
            }))
            .unwrap(),
        )
        .unwrap();
    response_out
        .send(error_res)
        .expect("Could not send error response");
}
