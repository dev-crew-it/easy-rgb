//! RGB Wallet RPC methods
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::str::FromStr;

use rgb_common::core::ContractId;
use serde::{Deserialize, Serialize};
use serde_json as json;
use serde_json::Value;

use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;

// TODO this should be hidden inside the common crate
use rgb_common::types::RgbInfo;

use crate::plugin::State;

#[derive(Deserialize, Serialize)]
pub struct RGBBalanceRequest {
    asset_id: String,
}

/// Return the balance of an RGB assert
pub fn rgb_balance(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    log::info!("rgbbalances call with body `{request}`");
    let request: RGBBalanceRequest = json::from_value(request).map_err(|err| error!("{err}"))?;
    let balance = plugin
        .state
        .manager()
        .assert_balance(request.asset_id)
        .map_err(|err| error!("{err}"));
    Ok(json::to_value(balance)?)
}

#[derive(Deserialize, Serialize)]
pub struct RGBFundChannelRequest {
    peer_id: String,
    amount_msat: u64,
    asset_id: String,
}

/// Opening a RGB channel
pub fn fund_rgb_channel(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    let request: RGBFundChannelRequest = json::from_value(request)?;

    // check if the asset id is valit
    let contract_id = ContractId::from_str(&request.asset_id)
        .map_err(|err| error!("decoding contract id return error: `{err}`"))?;
    log::info!("opening channel for contract id {contract_id}");

    // Our plugin is not async :/ so this will create a deadlock!
    /*
    let assert_balance: Balance = plugin
        .state
        .call(
            "rgbbalances",
            RGBBalanceRequest {
                asset_id: request.asset_id.clone(),
            },
        )
        .map_err(|err| error!("{err}"))?;

     */
    // FIXME: Check if we are connected with the peer otherwise connect to them

    // FIXME: we need the magic of core lightning here
    let balance = request.amount_msat;
    let assert_balance = plugin
        .state
        .manager()
        .assert_balance(contract_id.to_string())
        .map_err(|err| error!("{err}"))?;
    log::info!("rgbalance {:?}", balance);

    if balance < assert_balance.spendable {
        return Err(error!(
            "Balance avaialbe `{}` is not enough to open a channel of `{}` capacity",
            assert_balance.spendable, balance
        ));
    }

    let fundchannel: json::Value = plugin
        .state
        .call(
            "fundchannel",
            json::json!({
                "id": request.peer_id,
                "amount": balance.to_string(),
            }),
        )
        .map_err(|err| error!("{err}"))?;
    let channel_id = fundchannel["channel_id"].to_string();
    log::info!("RGB channel id `{channel_id}` created");

    let info = RgbInfo {
        channel_id,
        contract_id,
        local_rgb_amount: balance,
        // FIXME: Check that we are not opening a dual funding channel with
        // liquidity ads
        remote_rgb_amount: 0,
    };

    plugin
        .state
        .manager()
        .add_rgb_info(&info, true)
        .map_err(|err| error!("{err}"))?;
    Ok(json::json!({
        "info": fundchannel,
        "rgb_info": info,
    }))
}
