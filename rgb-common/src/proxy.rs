//! A module for operating an RGB HTTP JSON-RPC proxy
use core::str::FromStr;
use core::time::Duration;

use amplify::s;
use bitcoin::Network;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};

use crate::BlockingClient;

const JSON: &str = "application/json";
const PROXY_TIMEOUT: u8 = 90;

#[derive(Debug, Clone)]
pub struct Client {
    inner: BlockingClient,
    network: Network,
}

impl Client {
    pub fn new(network: &str) -> anyhow::Result<Self> {
        let network = Network::from_str(network)?;
        let inner = BlockingClient::builder()
            .timeout(Duration::from_secs(PROXY_TIMEOUT as u64))
            .build()?;
        Ok(Self { inner, network })
    }

    pub fn get_consignment(&self, consignment_id: &str) -> anyhow::Result<JsonRpcResponse<String>> {
        let body = JsonRpcRequest {
            method: s!("consignment.get"),
            jsonrpc: s!("2.0"),
            id: None,
            params: Some(BlindedUtxoParam {
                blinded_utxo: consignment_id.to_owned(),
            }),
        };

        // FIXME: add a URL for this
        let url = "";
        let resp = self
            .inner
            .post(format!("{url}"))
            .header(CONTENT_TYPE, JSON)
            .json(&body)
            .send()?
            .json::<JsonRpcResponse<String>>()?;
        Ok(resp)
    }
}

/// JSON-RPC Error
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JsonRpcError {
    pub(crate) code: i64,
    message: String,
}

/// JSON-RPC request
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest<P> {
    method: String,
    jsonrpc: String,
    id: Option<String>,
    params: Option<P>,
}

/// JSON-RPC response
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JsonRpcResponse<R> {
    id: Option<String>,
    pub(crate) result: Option<R>,
    pub(crate) error: Option<JsonRpcError>,
}

/// Blinded UTXO parameter
#[derive(Debug, Deserialize, Serialize)]
pub struct BlindedUtxoParam {
    blinded_utxo: String,
}
