//! Plugin implementation
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use serde_json as json;

use clightningrpc_plugin::plugin::Plugin;
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
    Ok(plugin)
}
