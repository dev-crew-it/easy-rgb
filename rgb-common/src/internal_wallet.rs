//! RGB Wallet mock
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use amplify::map;
use bdk;
use bdk::blockchain::ElectrumBlockchain;
use bdk::electrum_client::Client;
use bdk::SyncOptions;
use bp::seals::txout::CloseMethod;
use rgb_lib::wallet::SendResult;
use strict_encoding::{FieldName, TypeName};

use crate::bitcoin::bip32::ChildNumber;
use crate::bitcoin::bip32::ExtendedPrivKey;
use crate::bitcoin::bip32::ExtendedPubKey;
use crate::bitcoin::secp256k1::hashes::Hash;
use crate::bitcoin::secp256k1::Secp256k1;
use crate::bitcoin::Network;
use crate::bitcoin::{ScriptBuf, TxOut};
use crate::bitcoin30::psbt::PartiallySignedTransaction as RgbPsbt;
use crate::core::contract::Operation;
use crate::core::SecretSeal;
use crate::json;
use crate::lib::utils::load_rgb_runtime;
use crate::lib::wallet::RecipientData;
use crate::lib::wallet::{AssetNIA, ReceiveData, Recipient};
use crate::lib::wallet::{DatabaseType, Online, Wallet as RgbWallet, WalletData};
use crate::lib::BitcoinNetwork;
use crate::rgb::persistence::Inventory;
use crate::rgb::psbt::opret::OutputOpret;
use crate::rgb::psbt::{PsbtDbc, RgbExt, RgbInExt};
use crate::rgb_manager::STATIC_BLINDING;
use crate::std::containers::BuilderSeal;
use crate::std::contract::GraphSeal;
use crate::std::interface::TypedState;
use crate::types;
use crate::types::RgbInfo;

pub struct Wallet {
    path: String,
    pub network: BitcoinNetwork,
    pub wallet: Arc<Mutex<RgbWallet>>,
    pub online_wallet: Option<Online>,
    /// RGB proxy endpoint
    proxy_endpoint: String,
    /// bdk wallet with the private key of cln
    /// this is dangerus to keep because cln should sign
    /// our  stuff too, but currently we use this approach
    /// in the future we should find a solution.
    ///
    /// FIXME: please fix this
    master_wallet: bdk::Wallet<bdk::database::MemoryDatabase>,
}

impl Wallet {
    pub fn new(network: &Network, xprv: ExtendedPrivKey, path: &str) -> anyhow::Result<Self> {
        let btc_network = BitcoinNetwork::from_str(&network.to_string())?;
        let master_wallet = bdk::Wallet::new(
            bdk::template::Bip84(xprv, bdk::KeychainKind::External),
            Some(bdk::template::Bip84(xprv, bdk::KeychainKind::Internal)),
            bdk::bitcoin::Network::Testnet,
            bdk::database::MemoryDatabase::default(),
        )?;
        let (url, proxy) = match network {
            Network::Bitcoin => (None, None),
            Network::Testnet => (
                Some("ssl://electrum.iriswallet.com:50013"),
                Some("rpcs://proxy.iriswallet.com/0.2/json-rpc"),
            ),
            Network::Signet => (None, None),
            Network::Regtest => (
                Some("127.0.0.1:50001"),
                Some("rpc://127.0.0.1:3000/json-rpc"),
            ),
            _ => anyhow::bail!("Network `{network}` not supported"),
        };

        let (Some(url), Some(proxy)) = (url, proxy) else {
            anyhow::bail!("Network `{network}` not supported by the plugin");
        };

        let blockchain = ElectrumBlockchain::from(Client::new(url)?);
        master_wallet.sync(&blockchain, SyncOptions::default())?;
        // with rgb library tere is a new function for calculate the account key
        let account_privkey = Self::derive_account_xprv_from_mnemonic(btc_network, &xprv)?;
        let account_xpub = ExtendedPubKey::from_priv(&Secp256k1::new(), &account_privkey);
        let mut wallet = RgbWallet::new(WalletData {
            data_dir: path.to_owned(),
            bitcoin_network: btc_network,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 11,
            pubkey: account_xpub.to_string().to_owned(),
            mnemonic: None,
            vanilla_keychain: None,
        })?;

        let mut online_info = None;
        if !url.is_empty() {
            online_info = Some(wallet.go_online(false, url.to_owned())?);
        }
        Ok(Self {
            path: path.to_owned(),
            proxy_endpoint: proxy.to_owned(),
            wallet: Arc::new(Mutex::new(wallet)),
            network: BitcoinNetwork::from_str(&network.to_string())?,
            online_wallet: online_info,
            master_wallet,
        })
    }

    pub fn path(&self) -> PathBuf {
        self.wallet.lock().unwrap().get_wallet_dir()
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

    #[cfg(debug_assertions)]
    pub fn issue_asset_nia(
        &self,
        ticker: String,
        name: String,
        precision: u8,
        amounts: Vec<u64>,
    ) -> anyhow::Result<AssetNIA> {
        let Some(ref online) = self.online_wallet else {
            anyhow::bail!("Wallet is not online");
        };
        let assert = self.wallet.lock().unwrap().issue_asset_nia(
            online.clone(),
            ticker,
            name,
            precision,
            amounts,
        )?;
        Ok(assert)
    }

    pub fn new_addr(&self) -> anyhow::Result<String> {
        let addr = self.wallet.lock().unwrap().get_address()?;
        Ok(addr)
    }

    pub fn send_asset<F>(
        &self,
        data: &types::RGBSendAssetData,
        feerate: f32,
        minconf: u8,
        sign_psbt: F,
    ) -> anyhow::Result<SendResult>
    where
        F: FnOnce(&mut bitcoin::psbt::PartiallySignedTransaction) -> anyhow::Result<()>,
    {
        let online = self
            .online_wallet
            .clone()
            .ok_or(anyhow::anyhow!("Wallet is offline"))?;
        let wallet = self.wallet.lock().unwrap();
        let seal = SecretSeal::from_str(&data.blinded_utxo)?;
        let recipient_map = map! {
            data.asset_id.clone() => vec![Recipient {
                recipient_data: RecipientData::BlindedUTXO(seal),
                amount: data.amount,
                transport_endpoints: vec![self.proxy_endpoint.clone()],
            }]
        };

        let psbt = wallet.send_begin(
            online.clone(),
            recipient_map,
            data.donation,
            feerate,
            minconf,
        )?;
        let mut psbt = bitcoin::psbt::PartiallySignedTransaction::from_str(&psbt)?;
        sign_psbt(&mut psbt)?;
        let sendresult = wallet.send_end(online, psbt.to_string())?;
        Ok(sendresult)
    }

    pub fn new_blind_receive(
        &self,
        asset_id: Option<String>,
        min_confirmations: u8,
    ) -> anyhow::Result<ReceiveData> {
        let blind_receive = self.wallet.lock().unwrap().blind_receive(
            asset_id,
            None,
            None,
            vec![self.proxy_endpoint.clone()],
            min_confirmations,
        )?;
        Ok(blind_receive)
    }

    /// Preallocate the UTXO assets on chain for RGB.
    pub fn create_utxos<F>(&self, fee_rate: f32, sign_psbt: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut bitcoin::psbt::PartiallySignedTransaction) -> anyhow::Result<()>,
    {
        // FIXME: Mh, I should know why for this?
        const UTXO_SIZE_SAT: u32 = 32000;

        let wallet_online = self
            .online_wallet
            .clone()
            .ok_or(anyhow::anyhow!("Wallet not online"))?;

        let wallet = self.wallet.lock().unwrap();

        let unsigned_psbt = wallet.create_utxos_begin(
            wallet_online.clone(),
            false,
            Some(1),
            Some(UTXO_SIZE_SAT),
            fee_rate,
        )?;

        let mut unsigned_psbt =
            bitcoin::psbt::PartiallySignedTransaction::from_str(&unsigned_psbt)?;

        sign_psbt(&mut unsigned_psbt)?;
        wallet.create_utxos_end(wallet_online, unsigned_psbt.to_string())?;

        Ok(())
    }

    pub fn get_btc_balance(&self) -> anyhow::Result<json::Value> {
        let wallet = self.wallet.lock().unwrap();
        let balance = wallet.get_btc_balance(
            self.online_wallet
                .clone()
                .ok_or(anyhow::anyhow!("wallet is not online"))?,
        )?;
        let cln = self.master_wallet.get_balance()?;
        Ok(json::json!({
            "cln": cln,
            "rgb": balance,
        }))
    }

    pub fn sing_with_master_key(
        &self,
        psbt: &mut bitcoin::psbt::PartiallySignedTransaction,
    ) -> anyhow::Result<()> {
        let sign_options = bdk::SignOptions {
            trust_witness_utxo: true,
            ..Default::default()
        };

        if !self.master_wallet.sign(psbt, sign_options)? {
            anyhow::bail!("bdk is not able to sing with master key the psbt `{psbt}`");
        }
        Ok(())
    }

    pub fn rgb_funding_complete(
        &self,
        recipient_map: HashMap<String, Vec<Recipient>>,
        fee_rate: f32,
        min_conf: u8,
    ) -> anyhow::Result<bitcoin::psbt::PartiallySignedTransaction> {
        let wallet = self.wallet.lock().unwrap();
        let online = self
            .online_wallet
            .as_ref()
            .ok_or(anyhow::anyhow!("Wallet not online"))?;
        let unsigned_psbt =
            wallet.send_begin(online.clone(), recipient_map, true, fee_rate, min_conf)?;
        let mut psbt = bitcoin::psbt::PartiallySignedTransaction::from_str(&unsigned_psbt)?;
        self.sing_with_master_key(&mut psbt)?;
        Ok(psbt)
    }

    /// Given A PSBT we add the rgb information into it
    pub fn colored_funding(
        &self,
        psbt: &mut bitcoin::psbt::PartiallySignedTransaction,
        funding_outpoint: types::OutPoint,
        commitment_info: &RgbInfo,
        holder_vout: u32,
    ) -> anyhow::Result<()> {
        use bp::Outpoint;

        let mut tx = psbt.clone().extract_tx();
        tx.output.push(TxOut {
            value: 0,
            script_pubkey: ScriptBuf::new_op_return(&[1]),
        });
        let mut rgb_psbt = RgbPsbt::from_unsigned_tx(tx.clone())?;
        let mut runtime = load_rgb_runtime(self.path.clone().into(), self.network)?;

        let holder_vout_amount = commitment_info.local_rgb_amount;
        let counterparty_vout_amount = commitment_info.remote_rgb_amount;
        let counterparty_vout = holder_vout ^ 1;

        let mut beneficiaries = vec![];
        let mut asset_transition_builder = runtime
            .runtime
            .transition_builder(
                commitment_info.contract_id,
                TypeName::try_from("RGB20").unwrap(),
                None::<&str>,
            )
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let assignment_id = asset_transition_builder
            .assignments_type(&FieldName::from("beneficiary"))
            .ok_or(anyhow::anyhow!(
                "`None` returned during `asset_transition_builder.assignments_type`"
            ))?;

        if holder_vout_amount > 0 {
            let holder_seal = BuilderSeal::Revealed(GraphSeal::with_vout(
                CloseMethod::OpretFirst,
                holder_vout as u32,
                STATIC_BLINDING,
            ));
            beneficiaries.push(holder_seal);
            asset_transition_builder = asset_transition_builder.add_raw_state(
                assignment_id,
                holder_seal,
                TypedState::Amount(holder_vout_amount),
            )?;
        }

        if counterparty_vout_amount > 0 {
            let counterparty_seal = BuilderSeal::Revealed(GraphSeal::with_vout(
                CloseMethod::OpretFirst,
                counterparty_vout as u32,
                STATIC_BLINDING,
            ));
            beneficiaries.push(counterparty_seal);
            asset_transition_builder = asset_transition_builder.add_raw_state(
                assignment_id,
                counterparty_seal,
                TypedState::Amount(counterparty_vout_amount),
            )?;
        }

        let prev_outputs = rgb_psbt
            .unsigned_tx
            .input
            .iter()
            .map(|txin| txin.previous_output)
            .map(|outpoint| Outpoint::new(outpoint.txid.to_byte_array().into(), outpoint.vout))
            .collect::<Vec<_>>();
        for (opout, _state) in runtime
            .runtime
            .state_for_outpoints(commitment_info.contract_id, prev_outputs.iter().copied())
            .map_err(|err| anyhow::anyhow!("{err}"))?
        {
            asset_transition_builder = asset_transition_builder.add_input(opout)?;
        }
        let transition =
            asset_transition_builder.complete_transition(commitment_info.contract_id)?;

        let inputs = [Outpoint::new(
            bp::Txid::from_str(&funding_outpoint.txid.to_string()).unwrap(),
            funding_outpoint.index as u32,
        )];
        for (input, txin) in rgb_psbt.inputs.iter_mut().zip(&rgb_psbt.unsigned_tx.input) {
            let prevout = txin.previous_output;
            let outpoint = Outpoint::new(prevout.txid.to_byte_array().into(), prevout.vout);
            if inputs.contains(&outpoint) {
                input.set_rgb_consumer(commitment_info.contract_id, transition.id())?;
            }
        }
        rgb_psbt.push_rgb_transition(transition)?;
        // FIXME: we can comment the code below?
        // let bundles = rgb_psbt.rgb_bundles().expect("able to get bundles");
        let (opreturn_index, _) = rgb_psbt
            .unsigned_tx
            .output
            .iter()
            .enumerate()
            .find(|(_, o)| o.script_pubkey.is_op_return())
            .unwrap();
        let (_, opreturn_output) = rgb_psbt
            .outputs
            .iter_mut()
            .enumerate()
            .find(|(i, _)| i == &opreturn_index)
            .unwrap();
        opreturn_output.set_opret_host()?;
        rgb_psbt.rgb_bundle_to_lnpbp4().expect("ok");
        let _ = rgb_psbt.dbc_conclude(CloseMethod::OpretFirst)?;

        *psbt = bitcoin::psbt::PartiallySignedTransaction::from_str(&rgb_psbt.to_string()).unwrap();

        Ok(())
    }
}
