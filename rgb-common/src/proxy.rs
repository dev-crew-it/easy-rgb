//! A module for operating an RGB HTTP JSON-RPC proxy
use core::str::FromStr;
use core::time::Duration;
use std::path::Path;

use amplify::s;
use reqwest::blocking::multipart::Form;
use reqwest::blocking::multipart::Part;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};

use crate::bitcoin::Network;
use crate::BlockingClient;

const JSON: &str = "application/json";
const PROXY_TIMEOUT: u8 = 90;

#[derive(Debug, Clone)]
pub struct ConsignmentClient {
    inner: BlockingClient,
    #[allow(dead_code)]
    network: Network,
    pub url: String,
}

impl ConsignmentClient {
    pub fn new(network: &str) -> anyhow::Result<Self> {
        let network = Network::from_str(network)?;
        let inner = BlockingClient::builder()
            .timeout(Duration::from_secs(PROXY_TIMEOUT as u64))
            .build()?;
        Ok(Self {
            inner,
            network,
            url: "".to_owned(),
        })
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
        let url = format!("{}/TODO", self.url);
        let resp = self
            .inner
            .post(format!("{url}"))
            .header(CONTENT_TYPE, JSON)
            .json(&body)
            .send()?
            .json::<JsonRpcResponse<String>>()?;
        Ok(resp)
    }

    pub fn post_consignment(
        &self,
        consignment_path: &Path,
        recipient_id: String,
        txid: String,
        vout: Option<u32>,
    ) -> anyhow::Result<()> {
        let file_name = consignment_path
            .file_name()
            .map(|filename| filename.to_string_lossy().into_owned())
            .unwrap();
        let consignment_file = Part::file(consignment_path)?.file_name(file_name);
        let params = serde_json::json!({
            "recipient_id": recipient_id,
            "txid": txid,
            "vout": vout,
        });

        let form = Form::new()
            .text("method", "consignment.post")
            .text("jsonrpc", "2.0")
            .text("id", "1")
            .text("params", serde_json::to_string(&params)?)
            .part("file", consignment_file);

        self.inner
            .post(format!("{}", self.url))
            .header(CONTENT_TYPE, JSON)
            .multipart(form)
            .send()?
            .json::<JsonRpcResponse<String>>()?;

        Ok(())
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
