//! RGB Plugin implementation
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::fmt;
use std::fs;
use std::io;
use std::str::FromStr;
use std::sync::Arc;

use json::Value;
use lightning_signer::bitcoin as vlsbtc;
use lightning_signer::signer::derive::KeyDerive;
use lightning_signer::signer::derive::NativeKeyDerive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json as json;

use clightningrpc_common::client::Client;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::{commands::RPCCommand, plugin::Plugin};
use clightningrpc_plugin_macros::{plugin, rpc_method};

use rgb_common::bitcoin::bip32::ExtendedPrivKey;
use rgb_common::bitcoin::consensus::encode::serialize_hex;
use rgb_common::bitcoin::consensus::Decodable;
use rgb_common::bitcoin::hashes::hex::FromHex;
use rgb_common::bitcoin::psbt::PartiallySignedTransaction;
use rgb_common::RGBManager;
use rgb_common::{anyhow, bitcoin};

mod walletrpc;

#[derive(Clone, Debug)]
pub(crate) struct State {
    /// The RGB Manager where we ask to do everything
    /// related to lightning.
    rgb_manager: Option<Arc<RGBManager>>,
    /// CLN RPC
    cln_rpc: Option<Arc<Client>>,
}

impl State {
    pub fn new() -> Self {
        State {
            rgb_manager: None,
            cln_rpc: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn rpc(&self) -> Arc<Client> {
        self.cln_rpc.clone().unwrap()
    }

    pub(crate) fn manager(&self) -> Arc<RGBManager> {
        self.rgb_manager.clone().unwrap()
    }

    #[allow(dead_code)]
    pub fn call<T: Serialize, U: DeserializeOwned + fmt::Debug>(
        &self,
        method: &str,
        payload: T,
    ) -> anyhow::Result<U> {
        if let Some(rpc) = &self.cln_rpc {
            let response = rpc.send_request(method, payload)?;
            log::debug!("cln answer with {:?}", response);
            if let Some(err) = response.error {
                anyhow::bail!("cln error: {}", err.message);
            }
            return Ok(response.result.unwrap());
        }
        anyhow::bail!("rpc connection to core lightning not available")
    }
}

pub fn build_plugin() -> anyhow::Result<Plugin<State>> {
    let mut plugin = plugin! {
        state: State::new(),
        dynamic: true,
        notification: [ ],
        methods: [
            rgb_balance,
            rgb_fundchannel,
        ],
        hooks: [],
    };
    plugin.on_init(on_init);

    plugin = plugin.register_hook("onfunding_channel_tx", None, None, OnFundingChannelTx);
    Ok(plugin)
}

#[rpc_method(rpc_name = "rgbbalances", description = "Return the RGB balance")]
pub fn rgb_balance(plugin: &mut Plugin<State>, requet: Value) -> Result<Value, PluginError> {
    walletrpc::rgb_balance(plugin, requet)
}

#[rpc_method(rpc_name = "fundrgbchannel", description = "Funding a RGB Channel")]
fn rgb_fundchannel(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    walletrpc::fund_rgb_channel(plugin, request)
}

fn read_secret(file: fs::File, network: &str) -> anyhow::Result<ExtendedPrivKey> {
    let buffer = io::BufReader::new(file);
    let network = vlsbtc::Network::from_str(network)?;
    let hsmd_derive = NativeKeyDerive::new(network);
    let xpriv = hsmd_derive.master_key(buffer.buffer()).to_string();
    let xpriv = ExtendedPrivKey::from_str(&xpriv)?;
    Ok(xpriv)
}

fn on_init(plugin: &mut Plugin<State>) -> json::Value {
    let config = plugin.configuration.clone().unwrap();
    let rpc_file = format!("{}/{}", config.lightning_dir, config.rpc_file);
    let hsmd_file = format!("{}/hsm_secret", config.lightning_dir);

    let hsmd_file = fs::File::open(hsmd_file);
    if let Err(err) = hsmd_file {
        log::error!("failing open the hsmd file: {err}");
        return json::json!({ "disable": format!("{err}") });
    }
    // SAFETY: we check if it is an error just before.
    let hsmd_file = hsmd_file.unwrap();

    let hsmd_secret = read_secret(hsmd_file, &config.network);
    if let Err(err) = hsmd_secret {
        log::error!("failing reading hsmd secret: {err}");
        return json::json!({ "disable": format!("{err}") });
    }
    // SAFETY: we check if it is an error just error.
    let master_xprv = hsmd_secret.unwrap();

    let rpc = Client::new(rpc_file);
    plugin.state.cln_rpc = Some(Arc::new(rpc));

    let manager = RGBManager::init(&config.lightning_dir, &master_xprv, &config.network);
    if let Err(err) = manager {
        log::error!("failing to init the rgb managar: {err}");
        return json::json!({ "disable": format!("{err}") });
    }
    // SAFETY: we check if it is an error just before.
    let manager = manager.unwrap();
    plugin.state.rgb_manager = Some(Arc::new(manager));
    json::json!({})
}

#[derive(Clone, Debug)]
struct OnFundingChannelTx;

#[derive(Clone, Debug, Deserialize)]
struct OnFundingChannelTxHook {
    onfunding_channel_tx: OnFundingChannelTxBody,
}

#[derive(Clone, Debug, Deserialize)]
struct OnFundingChannelTxBody {
    tx: String,
    txid: String,
    psbt: String,
    channel_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct OnFundingChannelTxResponse {
    tx: String,
    psbt: String,
}

impl RPCCommand<State> for OnFundingChannelTx {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<State>,
        body: json::Value,
    ) -> Result<json::Value, clightningrpc_plugin::errors::PluginError> {
        log::info!("Calling hook `onfunding_channel_tx` with `{body}`",);
        let body: OnFundingChannelTxHook = json::from_value(body)?;
        let body = body.onfunding_channel_tx;
        let raw_tx = Vec::from_hex(&body.tx).unwrap();
        let tx: bitcoin::Transaction = Decodable::consensus_decode(&mut raw_tx.as_slice()).unwrap();
        let txid = bitcoin::Txid::from_str(&body.txid).unwrap();
        assert_eq!(txid, tx.txid());

        let psbt_from_base64 =
            bitcoin::base64::decode(&body.psbt).map_err(|err| error!("{err}"))?;
        let mut psbt = PartiallySignedTransaction::deserialize(&psbt_from_base64)
            .map_err(|err| error!("{err}"))?;

        let tx = plugin
            .state
            .manager()
            .handle_onfunding_tx(tx, txid, &mut psbt, body.channel_id)
            .unwrap();
        let result = OnFundingChannelTxResponse {
            tx: serialize_hex(&tx),
            psbt: psbt.serialize_hex(),
        };
        Ok(json::json!({ "result": json::to_value(&result)? }))
    }
}
