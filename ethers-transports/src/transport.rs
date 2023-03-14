use ethers_pub_use::{
    futures_channel,
    serde::{Deserialize, Serialize},
    serde_json::value::RawValue,
};

use std::{borrow::Cow, fmt::Debug};

use crate::{call::RpcCall, common::*, TransportError};

pub trait Connection: Debug + Send + Sync {
    fn is_local(&self) -> bool;

    fn increment_id(&self) -> u64;

    fn next_id(&self) -> Id<'static> {
        Id::Number(self.increment_id())
    }

    fn json_rpc_request(&self, req: &Request<'_>) -> RpcFuture;

    fn batch_request(&self, reqs: Vec<&Request<'_>>) -> BatchRpcFuture;

    fn request<Params, Resp>(
        &self,
        method: &'static str,
        params: Params,
    ) -> RpcCall<Self, Params, Resp>
    where
        Self: Clone,
        Params: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        RpcCall::new(self.clone(), method, params, self.next_id())
    }
}

pub trait PubSubConnection: Connection {
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
    use crate::{Connection, PubSubConnection};

    fn __compile_check() -> Box<dyn Connection> {
        todo!()
    }
    fn __compile_check_pubsub() -> Box<dyn PubSubConnection> {
        todo!()
    }
}
