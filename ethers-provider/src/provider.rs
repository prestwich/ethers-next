use std::{borrow::Cow, fmt::Debug, str::FromStr, sync::Arc, time::Duration};

use ethers_pub_use::{
    futures_channel::mpsc, once_cell::sync::OnceCell, serde_json::value::RawValue,
};
use ethers_transports::{
    common::*, transports::Http, Connection, PubSubConnection, TransportError,
};

use crate::{DEFAULT_LOCAL_POLL_INTERVAL, DEFAULT_POLL_INTERVAL};

/// An `HttpProvider` is a [`Provider`] backed by an [`Http`] transport. See the
/// provider docs for full details
pub type HttpProvider = Provider<ethers_transports::transports::Http>;

/// Node Clients
#[derive(Copy, Clone, Debug)]
pub enum NodeClient {
    /// Geth
    Geth,
    /// Erigon
    Erigon,
    /// OpenEthereum
    OpenEthereum,
    /// Nethermind
    Nethermind,
    /// Besu
    Besu,
}

impl std::fmt::Display for NodeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeClient::Geth => write!(f, "Geth"),
            NodeClient::Erigon => write!(f, "Erigon"),
            NodeClient::OpenEthereum => write!(f, "OpenEthereum"),
            NodeClient::Nethermind => write!(f, "Nethermind"),
            NodeClient::Besu => write!(f, "Besu"),
        }
    }
}

#[derive(Clone)]
pub struct Provider<T> {
    transport: T,
    node_client: Arc<OnceCell<NodeClient>>,
    interval: Option<Duration>,
}

impl<T> Provider<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            node_client: Default::default(),
            interval: None,
        }
    }

    #[must_use = "Builder method outputs must be used"]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.set_interval(interval);
        self
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = Some(interval);
    }
}

impl<T> Provider<T>
where
    T: Connection,
{
    pub fn interval(&self) -> Duration {
        self.interval.unwrap_or_else(|| match self.is_local() {
            true => DEFAULT_LOCAL_POLL_INTERVAL,
            false => DEFAULT_POLL_INTERVAL,
        })
    }
}

impl<T> std::fmt::Debug for Provider<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = self
            .node_client
            .get()
            .map(ToString::to_string)
            .unwrap_or_else(|| "Unknown".to_owned());
        f.debug_struct("Provider")
            .field("transport", &self.transport)
            .field("_node_client", &node)
            .field("interval", &self.interval)
            .finish()
    }
}

impl FromStr for Provider<Http> {
    type Err = <Http as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self::new)
    }
}

impl<T> ethers_transports::Connection for Provider<T>
where
    T: Connection,
{
    fn is_local(&self) -> bool {
        self.transport.is_local()
    }

    fn increment_id(&self) -> u64 {
        self.transport.increment_id()
    }

    fn json_rpc_request(&self, req: &Request<'_>) -> RpcFuture {
        self.transport.json_rpc_request(req)
    }

    fn batch_request(&self, reqs: Vec<&Request<'_>>) -> BatchRpcFuture {
        self.transport.batch_request(reqs)
    }
}

impl<T> PubSubConnection for Provider<T>
where
    T: PubSubConnection,
{
    fn uninstall_listener(&self, id: [u8; 32]) -> Result<(), TransportError> {
        self.transport.uninstall_listener(id)
    }

    fn install_listener(
        &self,
        id: [u8; 32],
    ) -> Result<mpsc::UnboundedReceiver<Cow<RawValue>>, TransportError> {
        self.transport.install_listener(id)
    }
}
