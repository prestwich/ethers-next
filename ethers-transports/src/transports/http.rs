use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use reqwest::{header::HeaderValue, Client, Url};

use crate::{
    common::{Authorization, RawRpcResponse},
    transport::Transport,
    utils::resp_to_raw_result,
    TransportError,
};

#[derive(Debug)]
pub struct HttpInternal {
    id: AtomicU64,
    client: Client,
    url: Url,
}

#[derive(Clone, Debug)]
pub struct Http(Arc<HttpInternal>);

impl Deref for Http {
    type Target = HttpInternal;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
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

#[async_trait::async_trait]
impl Transport for Http {
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
    use crate::Transport;

    use super::Http;

    #[tokio::test]
    async fn chain_id() {
        let http = Http::new("http://127.0.0.1:8545".parse().unwrap());
        let resp: String = http.request("eth_chainId", &()).await.unwrap().unwrap();
        dbg!(&resp);
    }
}
