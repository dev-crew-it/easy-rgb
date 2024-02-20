//! Plugin implementation
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use serde_json as json;

use clightningrpc_plugin::{commands::RPCCommand, plugin::Plugin};
use clightningrpc_plugin_macros::plugin;

#[derive(Clone, Debug)]
pub(crate) struct State;

impl State {
    pub fn new() -> Self {
        State
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
    plugin.on_init(|_| json::json!({}));

    plugin = plugin.register_hook("onfunding_channel_tx", None, None, OnFundingChannelTx);
    Ok(plugin)
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
