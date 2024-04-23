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
use serde::Serialize;
use serde_json as json;

use clightningrpc::LightningRPC;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::{commands::RPCCommand, plugin::Plugin};
use clightningrpc_plugin_macros::{notification, plugin, rpc_method};

use rgb_common::anyhow;
use rgb_common::bitcoin::bip32::ExtendedPrivKey;
use rgb_common::RGBManager;

mod macros;
mod walletrpc;

#[derive(Clone, Debug)]
pub(crate) struct State {
    /// The RGB Manager where we ask to do everything
    /// related to lightning.
    rgb_manager: Option<Arc<RGBManager>>,
    /// CLN RPC path
    cln_rpc_path: Option<String>,
}

impl State {
    pub fn new() -> Self {
        State {
            rgb_manager: None,
            cln_rpc_path: None,
        }
    }

    pub(crate) fn manager(&self) -> Arc<RGBManager> {
        self.rgb_manager.clone().unwrap()
    }

    pub fn call<T: Serialize, U: DeserializeOwned + fmt::Debug>(
        &self,
        method: &str,
        payload: T,
    ) -> anyhow::Result<U> {
        let path = self
            .cln_rpc_path
            .as_ref()
            .ok_or(anyhow::anyhow!("cln socket patch not found"))?;
        let rpc = LightningRPC::new(path);
        let response: U = rpc.call(method, payload)?;
        log::debug!("cln answer with {:?}", response);
        return Ok(response);
    }
}

pub fn build_plugin() -> anyhow::Result<Plugin<State>> {
    let mut plugin = plugin! {
        state: State::new(),
        dynamic: true,
        notification: [
            on_block_added,
        ],
        methods: [
            rgb_balance,
            rgb_fundchannel,
            rgb_issue_asset,
            rgb_receive,
            rgb_info,
        ],
        hooks: [],
    };
    plugin.on_init(on_init);

    // FIXME: we disable this because it will create loop
    //plugin = plugin.register_hook("rpc_command", None, None, OnRpcCommand);
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

#[rpc_method(rpc_name = "issueasset", description = "Issue a new RGB asset")]
fn rgb_issue_asset(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    walletrpc::rgb_issue_new_assert(plugin, request)
}

#[rpc_method(rpc_name = "rgbreceive", description = "RGB Receive a asset on chain")]
fn rgb_receive(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    walletrpc::rgb_receive(plugin, request)
}

#[rpc_method(rpc_name = "rgbsendasset", description = "RGB Send a asset on chain")]
fn rgb_send(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    walletrpc::rgb_send(plugin, request)
}

// FIXME: this is just a test, we should remove it at some point
#[rpc_method(rpc_name = "rgbinfo", description = "RGB Information")]
fn rgb_info(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    let info: Value = plugin
        .state
        .call("getinfo", json::json!({}))
        .map_err(|err| error!("{err}"))?;
    Ok(info)
}

#[notification(on = "block_added")]
fn on_block_added(plugin: &mut Plugin<State>, request: &Value) {
    let manager = plugin.state.manager();
    let Err(err) = manager.refresh() else {
        return;
    };
    log::error!("{err}");
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

    plugin.state.cln_rpc_path = Some(rpc_file);

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
