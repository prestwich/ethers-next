use ethers_transports::*;

// TODO: start anvil for these tests
#[tokio::test]
async fn it_calls() {
    let http: Http = "http://127.0.0.1:8545".parse().unwrap();
    let resp: String = http.request("eth_chainId", ()).await.unwrap().unwrap();
    dbg!(resp);
}

#[tokio::test]
async fn it_batch_calls() {
    let http: Http = "http://127.0.0.1:8545".parse().unwrap();

    let reqs = std::iter::repeat("eth_chainId")
        .take(5)
        .map(|method| common::Request::owned(http.next_id(), method, None))
        .collect::<Vec<_>>();
    let resp = http.batch_request(&reqs).await.unwrap();
    dbg!(resp);
}
