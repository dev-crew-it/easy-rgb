//! RGB Manager
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use bitcoin::bip32::ChildNumber;
use bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::secp256k1;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use rgbwallet::bitcoin;

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

fn get_coin_type(bitcoin_network: BitcoinNetwork) -> u32 {
    u32::from(bitcoin_network != BitcoinNetwork::Mainnet)
}

fn derive_account_xprv_from_mnemonic(
    bitcoin_network: BitcoinNetwork,
    master_xprv: &ExtendedPrivKey,
) -> anyhow::Result<ExtendedPrivKey> {
    const PURPOSE: u8 = 84;
    const ACCOUNT: u8 = 0;

    let coin_type = get_coin_type(bitcoin_network);
    let account_derivation_path = vec![
        ChildNumber::from_hardened_idx(PURPOSE as u32).unwrap(),
        ChildNumber::from_hardened_idx(coin_type).unwrap(),
        ChildNumber::from_hardened_idx(ACCOUNT as u32).unwrap(),
    ];
    Ok(master_xprv.derive_priv(&Secp256k1::new(), &account_derivation_path)?)
}

impl RGBManager {
    pub fn init(
        root_dir: &str,
        master_xprv: &ExtendedPrivKey,
        network: &str,
    ) -> anyhow::Result<Self> {
        let client = proxy::Client::new(network)?;

        let bitcoin_network = BitcoinNetwork::from_str(network)?;
        // with rgb library tere is a new function for calculate the account key
        let account_privkey = derive_account_xprv_from_mnemonic(bitcoin_network, master_xprv)?;
        let account_xpub = ExtendedPubKey::from_priv(&Secp256k1::new(), &account_privkey);
        let mut wallet = Wallet::new(WalletData {
            data_dir: root_dir.to_owned(),
            bitcoin_network,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 11,
            pubkey: account_xpub.to_string().to_owned(),
            mnemonic: None,
            vanilla_keychain: None,
        })?;
        let network = Network::from_str(network)?;
        let url = match network {
            Network::Bitcoin => "https://mempool.space/api",
            Network::Testnet => "https://mempool.space/testnet/api",
            Network::Signet => "https://mempool.space/signet/api",
            Network::Regtest => "",
            _ => anyhow::bail!("Network `{network}` not supported"),
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

    /// Modify the funding transaction before sign it with the node signer.
    pub fn handle_onfunding_tx(
        &self,
        tx: bitcoin::Transaction,
        txid: bitcoin::Txid,
        channel_id: String,
    ) -> anyhow::Result<bitcoin::Transaction> {
        Ok(tx)
    }
}
