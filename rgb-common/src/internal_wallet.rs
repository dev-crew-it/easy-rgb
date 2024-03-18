//! RGB Wallet mock
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bp::seals::txout::CloseMethod;
use strict_encoding::{FieldName, TypeName};

use crate::bitcoin::bip32::ChildNumber;
use crate::bitcoin::bip32::ExtendedPrivKey;
use crate::bitcoin::bip32::ExtendedPubKey;
use crate::bitcoin::psbt::PartiallySignedTransaction;
use crate::bitcoin::secp256k1::hashes::Hash;
use crate::bitcoin::secp256k1::Secp256k1;
use crate::bitcoin::Network;
use crate::bitcoin::{ScriptBuf, TxOut};
use crate::bitcoin30::psbt::PartiallySignedTransaction as RgbPsbt;
use crate::core::contract::Operation;
use crate::lib::utils::load_rgb_runtime;
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
}

impl Wallet {
    pub fn new(network: &Network, xprv: ExtendedPrivKey, path: &str) -> anyhow::Result<Self> {
        let btc_network = BitcoinNetwork::from_str(&network.to_string())?;
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
            path: path.to_owned(),
            wallet: Arc::new(Mutex::new(wallet)),
            network: BitcoinNetwork::from_str(&network.to_string())?,
            online_wallet: online_info,
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

    /// Given A PSBT we add the rgb information into it
    pub fn colored_funding(
        &self,
        psbt: &mut PartiallySignedTransaction,
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

        *psbt = PartiallySignedTransaction::from_str(&rgb_psbt.to_string()).unwrap();

        Ok(())
    }
}
