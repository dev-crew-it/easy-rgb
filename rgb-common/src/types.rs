//! RGB types
use std::collections::BTreeMap;

use commit_verify::mpc::MerkleBlock;
use serde::{Deserialize, Serialize};

use crate::bitcoin::Txid;
use crate::core::{Anchor, TransitionBundle};
use crate::std::contract::ContractId;

/// RGB Send asset data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RGBSendAssetData {
    pub asset_id: String,
    pub amount: u64,
    pub blinded_utxo: String,
    pub donation: bool,
}

/// RGB channel info
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RgbInfo {
    /// Channel_id
    pub channel_id: String,
    /// Channel contract ID
    pub contract_id: ContractId,
    /// Channel RGB local amount
    pub local_rgb_amount: u64,
    /// Channel RGB remote amount
    pub remote_rgb_amount: u64,
}

/// RGB payment info
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RgbPaymentInfo {
    /// RGB contract ID
    pub contract_id: ContractId,
    /// RGB payment amount
    pub amount: u64,
    /// RGB local amount
    pub local_rgb_amount: u64,
    /// RGB remote amount
    pub remote_rgb_amount: u64,
    /// Whether the RGB amount in route should be overridden
    pub override_route_amount: bool,
}

/// RGB transfer info
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransferInfo {
    /// Transfer anchor
    pub anchor: Anchor<MerkleBlock>,
    /// Transfer bundles
    pub bundles: BTreeMap<ContractId, TransitionBundle>,
    /// Transfer contract ID
    pub contract_id: ContractId,
    /// Transfer RGB amount
    pub rgb_amount: u64,
}

/// A reference to a transaction output.
///
/// Differs from bitcoin::blockdata::transaction::OutPoint as the index is a u16 instead of u32
/// due to LN's restrictions on index values. Should reduce (possibly) unsafe conversions this way.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct OutPoint {
    /// The referenced transaction's txid.
    pub txid: Txid,
    /// The index of the referenced output in its transaction's vout.
    pub index: u16,
}
