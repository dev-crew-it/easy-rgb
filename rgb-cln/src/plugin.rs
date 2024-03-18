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

use clightningrpc_common::client::Client;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::{commands::RPCCommand, plugin::Plugin};
use clightningrpc_plugin_macros::{plugin, rpc_method};

use rgb_common::anyhow;
use rgb_common::bitcoin::bip32::ExtendedPrivKey;
use rgb_common::RGBManager;

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
