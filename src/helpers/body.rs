use crate::bindings::wasi::http::types::IncomingBody;
use anyhow::Result;
use bytes::Bytes;

pub trait FromBody: Sized {
    fn from_body(body: IncomingBody) -> Result<Self>;
}

impl FromBody for () {
    fn from_body(_: IncomingBody) -> Result<Self> {
        Ok(())
    }
}

impl FromBody for IncomingBody {
    fn from_body(body: IncomingBody) -> Result<Self> {
        Ok(body)
    }
}

impl FromBody for Bytes {
    fn from_body(body: IncomingBody) -> Result<Self> {
        body.read()
    }
}

pub trait IntoBody: Sized {
    fn into_body(self) -> Result<Bytes>;
    fn handle_error(err: anyhow::Error) -> http::Response<Bytes>;

    #[allow(unused_variables)]
    fn extend_response_parts(&self, parts: &mut http::response::Parts) {}
}

impl IntoBody for Bytes {
    fn into_body(self) -> Result<Bytes> {
        Ok(self)
    }

    fn handle_error(_: anyhow::Error) -> http::Response<Bytes> {
        http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(Bytes::new())
            .unwrap()
    }
}

impl IntoBody for () {
    fn into_body(self) -> Result<Bytes> {
        Ok(Bytes::new())
    }

    fn handle_error(err: anyhow::Error) -> http::Response<Bytes> {
        Bytes::handle_error(err)
    }
}

impl IntoBody for String {
    fn into_body(self) -> Result<Bytes> {
        Ok(Bytes::from(self))
    }

    fn handle_error(err: anyhow::Error) -> http::Response<Bytes> {
        Bytes::handle_error(err)
    }
}

// Data types

#[derive(Debug)]
pub struct Json<T>(pub T);

impl<T: serde::de::DeserializeOwned> FromBody for Json<T> {
    fn from_body(body: IncomingBody) -> Result<Self> {
        let bytes = body.read()?;
        let data = serde_json::from_slice(&bytes)?;
        Ok(Self(data))
    }
}

impl<T: serde::Serialize> IntoBody for Json<T> {
    fn into_body(self) -> Result<Bytes> {
        use bytes::{BufMut, BytesMut};

        let mut buf = BytesMut::with_capacity(128).writer();
        serde_json::to_writer(&mut buf, &self.0)?;
        Ok(buf.into_inner().freeze())
    }

    fn handle_error(err: anyhow::Error) -> http::Response<Bytes> {
        json_error_response(err.to_string())
    }

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        parts
            .headers
            .entry(http::header::CONTENT_TYPE)
            .or_insert(http::HeaderValue::from_static("application/json"));
    }
}

pub struct Html<T>(pub T);

impl<T: Into<Bytes>> IntoBody for Html<T> {
    fn into_body(self) -> Result<Bytes> {
        Ok(self.0.into())
    }

    fn handle_error(_err: anyhow::Error) -> http::Response<Bytes> {
        html_error_response()
    }

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        parts
            .headers
            .entry(http::header::CONTENT_TYPE)
            .or_insert(http::HeaderValue::from_static("text/html; charset=utf-8"));
    }
}

// Error responses

fn html_error_response() -> http::Response<Bytes> {
    http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(http::header::CONTENT_TYPE, "text/html")
        .body(super::ERROR_PAGE)
        .unwrap()
}

fn json_error_response(msg: impl Into<String>) -> http::Response<Bytes> {
    http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::to_vec(&serde_json::json!({
                "error": "Internal server error",
                "message": msg.into(),
            }))
            .unwrap()
            .into(),
        )
        .unwrap()
}
