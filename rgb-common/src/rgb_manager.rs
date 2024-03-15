//! RGB Manager
use std::str::FromStr;
use std::sync::Arc;

use bitcoin::bip32::ExtendedPrivKey;
use rgb_lib::wallet::Balance;
use rgb_lib::wallet::Recipient;
use rgb_lib::wallet::RecipientData;
use rgb_lib::ScriptBuf;
use rgbwallet::bitcoin;
use rgbwallet::bitcoin::psbt::PartiallySignedTransaction;

use crate::internal_wallet::Wallet;
use crate::lib::BitcoinNetwork;
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
        let bitcoin_network = BitcoinNetwork::from_str(network)?;
        let wallet = Wallet::new(&bitcoin_network, *master_xprv, root_dir)?;
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

    pub fn assert_balance(&self, asset_id: String) -> anyhow::Result<Balance> {
        let balance = self
            .wallet
            .wallet
            .lock()
            .unwrap()
            .get_asset_balance(asset_id)?;
        Ok(balance)
    }

    pub fn add_rgb_info(&self, info: &RgbInfo, pending: bool) -> anyhow::Result<()> {
        self.storage.write_rgb_info(&info.channel_id, pending, info)
    }

    /// Modify the funding transaction before sign it with the node signer.
    pub fn handle_onfunding_tx(
        &self,
        tx: bitcoin::Transaction,
        txid: bitcoin::Txid,
        psbt: &mut PartiallySignedTransaction,
        channel_id: String,
    ) -> anyhow::Result<bitcoin::Transaction> {
        debug_assert!(tx.txid() == txid);
        debug_assert!(psbt.clone().extract_tx().txid() == txid);
        // allow looup by channel and returnt the rgb info
        if self.storage.is_channel_rgb(&channel_id, false)? {
            // Step 1: get the rgb info https://github.com/RGB-Tools/rgb-lightning-node/blob/master/src/ldk.rs#L328
            let mut info = self.storage.get_rgb_channel_info_pending(&channel_id)?;
            info.channel_id = channel_id;
            // Step 2: Modify the psbt and start sending with the rgb wallet
            let funding_outpoint = types::OutPoint {
                txid,
                index: 0, /* FIXME: cln should tell this info to us */
            };
            self.prepare_rgb_tx(&info, funding_outpoint, &tx, psbt)?;

            // Step 3: Make the cosignemtn and post it somewhere
            let consignment_path = self
                .wallet()
                .path()
                .join("transfers")
                .join(txid.to_string().clone())
                .join(info.contract_id.to_string())
                .join("consignment_out");
            self.consignment_proxy().post_consignment(
                &consignment_path,
                txid.to_string(),
                txid.to_string(),
                Some(0),
            )?;
            return Ok(tx);
        }
        Ok(tx)
    }

    // Missing parameters: amount_sat of the funding tx and
    // the script one
    //
    // Maybe it is possible extract from the tx if we know the information from
    // the tx, but not sure if this is a good usage of cln hooks?
    fn prepare_rgb_tx(
        &self,
        info: &types::RgbInfo,
        funding_outpoint: types::OutPoint,
        tx: &bitcoin::Transaction,
        psb: &mut PartiallySignedTransaction,
    ) -> anyhow::Result<()> {
        // TODO: this is still needed?
        let recipient_map = amplify::map! {
            info.contract_id.to_string() => vec![Recipient {
                recipient_data: RecipientData::WitnessData {
                    script_buf: ScriptBuf::new(), // TODO: get this from the transaction
                    amount_sat: 0, // TODO get this from the transaction
                    blinding: Some(STATIC_BLINDING),
                },
                amount: info.local_rgb_amount,
                transport_endpoints: vec![self.consignment_proxy.url.clone()]
            }]
        };
        // FIXME: find the position of the vout;
        self.wallet
            .colored_funding(psb, funding_outpoint, info, 0)?;
        Ok(())
    }
}
