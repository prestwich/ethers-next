use serde::{de::DeserializeOwned, Serialize};

use serde_json::value::RawValue;

use std::{borrow::Cow, fmt::Debug, future::Future, pin::Pin};

use crate::{common::*, TransportError};

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait Transport: Debug + Send + Sync {
    fn increment_id(&self) -> u64;

    fn next_id(&self) -> Id<'static> {
        Id::Number(self.increment_id())
    }

    async fn json_rpc_request(
        &self,
        req: &jsonrpsee_types::Request<'_>,
    ) -> Result<RawRpcResponse, TransportError>;

    async fn request_raw(
        &self,
        method: &str,
        params: &RawValue,
    ) -> Result<RawRpcResult, TransportError> {
        let req = jsonrpsee_types::Request::new(method.into(), Some(params), self.next_id());

        let resp = self.json_rpc_request(&req).await?;

        Ok(resp
            .map(|r| r.result)
            .map_err(|e| e.error_object().to_owned().into_owned()))
    }

    fn request<Param, Resp>(
        &self,
        method: &'static str,
        params: &Param,
    ) -> Pin<Box<dyn Future<Output = Result<RpcResult<Resp>, TransportError>> + '_>>
    where
        Self: Sized,
        Param: Serialize,
        Resp: DeserializeOwned,
    {
        match crate::utils::to_json_raw_value(params) {
            Err(err) => Box::pin(async move {
                Err(TransportError::SerdeJson {
                    err,
                    text: Default::default(),
                })
            }),
            Ok(params) => Box::pin(async move {
                let resp = self.request_raw(method, &params).await?;

                match resp {
                    Ok(v) => Ok(Ok(crate::utils::from_json_val(v.get())?)),
                    Err(e) => Ok(Err(e)),
                }
            }),
        }
    }
}

pub trait PubSubTransport: Transport {
    #[doc(hidden)]
    fn uninstall_listener(&self, id: [u8; 32]) -> Result<(), TransportError>;

    #[doc(hidden)]
    fn install_listener(
        &self,
        id: [u8; 32],
    ) -> Result<futures_channel::mpsc::UnboundedReceiver<Cow<RawValue>>, TransportError>;
}

#[cfg(test)]
mod test {
    use crate::{PubSubTransport, Transport};

    fn __compile_check() -> Box<dyn Transport> {
        todo!()
    }
    fn __compile_check_pubsub() -> Box<dyn PubSubTransport> {
        todo!()
    }
}
