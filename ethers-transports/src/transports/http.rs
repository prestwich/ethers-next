use std::{
    ops::Deref,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use reqwest::{header::HeaderValue, Client, Url};

use ethers_pub_use::async_trait;

use crate::{
    common::{Authorization, RawRpcResponse},
    transport::Connection,
    utils::resp_to_raw_result,
    TransportError,
};

#[derive(Debug)]
pub struct HttpInternal {
    id: AtomicU64,
    client: Client,
    url: Url,
}

impl HttpInternal {
    pub fn new(url: Url) -> Self {
        Self {
            id: Default::default(),
            client: Default::default(),
            url,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Http(Arc<HttpInternal>);

impl Deref for Http {
    type Target = HttpInternal;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl FromStr for Http {
    type Err = <Url as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self::new)
    }
}

impl Http {
    pub fn new(url: Url) -> Self {
        Self::new_with_client(url, Default::default())
    }

    pub fn new_with_client(url: Url, client: Client) -> Self {
        Self(Arc::new(HttpInternal {
            id: Default::default(),
            client,
            url,
        }))
    }

    pub fn new_with_auth(url: Url, auth: Authorization) -> Self {
        let mut auth_value = HeaderValue::from_str(&auth.to_string()).expect("valid auth");
        auth_value.set_sensitive(true);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("reqwest builds");

        Self::new_with_client(url, client)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl Connection for Http {
    fn is_local(&self) -> bool {
        self.url.as_str().contains("127.0.0.1") || self.url.as_str().contains("localhost")
    }

    fn increment_id(&self) -> u64 {
        self.id.fetch_add(1, Ordering::Relaxed)
    }

    async fn json_rpc_request(
        &self,
        req: &jsonrpsee_types::Request<'_>,
    ) -> Result<RawRpcResponse, TransportError> {
        let res = self
            .client
            .post(self.url.as_ref())
            .json(&req)
            .send()
            .await?;
        let body = res.text().await?;
        resp_to_raw_result(&body)
    }
}

#[cfg(test)]
mod test {
    use crate::Connection;

    use super::Http;

    #[tokio::test]
    async fn chain_id() {
        let http = Http::new("http://127.0.0.1:8545".parse().unwrap());
        let resp: String = http.request("eth_chainId", &()).await.unwrap().unwrap();
        dbg!(&resp);
    }
}
