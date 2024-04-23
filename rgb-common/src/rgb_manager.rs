//! RGB Manager
use std::str::FromStr;
use std::sync::Arc;

use bitcoin::bip32::ExtendedPrivKey;
use bitcoin::Network;
use rgb_lib::wallet::AssetNIA;
use rgb_lib::wallet::Balance;
use rgb_lib::wallet::Recipient;
use rgb_lib::wallet::RecipientData;
use rgbwallet::bitcoin;

use crate::internal_wallet::Wallet;
use crate::json;
use crate::proxy;
use crate::rgb_storage as store;
use crate::rgb_storage::RGBStorage;
use crate::types;
use crate::types::RgbInfo;

/// Static blinding costant (will be removed in the future)
/// See https://github.com/RGB-Tools/rust-lightning/blob/80497c4086beea490b56e5b8413b7f6d86f2c042/lightning/src/rgb_utils/mod.rs#L53
pub const STATIC_BLINDING: u64 = 777;

pub struct RGBManager {
    consignment_proxy: Arc<proxy::ConsignmentClient>,
    storage: Box<dyn store::RGBStorage>,
    wallet: Arc<Wallet>,
    #[allow(dead_code)]
    path: String,
}

impl std::fmt::Debug for RGBManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "RGB manager struct {{ .. }}")
    }
}

impl RGBManager {
    pub fn init(
        root_dir: &str,
        master_xprv: &ExtendedPrivKey,
        network: &str,
    ) -> anyhow::Result<Self> {
        let storage = Box::new(store::InMemoryStorage::new()?);
        let client = proxy::ConsignmentClient::new(network)?;
        let network = Network::from_str(network)?;

        let wallet = Wallet::new(&network, *master_xprv, root_dir)?;
        // FIXME: setting up the correct proxy client URL
        Ok(Self {
            consignment_proxy: Arc::new(client),
            wallet: Arc::new(wallet),
            path: root_dir.to_owned(),
            storage,
        })
    }

    pub fn wallet(&self) -> Arc<Wallet> {
        self.wallet.clone()
    }

    pub fn consignment_proxy(&self) -> Arc<proxy::ConsignmentClient> {
        self.consignment_proxy.clone()
    }

    #[cfg(debug_assertions)]
    pub fn issue_asset_nia(
        &self,
        ticker: String,
        name: String,
        precision: u8,
        amounts: Vec<u64>,
    ) -> anyhow::Result<AssetNIA> {
        self.wallet
            .issue_asset_nia(ticker, name, precision, amounts)
    }

    pub fn assert_balance(&self, asset_id: String) -> anyhow::Result<Balance> {
        let balance = self
            .wallet
            .wallet
            .lock()
            .unwrap()
            .get_asset_balance(asset_id)?;
        Ok(balance)
    }

    pub fn onchain_balance(&self) -> anyhow::Result<json::Value> {
        let balance = self.wallet.get_btc_balance()?;
        Ok(balance)
    }

    pub fn add_rgb_info(&self, info: &RgbInfo, pending: bool) -> anyhow::Result<()> {
        self.storage.write_rgb_info(&info.channel_id, pending, info)
    }

    /// Using the rgb proxy we to sync up if there is
    /// anything for us that we need to sync.
    ///
    /// An example is when we receive a payment, we have something
    /// the proxy that need to be signed, so this operation is doing
    /// exactly this.
    pub fn refresh(&self) -> anyhow::Result<()> {
        self.wallet().refresh()?;
        Ok(())
    }

    pub fn listen_for(&self, asset_id: &str) -> anyhow::Result<()> {
        self.storage.listen_for_asset(asset_id)
    }

    /// Modify the funding transaction before sign it with the node signer.
    ///
    /// Please note that this will also propagate rgb cosigment to the network.
    pub fn build_rgb_funding_transaction(
        &self,
        rgb_info: &RgbInfo,
        scriptpubkey: bitcoin::ScriptBuf,
        fee_rate: f32,
        min_conf: u8,
    ) -> anyhow::Result<bitcoin::psbt::PartiallySignedTransaction> {
        // Step 1: get the rgb info https://github.com/RGB-Tools/rgb-lightning-node/blob/master/src/ldk.rs#L328
        //let mut info = self.storage.get_rgb_channel_info_pending(&channel_id)?;
        //info.channel_id = channel_id;

        // Step 2: Modify the psbt and start sending with the rgb wallet
        let psbt = self.prepare_rgb_tx(&rgb_info, scriptpubkey, fee_rate, min_conf)?;
        // FIXME: avoid cloning
        let txid = psbt.clone().extract_tx().txid();
        // Step 3: Make the cosignemtn and post it somewhere
        let consignment_path = self
            .wallet()
            .path()
            .join("transfers")
            .join(txid.to_string().clone())
            .join(rgb_info.contract_id.to_string())
            .join("consignment_out");
        self.consignment_proxy().post_consignment(
            &consignment_path,
            txid.to_string(),
            txid.to_string(),
            Some(0),
        )?;
        return Ok(psbt);
    }

    // Missing parameters: amount_sat of the funding tx and
    // the script one
    //
    // Maybe it is possible extract from the tx if we know the information from
    // the tx, but not sure if this is a good usage of cln hooks?
    fn prepare_rgb_tx(
        &self,
        info: &types::RgbInfo,
        scriptpubkey: bitcoin::ScriptBuf,
        fee_rate: f32,
        min_conf: u8,
    ) -> anyhow::Result<bitcoin::psbt::PartiallySignedTransaction> {
        let recipient_map = amplify::map! {
            info.contract_id.to_string() => vec![Recipient {
                recipient_data: RecipientData::WitnessData {
                    script_buf: scriptpubkey,
                    amount_sat: info.remote_rgb_amount,
                    blinding: Some(STATIC_BLINDING),
                },
                amount: info.local_rgb_amount,
                transport_endpoints: vec![self.consignment_proxy.url.clone()]
            }]
        };

        let psbt = self
            .wallet
            .rgb_funding_complete(recipient_map, fee_rate, min_conf)?;
        Ok(psbt)
    }
}
