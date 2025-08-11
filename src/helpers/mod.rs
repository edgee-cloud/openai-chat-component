#![allow(dead_code)]
use anyhow::Result;
use bytes::Bytes;
use http::{Request, Response, StatusCode};

use crate::bindings::wasi::http::types::{IncomingRequest, ResponseOutparam};
use body::{FromBody, IntoBody, Json};

pub mod body;
mod extensions;

// Request handling helpers

pub fn run<I, O, F>(req: IncomingRequest, response_out: ResponseOutparam, handler: F)
where
    F: FnOnce(Request<I>) -> Result<Response<O>>,
    I: FromBody,
    O: IntoBody,
{
    let req: Request<_> = req.try_into().unwrap();

    let (parts, body) = req.into_parts();
    let body = match I::from_body(body) {
        Ok(body) => body,
        Err(err) => {
            eprintln!("Errored during body parsing: {err}");

            let res = json_error_response(StatusCode::BAD_REQUEST, err);
            response_out.send(res).expect("Failed to send response");
            return;
        }
    };
    let req = Request::from_parts(parts, body);

    let res = match handler(req) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Errored during request handling: {err}");

            let res = json_error_response(StatusCode::INTERNAL_SERVER_ERROR, err);
            response_out.send(res).expect("Failed to send response");
            return;
        }
    };

    let (mut parts, data) = res.into_parts();
    data.extend_response_parts(&mut parts);
    let body = data.into_body().unwrap();
    let res = Response::from_parts(parts, body);

    response_out.send(res).expect("Failed to send response");
}

fn json_error_response(status_code: StatusCode, err: anyhow::Error) -> Response<Bytes> {
    Response::builder()
        .status(status_code)
        .body(
            Json(serde_json::json!({
                "error": err.to_string(),
            }))
            .into_body()
            .unwrap(),
        )
        .unwrap()
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_json_error_response_bad_request() {
        let err = anyhow::anyhow!("invalid input");
        let response = json_error_response(StatusCode::BAD_REQUEST, err);

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body_bytes = response.body();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("\"error\":\"invalid input\""));
    }

    #[test]
    fn test_json_error_response_internal_server_error() {
        let err = anyhow::anyhow!("something went wrong");
        let response = json_error_response(StatusCode::INTERNAL_SERVER_ERROR, err);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body_bytes = response.body();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("\"error\":\"something went wrong\""));
    }

    #[test]
    fn test_json_error_response_empty_error() {
        let err = anyhow::anyhow!("");
        let response = json_error_response(StatusCode::NOT_FOUND, err);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body_bytes = response.body();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("\"error\":\"\""));
    }
}
