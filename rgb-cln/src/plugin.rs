//! Plugin implementation
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::{fmt::Debug, sync::Arc};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json as json;

use clightningrpc_common::client::Client;
use clightningrpc_plugin::{commands::RPCCommand, plugin::Plugin};
use clightningrpc_plugin_macros::plugin;

use rgb_common::{anyhow, RGBManager};

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

    pub(crate) fn rpc(&self) -> Arc<Client> {
        self.cln_rpc.clone().unwrap()
    }

    pub(crate) fn manager(&self) -> Arc<RGBManager> {
        self.rgb_manager.clone().unwrap()
    }

    pub fn call<T: Serialize, U: DeserializeOwned + Debug>(
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
        methods: [],
        hooks: [],
    };
    plugin.on_init(on_init);

    plugin = plugin.register_hook("onfunding_channel_tx", None, None, OnFundingChannelTx);
    Ok(plugin)
}

// FIXME: move to another part of the code.
#[derive(Debug, Deserialize)]
pub struct GetInfo {
    id: String,
}

fn on_init(plugin: &mut Plugin<State>) -> json::Value {
    let config = plugin.configuration.clone().unwrap();
    let rpc_file = format!("{}/{}", config.lightning_dir, config.rpc_file);

    let rpc = Client::new(rpc_file);
    plugin.state.cln_rpc = Some(Arc::new(rpc));
    let getinfo: anyhow::Result<GetInfo> = plugin.state.call("getinfo", json::json!({}));
    if let Err(err) = getinfo {
        return json::json!({ "disable": format!("{err}") });
    }
    // SAFETY: Safe to unwrap because we unwrap before.
    let getinfo = getinfo.unwrap();

    // FIXME: I can get the public key from the configuration?
    let manager = RGBManager::init(&config.lightning_dir, &getinfo.id, &config.network);
    if let Err(err) = manager {
        return json::json!({ "disable": format!("{err}") });
    }

    json::json!({})
}

#[derive(Clone, Debug)]
struct OnFundingChannelTx;

impl RPCCommand<State> for OnFundingChannelTx {
    fn call<'c>(
        &self,
        _: &mut Plugin<State>,
        _: json::Value,
    ) -> Result<json::Value, clightningrpc_plugin::errors::PluginError> {
        log::info!("Calling hook `onfunding_channel_tx`");
        Ok(json::json!({ "result": "continue" }))
    }
}
