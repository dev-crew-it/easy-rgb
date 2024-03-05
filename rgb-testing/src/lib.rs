//! Core Lightning Integration testing
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>

#[cfg(test)]
mod utils;

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use json::Value;
    use serde::Deserialize;
    use serde_json as json;

    use clightning_testing::cln;

    use crate::node;
    #[allow(unused_imports)]
    use crate::utils::*;

    static INIT: Once = Once::new();

    fn init() {
        // ignore error
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_plugin() -> anyhow::Result<()> {
        init();
        let cln = node!();
        let result = cln
            .rpc()
            .call::<json::Value, json::Value>("getinfo", json::json!({}));
        log::info!(target: "test_init_plugin", "{:?}", result);
        assert!(result.is_ok(), "{:?}", result);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(560000)]
    async fn test_simple_open_rgb_channel() -> anyhow::Result<()> {
        init();

        let ocean_ln = node!();
        let btc = ocean_ln.btc();
        let miner_1 = node!(btc.clone());
        if let Err(err) = open_channel(&miner_1, &ocean_ln, false) {
            miner_1.print_logs()?;
            panic!("{err}");
        }

        ocean_ln.print_logs()?;

        #[derive(Deserialize, Debug)]
        struct Invoice {
            bolt11: String,
        }

        // the miner generate the payout reusable offer
        let payout_miner: Invoice = miner_1.rpc().call(
            "invoice",
            json::json!({
                "amount_msat": "any",
                "label": "invoice",
                "description": "invoice1",

            }),
        )?;

        log::info!("offer invoice: {:?}", payout_miner);
        // FIXME: we are not able at the moment to splice the channel to increase the balance,
        // so at the moment, so atm we open a new channel but this is not inside our simulation
        open_channel(&ocean_ln, &miner_1, false)?;

        let listchannels = ocean_ln.rpc().listchannels(None, None, None)?.channels;
        log::debug!(
            "channels before paying: {}",
            json::to_string(&listchannels)?
        );
        let listchannels = ocean_ln.rpc().listfunds()?.channels;
        log::debug!(
            "channels in list funds before paying: {}",
            json::to_string(&listchannels)?
        );
        let payout: Value = ocean_ln.rpc().call(
            "pay",
            json::json!({
                "bolt11": payout_miner.bolt11,
                "amount_msat": "10sat",
            }),
        )?;
        log::info!("payment result: {payout}");
        Ok(())
    }
}
