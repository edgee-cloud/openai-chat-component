use bytes::Bytes;
use http::header::{HeaderName, HeaderValue};
use http::uri;
use serde::de::DeserializeOwned;

use crate::bindings::wasi::http::types::{
    ErrorCode, Headers, IncomingBody, IncomingRequest, Method, ResponseOutparam, Scheme,
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

fn to_http_request_builder(
    scheme: Option<Scheme>,
    authority: Option<String>,
    path_and_query: Option<String>,
    method: Method,
) -> anyhow::Result<http::request::Builder> {
    let scheme = match scheme {
        Some(Scheme::Http) => uri::Scheme::HTTP,
        Some(Scheme::Https) => uri::Scheme::HTTPS,
        _ => anyhow::bail!("Invalid scheme"),
    };

    let authority: uri::Authority = match authority {
        Some(authority) => authority.try_into()?,
        None => anyhow::bail!("Missing authority"),
    };
    let path_and_query: uri::PathAndQuery = match path_and_query {
        Some(path_and_query) => path_and_query.try_into()?,
        None => anyhow::bail!("Missing path and query"),
    };
    let uri = uri::Builder::new()
        .scheme(scheme)
        .authority(authority)
        .path_and_query(path_and_query)
        .build()?;

    let builder = http::Request::builder()
        .method(http::Method::try_from(method)?)
        .uri(uri);

    Ok(builder)
}

impl TryFrom<IncomingRequest> for http::Request<IncomingBody> {
    type Error = anyhow::Error;

    fn try_from(req: IncomingRequest) -> anyhow::Result<Self, Self::Error> {
        let mut builder = to_http_request_builder(
            req.scheme(),
            req.authority(),
            req.path_with_query(),
            req.method(),
        )?;

        builder
            .headers_mut()
            .unwrap()
            .extend(http::header::HeaderMap::try_from(req.headers())?);

        let body = req
            .consume()
            .map_err(|_| anyhow::anyhow!("Could not consume request body"))?;

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

#[cfg(test)]
mod tests {
    use crate::bindings::wasi::http::types::{Method as WasiMethod, Scheme as WasiScheme};
    use http::Method as HttpMethod;

    #[test]
    fn test_try_from_method_success() {
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Get).unwrap(),
            HttpMethod::GET
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Post).unwrap(),
            HttpMethod::POST
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Put).unwrap(),
            HttpMethod::PUT
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Patch).unwrap(),
            HttpMethod::PATCH
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Delete).unwrap(),
            HttpMethod::DELETE
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Head).unwrap(),
            HttpMethod::HEAD
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Options).unwrap(),
            HttpMethod::OPTIONS
        );
        assert_eq!(
            HttpMethod::try_from(WasiMethod::Trace).unwrap(),
            HttpMethod::TRACE
        );
    }

    #[test]
    fn test_try_from_method_invalid() {
        // Assuming there's a variant not covered, e.g., an unknown value
        let result = HttpMethod::try_from(WasiMethod::Connect);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_http_request_builder_success() {
        let scheme = Some(WasiScheme::Https);
        let authority = Some("example.com".to_string());
        let path_and_query = Some("/api/test?foo=bar".to_string());
        let method = WasiMethod::Get;

        let builder = super::to_http_request_builder(scheme, authority, path_and_query, method)
            .expect("Should build request");

        let req = builder.body(()).unwrap();
        assert_eq!(req.method(), &HttpMethod::GET);
        assert_eq!(req.uri().scheme_str(), Some("https"));
        assert_eq!(
            req.uri().authority().map(|a| a.as_str()),
            Some("example.com")
        );
        assert_eq!(
            req.uri().path_and_query().map(|pq| pq.as_str()),
            Some("/api/test?foo=bar")
        );
    }

    #[test]
    fn test_to_http_request_builder_invalid_scheme() {
        let scheme = None;
        let authority = Some("example.com".to_string());
        let path_and_query = Some("/".to_string());
        let method = WasiMethod::Get;

        let result = super::to_http_request_builder(scheme, authority, path_and_query, method);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_http_request_builder_missing_authority() {
        let scheme = Some(WasiScheme::Http);
        let authority = None;
        let path_and_query = Some("/".to_string());
        let method = WasiMethod::Get;

        let result = super::to_http_request_builder(scheme, authority, path_and_query, method);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_http_request_builder_missing_path_and_query() {
        let scheme = Some(WasiScheme::Http);
        let authority = Some("example.com".to_string());
        let path_and_query = None;
        let method = WasiMethod::Get;

        let result = super::to_http_request_builder(scheme, authority, path_and_query, method);
        assert!(result.is_err());
    }
}
