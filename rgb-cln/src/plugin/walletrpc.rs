//! RGB Wallet RPC methods
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json as json;
use serde_json::Value;

use clightningrpc_common::json_utils;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;


use rgb_common::bitcoin30;
use rgb_common::core::ContractId;

use rgb_common::types::RgbInfo;

use crate::plugin::State;

#[derive(Deserialize, Serialize)]
pub struct RGBBalanceRequest {
    asset_id: Option<String>,
}

/// Return the balance of an RGB assert
pub fn rgb_balance(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    log::info!("rgbbalances call with body `{request}`");
    let request: RGBBalanceRequest = json::from_value(request).map_err(|err| error!("{err}"))?;
    let mut assets_balance = json_utils::init_payload();
    if let Some(asset_id) = request.asset_id {
        let balance = plugin.state.manager().assert_balance(asset_id);
        assets_balance = match balance {
            Ok(balance) => json::to_value(balance).map_err(|err| error!("{err}"))?,
            Err(err) => json::json!({
                "warning": err.to_string(),
            }),
        };
    }

    let btc_balance = plugin
        .state
        .manager()
        .wallet()
        .get_btc_balance()
        .map_err(|err| error!("{err}"))?;
    let balance = json::json!({
        "onchain": btc_balance,
        "assets": assets_balance,
    });
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

#[derive(Deserialize, Debug)]
pub struct NewAssetRequest {
    amounts: Vec<u64>,
    ticker: String,
    name: String,
    precision: u8,
}

pub fn rgb_issue_new_assert(
    plugin: &mut Plugin<State>,
    request: Value,
) -> Result<Value, PluginError> {
    log::info!("calling rgb issue asset with request body: `{request}`");
    let request: NewAssetRequest = json::from_value(request)?;
    let rgb = plugin.state.manager();
    let assert = rgb
        .issue_asset_nia(
            request.ticker,
            request.name,
            request.precision,
            request.amounts,
        )
        .map_err(|err| error!("{err}"))?;
    Ok(json::to_value(assert)?)
}

#[derive(Deserialize)]
struct RgbReceiveRequest {
    asset_id: Option<String>,
}

pub fn rgb_receive(plugin: &mut Plugin<State>, request: Value) -> Result<Value, PluginError> {
    log::info!("calling rgb receive with body `{request}`");
    let request: RgbReceiveRequest = json::from_value(request).map_err(|err| error!("{err}"))?;
    let wallet = plugin.state.manager().wallet();

    // Estimate the fee
    let fees: Value = plugin
        .state
        .call("estimatefees", json::json!({}))
        .map_err(|err| error!("{err}"))?;
    log::info!("estimated fee: {fees}");

    let minimum = fees
        .get("feerate_floor")
        .ok_or(error!("not able to find the feerate_floor in: `{fees}`"))?;
    let minimum = minimum.as_i64().unwrap_or_default();
    log::info!("creating utxo with fee `{minimum}`");
    wallet
        .create_utxos(minimum as f32, |psbt| wallet.sing_with_master_key(psbt))
        .map_err(|err| error!("{err}"))?;
    log::info!("get the new blind receive");
    let receive = wallet
        .new_blind_receive(request.asset_id, 6)
        .map_err(|err| error!("{err}"))?;
    Ok(json::json!(receive))
}
