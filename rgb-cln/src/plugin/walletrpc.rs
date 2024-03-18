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

use rgb_common::bitcoin30;
use rgb_common::core::ContractId;

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

#[derive(Debug, Deserialize, Serialize)]
pub struct FundincStartResponse {
    funding_address: String,
    scriptpubkey: String,
    close_to: String,
    channel_type: json::Value,
}

/// Opening a RGB channel
pub fn fund_rgb_channel(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    log::info!("calling fund rgb channel with `{request}`");
    let request: RGBFundChannelRequest = json::from_value(request)?;
    // FIXME: We should give a string like Tether/USTD and calculate the asset id in here
    // check if the asset id is valit
    let contract_id = ContractId::from_str(&request.asset_id)
        .map_err(|err| error!("decoding contract id return error: `{err}`"))?;

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

    let fundchannel: FundincStartResponse = plugin
        .state
        .call(
            "fundchannel_start",
            json::json!({
                "id": request.peer_id,
                "amount": balance.to_string(),
            }),
        )
        .map_err(|err| error!("{err}"))?;
    let Ok(scriptpubkey) = bitcoin30::ScriptBuf::from_hex(&fundchannel.scriptpubkey) else {
        let _: json::Value = plugin
            .state
            .call(
                "fundchannel_cancel",
                json::json!({
                    "id": request.peer_id,
                }),
            )
            .map_err(|err| error!("{err}"))?;
        return Err(error!("Impossible parse `scriptpubkey`, failing funding"));
    };

    let info = RgbInfo {
        channel_id: "<channel_id>".to_owned(),
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
    let Ok(psbt) =
        plugin
            .state
            .manager()
            .build_rgb_funding_transaction(&info, scriptpubkey, 1.1, 6)
    else {
        let _: json::Value = plugin
            .state
            .call(
                "fundchannel_cancel",
                json::json!({
                    "id": request.peer_id,
                }),
            )
            .map_err(|err| error!("{err}"))?;
        return Err(error!(
            "Impossible .build_rgb_funding_transaction, failing funding"
        ));
    };

    let psbt = psbt.serialize_hex();
    let fundchannel: json::Value = plugin
        .state
        .call(
            "fundchannel_complete",
            json::json!({
                "id": request.peer_id,
                "psbt": psbt,
            }),
        )
        .map_err(|err| error!("{err}"))?;
    Ok(json::json!({
        "info": fundchannel,
        "rgb_info": info,
    }))
}
