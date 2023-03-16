use ethers_pub_use::{
    hex,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    serde_with::{DeserializeFromStr, SerializeDisplay},
    thiserror,
};
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::ParseIntError,
    str::FromStr,
};
use url::{Host, Url};

use ethers_primitives::{B256, B512, U256};

// TODO
type PeerId = B512;

/// Represents a ENR in discv4.
///
/// Note: this is only an excerpt of the [`NodeRecord`] data structure.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    SerializeDisplay,
    DeserializeFromStr,
    // RlpEncodable, // TODO
    // RlpDecodable, // TODO
)]
pub struct NodeRecord {
    /// The Address of a node.
    pub address: IpAddr,
    /// TCP port of the port that accepts connections.
    pub tcp_port: u16,
    /// UDP discovery port.
    pub udp_port: u16,
    /// Public key of the discovery service
    pub id: PeerId,
}

impl NodeRecord {
    // /// Derive the [`NodeRecord`] from the secret key and addr
    // pub fn from_secret_key(addr: SocketAddr, sk: &SecretKey) -> Self {
    //     let pk = secp256k1::PublicKey::from_secret_key(SECP256K1, sk);
    //     let id = PeerId::from_slice(&pk.serialize_uncompressed()[1..]);
    //     Self::new(addr, id)
    // }

    /// Converts the `address` into an [`Ipv4Addr`] if the `address` is a mapped
    /// [Ipv6Addr](std::net::Ipv6Addr).
    ///
    /// Returns `true` if the address was converted.
    ///
    /// See also [std::net::Ipv6Addr::to_ipv4_mapped]
    pub fn convert_ipv4_mapped(&mut self) -> bool {
        // convert IPv4 mapped IPv6 address
        if let IpAddr::V6(v6) = self.address {
            if let Some(v4) = v6.to_ipv4_mapped() {
                self.address = v4.into();
                return true;
            }
        }
        false
    }

    /// Same as [Self::convert_ipv4_mapped] but consumes the type
    pub fn into_ipv4_mapped(mut self) -> Self {
        self.convert_ipv4_mapped();
        self
    }

    /// Creates a new record from a socket addr and peer id.
    #[allow(unused)]
    pub fn new(addr: SocketAddr, id: PeerId) -> Self {
        Self {
            address: addr.ip(),
            tcp_port: addr.port(),
            udp_port: addr.port(),
            id,
        }
    }

    /// The TCP socket address of this node
    #[must_use]
    pub fn tcp_addr(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.tcp_port)
    }

    /// The UDP socket address of this node
    #[must_use]
    pub fn udp_addr(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.udp_port)
    }
}

impl fmt::Display for NodeRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("enode://")?;
        hex::encode(self.id.as_bytes()).fmt(f)?;
        f.write_char('@')?;
        match self.address {
            IpAddr::V4(ip) => {
                ip.fmt(f)?;
            }
            IpAddr::V6(ip) => {
                // encapsulate with brackets
                f.write_char('[')?;
                ip.fmt(f)?;
                f.write_char(']')?;
            }
        }
        f.write_char(':')?;
        self.tcp_port.fmt(f)?;
        if self.tcp_port != self.udp_port {
            f.write_str("?discport=")?;
            self.udp_port.fmt(f)?;
        }

        Ok(())
    }
}

/// Possible error types when parsing a `NodeRecord`
#[derive(Debug, thiserror::Error)]
pub enum NodeRecordParseError {
    #[error("Failed to parse url: {0}")]
    InvalidUrl(String),
    #[error("Failed to parse id")]
    InvalidId(String),
    #[error("Failed to discport query: {0}")]
    Discport(ParseIntError),
}

impl FromStr for NodeRecord {
    type Err = NodeRecordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s).map_err(|e| NodeRecordParseError::InvalidUrl(e.to_string()))?;

        let address = match url.host() {
            Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
            Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
            Some(Host::Domain(ip)) => IpAddr::V4(
                Ipv4Addr::from_str(ip)
                    .map_err(|e| NodeRecordParseError::InvalidUrl(e.to_string()))?,
            ),
            _ => {
                return Err(NodeRecordParseError::InvalidUrl(format!(
                    "invalid host: {url:?}"
                )))
            }
        };
        let port = url
            .port()
            .ok_or_else(|| NodeRecordParseError::InvalidUrl("no port specified".to_string()))?;

        let udp_port = if let Some(discovery_port) =
            url.query_pairs().find_map(|(maybe_disc, port)| {
                if maybe_disc.as_ref() == "discport" {
                    Some(port)
                } else {
                    None
                }
            }) {
            discovery_port
                .parse::<u16>()
                .map_err(NodeRecordParseError::Discport)?
        } else {
            port
        };

        let id = url
            .username()
            .parse::<PeerId>()
            .map_err(|e| NodeRecordParseError::InvalidId(e.to_string()))?;

        Ok(Self {
            address,
            id,
            tcp_port: port,
            udp_port,
        })
    }
}

/// The status of the network being ran by the local node.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NetworkStatus {
    /// The local node client version.
    pub client_version: String,
    /// The current ethereum protocol version
    pub protocol_version: u64,
    /// Information about the Ethereum Wire Protocol.
    pub eth_protocol_info: EthProtocolInfo,
}
/// Information about the Ethereum Wire Protocol (ETH)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthProtocolInfo {
    /// The current difficulty at the head of the chain.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "reth_primitives::serde_helper::deserialize_json_u256")
    )]
    pub difficulty: U256,
    /// The block hash of the head of the chain.
    pub head: B256,
    /// Network ID in base 10.
    pub network: u64,
    /// Genesis block of the current chain.
    pub genesis: B256,
}

/// Represents the `admin_nodeInfo` response, which can be queried for all the information
/// known about the running node at the networking granularity.
///
/// Note: this format is not standardized. Reth follows Geth's format,
/// see: <https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-admin>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Enode of the node in URL format.
    pub enode: NodeRecord,
    /// ID of the local node.
    pub id: PeerId,
    /// IP of the local node.
    pub ip: IpAddr,
    /// Address exposed for listening for the local node.
    #[serde(rename = "listenAddr")]
    pub listen_addr: SocketAddr,
    /// Ports exposed by the node for discovery and listening.
    pub ports: Ports,
    /// Name of the network
    pub name: String,
    /// Networking protocols being run by the local node.
    pub protocols: Protocols,
}

impl NodeInfo {
    /// Creates a new instance of `NodeInfo`.
    pub fn new(enr: NodeRecord, status: NetworkStatus) -> NodeInfo {
        NodeInfo {
            enode: enr,
            id: enr.id,
            ip: enr.address,
            listen_addr: enr.tcp_addr(),
            ports: Ports {
                discovery: enr.udp_port,
                listener: enr.tcp_port,
            },
            name: status.client_version,
            protocols: Protocols {
                eth: status.eth_protocol_info,
                other: Default::default(),
            },
        }
    }
}

/// All supported protocols
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protocols {
    /// Info about `eth` sub-protocol
    pub eth: EthProtocolInfo,
    /// Placeholder for any other protocols
    #[serde(flatten, default)]
    pub other: BTreeMap<String, Value>,
}

/// Ports exposed by the node for discovery and listening.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ports {
    /// Port exposed for node discovery.
    pub discovery: u16,
    /// Port exposed for listening.
    pub listener: u16,
}

#[cfg(test)]
mod tests {
    use ethers_pub_use::serde_json;

    use super::*;

    // TODO
    // #[test]
    fn _test_parse_node_info_roundtrip() {
        let sample = r#"{"enode":"enode://44826a5d6a55f88a18298bca4773fca5749cdc3a5c9f308aa7d810e9b31123f3e7c5fba0b1d70aac5308426f47df2a128a6747040a3815cc7dd7167d03be320d@[::]:30303","id":"44826a5d6a55f88a18298bca4773fca5749cdc3a5c9f308aa7d810e9b31123f3e7c5fba0b1d70aac5308426f47df2a128a6747040a3815cc7dd7167d03be320d","ip":"::","listenAddr":"[::]:30303","name":"reth","ports":{"discovery":30303,"listener":30303},"protocols":{"eth":{"difficulty":17334254859343145000,"genesis":"0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3","head":"0xb83f73fbe6220c111136aefd27b160bf4a34085c65ba89f24246b3162257c36a","network":1}}}"#;

        let info: NodeInfo = serde_json::from_str(sample).unwrap();
        let serialized = serde_json::to_string_pretty(&info).unwrap();
        let de_serialized: NodeInfo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(info, de_serialized)
    }
}
