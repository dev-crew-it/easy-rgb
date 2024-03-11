//! RGB Wallet mock
use std::sync::{Arc, Mutex};

use crate::bitcoin::bip32::ExtendedPrivKey;
use crate::bitcoin::bip32::ExtendedPubKey;
use crate::bitcoin::psbt::PartiallySignedTransaction;
use crate::bitcoin::secp256k1::Secp256k1;
use crate::bitcoin::Network;
use crate::lib::wallet::{DatabaseType, Online, Wallet as RgbWallet, WalletData};
use crate::lib::BitcoinNetwork;
use crate::types::RgbInfo;

pub struct Wallet {
    wallet: Arc<Mutex<RgbWallet>>,
    online_wallet: Option<Online>,
}

impl Wallet {
    pub fn new(
        network: &BitcoinNetwork,
        xprv: ExtendedPrivKey,
        path: &str,
    ) -> anyhow::Result<Self> {
        // with rgb library tere is a new function for calculate the account key
        let account_privkey = Self::derive_account_xprv_from_mnemonic(network.clone(), xprv)?;
        let account_xpub = ExtendedPubKey::from_priv(&Secp256k1::new(), &account_privkey);
        let mut wallet = RgbWallet::new(WalletData {
            data_dir: path.to_owned(),
            bitcoin_network: network.clone(),
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
        let mut online_info = None;
        if !url.is_empty() {
            online_info = Some(wallet.go_online(false, url.to_owned())?);
        }
        Ok(Self {
            wallet: Arc::new(Mutex::new(wallet)),
            online_wallet: online_info,
        })
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

        let coin_type = Self::get_coin_type(bitcoin_network);
        let account_derivation_path = vec![
            ChildNumber::from_hardened_idx(PURPOSE as u32).unwrap(),
            ChildNumber::from_hardened_idx(coin_type).unwrap(),
            ChildNumber::from_hardened_idx(ACCOUNT as u32).unwrap(),
        ];
        Ok(master_xprv.derive_priv(&Secp256k1::new(), &account_derivation_path)?)
    }

    /// Given A PSBT we add the rgb information into it
    pub fn add_rgb_ouput(
        &self,
        psbt: &mut PartiallySignedTransaction,
        commitment_info: &RgbInfo,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
