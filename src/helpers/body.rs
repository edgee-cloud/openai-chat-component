use bytes::Bytes;
use serde::Serialize;

pub fn json<T: Serialize>(value: T) -> anyhow::Result<Bytes> {
    Ok(serde_json::to_vec(&value)?.into())
}
