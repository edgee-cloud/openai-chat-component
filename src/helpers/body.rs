use crate::bindings::wasi::http::types::IncomingBody;
use anyhow::Result;
use bytes::Bytes;

pub trait FromBody: Sized {
    fn from_data(data: Bytes) -> Result<Self>;

    fn from_body(body: IncomingBody) -> Result<Self> {
        Self::from_data(body.read()?)
    }
}

pub trait IntoBody: Sized {
    fn into_body(self) -> Result<Bytes>;

    #[allow(unused_variables)]
    fn extend_response_parts(&self, parts: &mut http::response::Parts) {}
}

impl FromBody for IncomingBody {
    fn from_data(_: Bytes) -> Result<Self> {
        unimplemented!("Should never be called")
    }

    fn from_body(body: IncomingBody) -> Result<Self> {
        Ok(body)
    }
}

impl FromBody for Bytes {
    fn from_data(data: Bytes) -> Result<Self> {
        Ok(data)
    }
}

impl IntoBody for Bytes {
    fn into_body(self) -> Result<Bytes> {
        Ok(self)
    }
}

impl FromBody for () {
    fn from_data(_: Bytes) -> Result<Self> {
        Ok(())
    }

    fn from_body(_: IncomingBody) -> Result<Self> {
        Ok(())
    }
}

impl IntoBody for () {
    fn into_body(self) -> Result<Bytes> {
        Ok(Bytes::new())
    }
}

impl FromBody for String {
    fn from_data(data: Bytes) -> Result<Self> {
        String::from_utf8(data.into()).map_err(Into::into)
    }
}

impl IntoBody for String {
    fn into_body(self) -> Result<Bytes> {
        Ok(Bytes::from(self))
    }
}

impl<T: FromBody> FromBody for Option<T> {
    fn from_data(data: Bytes) -> Result<Self> {
        if data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(T::from_data(data)?))
        }
    }
}

impl<T: IntoBody> IntoBody for Option<T> {
    fn into_body(self) -> Result<Bytes> {
        match self {
            Some(value) => value.into_body(),
            None => Ok(Bytes::new()),
        }
    }

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        if let Some(value) = self {
            value.extend_response_parts(parts);
        }
    }
}

// Data types

#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

impl<T: serde::de::DeserializeOwned> FromBody for Json<T> {
    fn from_data(bytes: Bytes) -> Result<Self> {
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

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        parts
            .headers
            .entry(http::header::CONTENT_TYPE)
            .or_insert(http::HeaderValue::from_static("application/json"));
    }
}

#[derive(Debug, Clone)]
pub struct RawJson<T>(pub T);

impl<T: Into<Bytes>> IntoBody for RawJson<T> {
    fn into_body(self) -> Result<Bytes> {
        Ok(self.0.into())
    }

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        Json(()).extend_response_parts(parts)
    }
}

#[derive(Debug, Clone)]
pub struct Html<T>(pub T);

impl<T: Into<Bytes>> IntoBody for Html<T> {
    fn into_body(self) -> Result<Bytes> {
        Ok(self.0.into())
    }

    fn extend_response_parts(&self, parts: &mut http::response::Parts) {
        parts
            .headers
            .entry(http::header::CONTENT_TYPE)
            .or_insert(http::HeaderValue::from_static("text/html; charset=utf-8"));
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_bytes_from_data() {
        let data = Bytes::from("hello");
        let result = Bytes::from_data(data.clone()).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_bytes_into_body() {
        let data = Bytes::from("world");
        let result = data.clone().into_body().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_unit_from_data() {
        let data = Bytes::from("ignored");
        let result = <()>::from_data(data).unwrap();
        assert_eq!(result, ());
    }

    #[test]
    fn test_unit_into_body() {
        let result = ().into_body().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_string_from_data() {
        let data = Bytes::from("hello world");
        let result = String::from_data(data).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_string_into_body() {
        let s = String::from("abc");
        let result = s.into_body().unwrap();
        assert_eq!(result, Bytes::from("abc"));
    }

    #[test]
    fn test_option_from_data_some() {
        let data = Bytes::from("foo");
        let result: Option<String> = Option::<String>::from_data(data).unwrap();
        assert_eq!(result, Some("foo".to_string()));
    }

    #[test]
    fn test_option_from_data_none() {
        let data = Bytes::new();
        let result: Option<String> = Option::<String>::from_data(data).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_option_into_body_some() {
        let opt = Some(Bytes::from("bar"));
        let result = opt.into_body().unwrap();
        assert_eq!(result, Bytes::from("bar"));
    }

    #[test]
    fn test_option_into_body_none() {
        let opt: Option<Bytes> = None;
        let result = opt.into_body().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_json_from_data_and_into_body() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Test {
            a: i32,
        }
        let obj = Test { a: 42 };
        let json_bytes = serde_json::to_vec(&obj).unwrap();
        let json = Json::<Test>::from_data(Bytes::from(json_bytes)).unwrap();
        assert_eq!(json.0, obj);

        let body = json.into_body().unwrap();
        let decoded: Test = serde_json::from_slice(&body).unwrap();
        assert_eq!(decoded, obj);
    }

    #[test]
    fn test_raw_json_into_body() {
        let raw = RawJson(Bytes::from("{\"x\":1}"));
        let result = raw.into_body().unwrap();
        assert_eq!(result, Bytes::from("{\"x\":1}"));
    }

    #[test]
    fn test_html_into_body() {
        let html = Html(Bytes::from("<h1>hi</h1>"));
        let result = html.into_body().unwrap();
        assert_eq!(result, Bytes::from("<h1>hi</h1>"));
    }

    #[test]
    fn test_json_extend_response_parts_sets_content_type() {
        #[derive(serde::Serialize)]
        struct Dummy {
            x: i32,
        }
        let json = Json(Dummy { x: 1 });
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        json.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_html_extend_response_parts_sets_content_type() {
        let html = Html(Bytes::from("<p>test</p>"));
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        html.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn test_raw_json_extend_response_parts_sets_content_type() {
        let raw = RawJson(Bytes::from("{}"));
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        raw.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_option_extend_response_parts_some() {
        #[derive(serde::Serialize)]
        struct Dummy {
            x: i32,
        }
        let json = Some(Json(Dummy { x: 2 }));
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        json.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_option_extend_response_parts_none() {
        let json: Option<Json<()>> = None;
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        json.extend_response_parts(&mut parts);
        assert!(parts.headers.get(http::header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn test_bytes_extend_response_parts_does_nothing() {
        let bytes = Bytes::from("data");
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        bytes.extend_response_parts(&mut parts);
        assert!(parts.headers.get(http::header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn test_unit_extend_response_parts_does_nothing() {
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        ().extend_response_parts(&mut parts);
        assert!(parts.headers.get(http::header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn test_string_extend_response_parts_does_nothing() {
        let s = String::from("abc");
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        s.extend_response_parts(&mut parts);
        assert!(parts.headers.get(http::header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn test_json_extend_response_parts_doesnt_overwrite_existing_content_type() {
        #[derive(serde::Serialize)]
        struct Dummy {
            x: i32,
        }
        let json = Json(Dummy { x: 1 });
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        parts.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/plain"),
        );
        json.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        // Should remain as "text/plain" since or_insert does not overwrite
        assert_eq!(content_type, "text/plain");
    }

    #[test]
    fn test_html_extend_response_parts_doesnt_overwrites_existing_content_type() {
        let html = Html(Bytes::from("<p>test</p>"));
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        parts.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        html.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        // Should remain as "application/json" since or_insert does not overwrite
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_json_extend_response_parts_adds_content_type() {
        #[derive(serde::Serialize)]
        struct Dummy {
            x: i32,
        }
        let json = Json(Dummy { x: 1 });
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        json.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        // Should remain as "text/plain" since or_insert does not overwrite
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_html_extend_response_parts_adds_content_type() {
        let html = Html(Bytes::from("<p>test</p>"));
        let (mut parts, _) = http::response::Response::new("ok").into_parts();
        html.extend_response_parts(&mut parts);
        let content_type = parts.headers.get(http::header::CONTENT_TYPE).unwrap();
        // Should remain as "application/json" since or_insert does not overwrite
        assert_eq!(content_type, "text/html; charset=utf-8");
    }
}
