use bytes::Bytes;
use serde::de::DeserializeOwned;

use crate::bindings::wasi::http::types::{
    ErrorCode, Headers, IncomingBody, IncomingRequest, Method, ResponseOutparam,
};

impl TryFrom<Method> for http::Method {
    type Error = anyhow::Error;

    fn try_from(method: Method) -> anyhow::Result<Self, Self::Error> {
        Ok(match method {
            Method::Get => http::Method::GET,
            Method::Post => http::Method::POST,
            Method::Put => http::Method::PUT,
            Method::Patch => http::Method::PATCH,
            Method::Delete => http::Method::DELETE,
            Method::Head => http::Method::HEAD,
            Method::Options => http::Method::OPTIONS,
            Method::Trace => http::Method::TRACE,
            _ => anyhow::bail!("Invalid method"),
        })
    }
}

impl TryFrom<IncomingRequest> for http::Request<IncomingBody> {
    type Error = anyhow::Error;

    fn try_from(req: IncomingRequest) -> anyhow::Result<Self, Self::Error> {
        use http::uri;

        use crate::bindings::wasi::http::types::Scheme;

        let body = req
            .consume()
            .map_err(|_| anyhow::anyhow!("Could not consume request body"))?;

        let scheme = match req.scheme() {
            Some(Scheme::Http) => uri::Scheme::HTTP,
            Some(Scheme::Https) => uri::Scheme::HTTPS,
            _ => anyhow::bail!("Invalid scheme"),
        };
        let authority: uri::Authority = match req.authority() {
            Some(authority) => authority.try_into()?,
            None => anyhow::bail!("Missing authority"),
        };
        let path_and_query: uri::PathAndQuery = match req.path_with_query() {
            Some(path_and_query) => path_and_query.try_into()?,
            None => anyhow::bail!("Missing path and query"),
        };
        let uri = uri::Builder::new()
            .scheme(scheme)
            .authority(authority)
            .path_and_query(path_and_query)
            .build()?;

        let mut builder = http::Request::builder()
            .method(http::Method::try_from(req.method())?)
            .uri(uri);

        builder
            .headers_mut()
            .unwrap()
            .extend(http::header::HeaderMap::try_from(req.headers())?);

        Ok(builder.body(body)?)
    }
}

impl TryFrom<Headers> for http::header::HeaderMap {
    type Error = anyhow::Error;

    fn try_from(headers: Headers) -> anyhow::Result<Self, Self::Error> {
        headers
            .entries()
            .into_iter()
            .map(|(name, value)| {
                use http::header::{HeaderName, HeaderValue};

                let name = HeaderName::from_bytes(name.as_bytes())?;
                let value = HeaderValue::from_bytes(&value)?;

                Ok((name, value))
            })
            .collect()
    }
}

impl From<http::header::HeaderMap> for Headers {
    fn from(headers: http::header::HeaderMap) -> Self {
        let entries: Vec<_> = headers
            .into_iter()
            .filter_map(|(name, value)| Some((name?, value)))
            .map(|(name, value)| {
                let name = name.to_string();
                let value = value.as_bytes().to_owned();

                (name, value)
            })
            .collect();
        Headers::from_list(&entries).unwrap()
    }
}

impl IncomingBody {
    pub fn read(&self) -> anyhow::Result<Bytes> {
        use bytes::BytesMut;

        use crate::bindings::wasi::io::streams::StreamError;

        let stream = self
            .stream()
            .map_err(|_| anyhow::anyhow!("Missing request body stream"))?;

        let mut bytes = BytesMut::new();

        loop {
            match stream.read(4096) {
                Ok(frame) => {
                    bytes.extend_from_slice(&frame);
                }
                Err(StreamError::Closed) => break,
                Err(err) => anyhow::bail!("Failed reading request body: {err}"),
            }
        }

        Ok(bytes.freeze())
    }

    pub fn read_json<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        let bytes = self.read()?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl ResponseOutparam {
    pub fn error(self, code: ErrorCode) {
        ResponseOutparam::set(self, Err(code));
    }

    pub fn send(self, res: http::Response<Bytes>) -> anyhow::Result<()> {
        use crate::bindings::wasi::http::types::{OutgoingBody, OutgoingResponse};

        let (parts, body) = res.into_parts();

        let res = OutgoingResponse::new(parts.headers.into());
        let _ = res.set_status_code(parts.status.into());

        let resp_body = res
            .body()
            .map_err(|_| anyhow::anyhow!("Could not get response body"))?;

        ResponseOutparam::set(self, Ok(res));

        let out = resp_body
            .write()
            .map_err(|_| anyhow::anyhow!("Could not get response body writer"))?;
        out.blocking_write_and_flush(&body)?;
        drop(out);

        OutgoingBody::finish(resp_body, None)?;

        Ok(())
    }
}
