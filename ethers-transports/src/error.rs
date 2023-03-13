use ethers_pub_use::{serde_json, thiserror};

#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    /// SerdeJson (de)ser
    #[error("{err}")]
    SerdeJson {
        err: serde_json::Error,
        text: String,
    },

    /// Http transport
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}
