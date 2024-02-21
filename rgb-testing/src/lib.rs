//! Core Lightning Integration testing
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>

mod utils;

#[cfg(test)]
mod tests {

    use std::sync::Once;

    use clightning_testing::cln;
    use serde_json as json;

    #[allow(unused_imports)]
    use super::utils::*;

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
        let pwd = std::env!("PWD");
        let cln = cln::Node::with_params(
            &format!("--developer --experimental-splicing --plugin={pwd}/target/debug/rgb-cln"),
            "regtest",
        )
        .await?;
        let result = cln.rpc().call::<json::Value, json::Value>("getinfo", json::json!({}));
        assert!(result.is_ok(), "{:?}", result);
        Ok(())

    }
}
