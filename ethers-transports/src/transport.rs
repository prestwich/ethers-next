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

    fn batch_request(&self, reqs: &[&Request<'_>]) -> BatchRpcFuture;

    fn request<Params, Resp>(
        &self,
        method: &'static str,
        params: Params,
    ) -> RpcCall<&Self, Self, Params, Resp>
    where
        Self: Sized,
        Params: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        RpcCall::new(self, method, params, self.next_id())
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
    use crate::{transports::Http, Connection, PubSubConnection};

    fn __compile_check() -> Box<dyn Connection> {
        todo!()
    }
    fn __compile_check_pubsub() -> Box<dyn PubSubConnection> {
        todo!()
    }

    #[tokio::test]
    async fn it_calls() {
        let http: Http = "http://127.0.0.1:8545".parse().unwrap();
        let resp: String = http.request("eth_chainId", ()).await.unwrap().unwrap();
        dbg!(resp);
    }

    #[tokio::test]
    async fn it_batch_calls() {
        let http: Http = "http://127.0.0.1:8545".parse().unwrap();
    }
}
