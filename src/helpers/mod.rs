#![allow(dead_code)]
use anyhow::Result;
use bytes::Bytes;

use crate::bindings::wasi::http::types::{IncomingRequest, ResponseOutparam};

pub mod body;
mod extensions;

const ERROR_PAGE: Bytes = Bytes::from_static(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/public/error.html"
)));

// Request handling helpers

pub fn run<I, O, F>(req: IncomingRequest, response_out: ResponseOutparam, handler: F)
where
    F: FnOnce(http::Request<I>) -> Result<http::Response<O>>,
    I: body::FromBody,
    O: body::IntoBody,
{
    let req: http::Request<_> = req.try_into().unwrap();

    let (parts, body) = req.into_parts();
    let body = match I::from_body(body) {
        Ok(body) => body,
        Err(err) => {
            let res = O::handle_error(err);
            response_out.send(res).expect("Failed to send response");
            return;
        }
    };
    let req = http::Request::from_parts(parts, body);

    let res = match handler(req) {
        Ok(res) => res,
        Err(err) => {
            let res = O::handle_error(err);
            response_out.send(res).expect("Failed to send response");
            return;
        }
    };

    let (mut parts, data) = res.into_parts();
    data.extend_response_parts(&mut parts);
    let body = data.into_body().unwrap();
    let res = http::Response::from_parts(parts, body);

    response_out.send(res).expect("Failed to send response");
}
