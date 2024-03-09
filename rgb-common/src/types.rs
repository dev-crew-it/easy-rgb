//! RGB types
use std::collections::BTreeMap;

use commit_verify::mpc::MerkleBlock;
use serde::{Deserialize, Serialize};

use crate::core::{Anchor, TransitionBundle};
use crate::std::contract::ContractId;

/// RGB channel info
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RgbInfo {
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
