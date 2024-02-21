//! RGB Manager
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use bitcoin::Network;

use crate::lib::wallet::{DatabaseType, Wallet, WalletData};
use crate::lib::BitcoinNetwork;
use crate::proxy;

pub struct RGBManager {
    proxy_client: Arc<proxy::Client>,
    wallet: Arc<Mutex<Wallet>>,
}

impl std::fmt::Debug for RGBManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "RGB manager struct {{ .. }}")
    }
}

impl RGBManager {
    pub fn init(root_dir: &str, pubkey: &str, network: &str) -> anyhow::Result<Self> {
        let client = proxy::Client::new(network)?;
        let mut wallet = Wallet::new(WalletData {
            data_dir: root_dir.to_owned(),
            bitcoin_network: BitcoinNetwork::from_str(network)?,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 11,
            pubkey: pubkey.to_owned(),
            mnemonic: None,
            vanilla_keychain: None,
        })?;
        let network = Network::from_str(network)?;
        let url = match network {
            Network::Bitcoin => "https://mempool.space/api",
            Network::Testnet => "https://mempool.space/testnet/api",
            Network::Signet => "https://mempool.space/signet/api",
            Network::Regtest => "",
        };
        if !url.is_empty() {
            let _ = wallet.go_online(false, url.to_owned())?;
        }
        // FIXME: setting up the correct proxy client URL
        Ok(Self {
            proxy_client: Arc::new(client),
            wallet: Arc::new(Mutex::new(wallet)),
        })
    }

    pub fn wallet(&self) -> Arc<Mutex<Wallet>> {
        self.wallet.clone()
    }

    pub fn proxy_client(&self) -> Arc<proxy::Client> {
        self.proxy_client.clone()
    }
}
