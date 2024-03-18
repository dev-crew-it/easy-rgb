//! Test Utils
use std::{str::FromStr, sync::Arc};

use clightning_testing::{btc, cln};
use serde_json::json;

#[macro_export]
macro_rules! wait {
    ($callback:expr, $timeout:expr) => {{
        let mut success = false;
        for wait in 0..$timeout {
            let result = $callback();
            if let Err(_) = result {
                std::thread::sleep(std::time::Duration::from_millis(wait));
                continue;
            }
            log::info!("callback completed in {wait} milliseconds");
            success = true;
            break;
        }
        assert!(success, "callback got a timeout");
    }};
    ($callback:expr) => {
        $crate::wait!($callback, 100);
    };
}

#[macro_export]
macro_rules! node {
    ($btc:expr) => {{
        let pwd = std::env!("PWD");
        let plugin_name = std::env!("PLUGIN_NAME");
        log::debug!("plugin path: {pwd}/../{plugin_name}");
        cln::Node::with_btc_and_params(
            $btc,
            &format!("--developer --experimental-offers --plugin={pwd}/target/debug/{plugin_name}"),
            "regtest",
        )
        .await?
    }};
    () => {{
        let pwd = std::env!("PWD");
        let plugin_name = std::env!("PLUGIN_NAME");
        log::debug!("plugin path: {pwd}/../{plugin_name}");
        cln::Node::with_params(
            &format!("--developer --experimental-offers --plugin={pwd}/target/debug/{plugin_name}"),
            "regtest",
        )
        .await?
    }};
}

#[macro_export]
macro_rules! check {
    ($cln:expr, $value:expr, $($arg:tt)+) => {{
        if $value.is_err() {
            let _ = $cln.print_logs();
        }
        assert!($value.is_ok());
    }};
}

#[macro_export]
macro_rules! wait_sync {
    ($cln:expr) => {{
        wait!(
            || {
                let Ok(cln_info) = $cln.rpc().getinfo() else {
                    return Err(());
                };
                log::trace!("cln info: {:?}", cln_info);
                if cln_info.warning_bitcoind_sync.is_some() {
                    return Err(());
                }

                if cln_info.warning_lightningd_sync.is_some() {
                    return Err(());
                }
                let mut out = $cln.rpc().listfunds().unwrap().outputs;
                log::trace!("{:?}", out);
                out.retain(|tx| tx.status == "confirmed");
                if out.is_empty() {
                    let addr = $cln.rpc().newaddr(None).unwrap().bech32.unwrap();
                    let _ = fund_wallet($cln.btc(), &addr, 6);
                    return Err(());
                }

                Ok(())
            },
            10000
        );
    }};
}

pub fn make_new_asset_id(node: &cln::Node, ticker: String, name: String) -> anyhow::Result<String> {
    let asset: serde_json::Value = node.rpc().call(
        "issueasset",
        json!({
            "name": name,
            "ticker": ticker,
            "amounts": [10000],
            "precision": 0,
        }),
    )?;
    log::info!("new asset generated is `{asset}`");
    let asset_id = asset.get("asset_id").unwrap();
    Ok(asset_id.to_string())
}

/// Open a channel from node_a -> node_b
pub fn open_rgb_channel(
    node_a: &cln::Node,
    node_b: &cln::Node,
    _dual_open: bool,
) -> anyhow::Result<()> {
    let addr = node_a.rpc().newaddr(None)?.bech32.unwrap();
    fund_wallet(node_a.btc(), &addr, 8)?;
    wait_for_funds(node_a)?;

    wait_sync!(node_a);

    let getinfo2 = node_b.rpc().getinfo()?;
    node_a
        .rpc()
        .connect(&getinfo2.id, Some(&format!("127.0.0.1:{}", node_b.port)))?;
    let asset_id = make_new_asset_id(node_a, "USTD".to_string(), "Tether".to_string())?;
    let value: serde_json::Value = node_a.rpc().call(
        "fundrgbchannel",
        serde_json::json!({
            "peer_id": getinfo2.id,
            "amount_msat": "all",
            "asset_id": asset_id,
        }),
    )?;
    log::info!("`{value}`");
    wait!(
        || {
            let mut channels = node_a.rpc().listfunds().unwrap().channels;
            log::info!("{:?}", channels);
            let origin_size = channels.len();
            channels.retain(|chan| chan.state == "CHANNELD_NORMAL");
            if channels.len() == origin_size {
                return Ok(());
            }
            let addr = node_a.rpc().newaddr(None).unwrap().bech32.unwrap();
            fund_wallet(node_a.btc(), &addr, 6).unwrap();
            wait_sync!(node_a);
            Err(())
        },
        10000
    );
    Ok(())
}

pub fn fund_wallet(btc: Arc<btc::BtcNode>, addr: &str, blocks: u64) -> anyhow::Result<String> {
    use clightning_testing::prelude::bitcoincore_rpc;
    use clightning_testing::prelude::bitcoincore_rpc::RpcApi;
    // mine some bitcoin inside the lampo address
    let address = bitcoincore_rpc::bitcoin::Address::from_str(addr)
        .unwrap()
        .assume_checked();
    let _ = btc.rpc().generate_to_address(blocks, &address).unwrap();

    Ok(address.to_string())
}

pub fn wait_for_funds(cln: &cln::Node) -> anyhow::Result<()> {
    use clightning_testing::prelude::bitcoincore_rpc;
    use clightning_testing::prelude::bitcoincore_rpc::RpcApi;

    wait!(
        || {
            let addr = cln.rpc().newaddr(None).unwrap().bech32.unwrap();
            let address = bitcoincore_rpc::bitcoin::Address::from_str(&addr)
                .unwrap()
                .assume_checked();
            let _ = cln.btc().rpc().generate_to_address(1, &address).unwrap();

            let Ok(funds) = cln.rpc().listfunds() else {
                return Err(());
            };
            log::trace!("listfunds {:?}", funds);
            if funds.outputs.is_empty() {
                return Err(());
            }
            Ok(())
        },
        10000
    );
    Ok(())
}
