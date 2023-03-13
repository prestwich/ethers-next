use base64::{engine::general_purpose, Engine};
use serde_json::value::RawValue;
use std::{borrow::Cow, fmt};

pub use jsonrpsee_types::{ErrorObject, ErrorResponse, Id, Response};

type ReqRes<'a, T> = Result<jsonrpsee_types::Response<'a, T>, ErrorResponse<'a>>;
type RawRes<'a> = ReqRes<'a, Cow<'a, RawValue>>;
pub type RpcResponse<T> = ReqRes<'static, T>;
pub type RawRpcResponse = RawRes<'static>;

pub type RpcResult<T> = Result<T, ErrorObject<'static>>;
pub type RawRpcResult = RpcResult<Cow<'static, RawValue>>;

/// Basic or bearer authentication in http or websocket transport
///
/// Use to inject username and password or an auth token into requests
#[derive(Clone, Debug)]
pub enum Authorization {
    /// HTTP Basic Auth
    Basic(String),
    /// Bearer Auth
    Bearer(String),
}

impl Authorization {
    /// Make a new basic auth
    pub fn basic(username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        let username = username.as_ref();
        let password = password.as_ref();
        let auth_secret = general_purpose::STANDARD.encode(format!("{username}:{password}"));
        Self::Basic(auth_secret)
    }

    /// Make a new bearer auth
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer(token.into())
    }
}

impl fmt::Display for Authorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Authorization::Basic(auth_secret) => write!(f, "Basic {auth_secret}"),
            Authorization::Bearer(token) => write!(f, "Bearer {token}"),
        }
    }
}
