use std::borrow::Cow;

use ethers_pub_use::{
    serde::{de::DeserializeOwned, Serialize},
    serde_json::{self, value::RawValue},
};
use jsonrpsee_types::{ErrorResponse, Response};

use crate::{common::RawRpcResponse, TransportError};

pub fn to_json_raw_value<S>(s: &S) -> Result<Box<RawValue>, serde_json::Error>
where
    S: Serialize,
{
    RawValue::from_string(serde_json::to_string(s)?)
}

pub fn from_json_val<'de, T, S>(s: S) -> Result<T, TransportError>
where
    T: DeserializeOwned,
    S: AsRef<str> + 'de,
{
    let s = s.as_ref();
    match serde_json::from_str(s) {
        Ok(val) => Ok(val),
        Err(err) => Err(TransportError::SerdeJson {
            err,
            text: s.to_owned(),
        }),
    }
}

pub fn resp_to_raw_result(resp: &str) -> Result<RawRpcResponse, TransportError> {
    if let Ok(err) = serde_json::from_str::<ErrorResponse<'_>>(resp) {
        return Ok(Err(err.into_owned()));
    }
    let deser = serde_json::from_str::<Response<'_, Cow<'_, RawValue>>>(resp);
    match deser {
        Ok(v) => Ok(Ok(v.into_owned())),
        Err(err) => Err(TransportError::SerdeJson {
            err,
            text: resp.to_owned(),
        }),
    }
}
