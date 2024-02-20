//! RGB Manager
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use crate::lib::wallet::{DatabaseType, Wallet, WalletData};
use crate::lib::BitcoinNetwork;
use crate::proxy;

pub struct RGBManager {
    proxy_client: Arc<crate::BlockingClient>,
    wallet: Arc<Mutex<Wallet>>,
}

impl RGBManager {
    pub fn init(root_dir: &str, pubkey: &str, network: &str) -> anyhow::Result<Self> {
        let client = proxy::get_blocking_client();
        let wallet = Wallet::new(WalletData {
            data_dir: root_dir.to_owned(),
            bitcoin_network: BitcoinNetwork::from_str(network)?,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 11,
            pubkey: pubkey.to_owned(),
            mnemonic: None,
            vanilla_keychain: None,
        })?;
        // FIXME: go online
        // FIXME: setting up the correct proxy client URL
        Ok(Self {
            proxy_client: Arc::new(client),
            wallet: Arc::new(Mutex::new(wallet)),
        })
    }

    pub fn wallet(&self) -> Arc<Mutex<Wallet>> {
        self.wallet.clone()
    }

    pub fn proxy_client(&self) -> Arc<crate::BlockingClient> {
        self.proxy_client.clone()
    }
}
