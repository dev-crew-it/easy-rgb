//! A module to provide RGB functionality
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use rgb_core::ContractId;

use crate::ldk;

use ldk::ln::PaymentHash;

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
}

/// Get RgbPaymentInfo file
pub fn get_rgb_payment_info(payment_hash: &PaymentHash, ldk_data_dir: &PathBuf) -> RgbPaymentInfo {
    let rgb_payment_info_path = ldk_data_dir.join(hex::encode(payment_hash.0));
    parse_rgb_payment_info(&rgb_payment_info_path)
}

/// Parse RgbPaymentInfo
pub fn parse_rgb_payment_info(rgb_payment_info_path: &PathBuf) -> RgbPaymentInfo {
    let serialized_info =
        fs::read_to_string(&rgb_payment_info_path).expect("valid rgb payment info");
    serde_json::from_str(&serialized_info).expect("valid rgb info file")
}

/// Get RgbInfo file
pub fn get_rgb_channel_info(
    channel_id: &[u8; 32],
    ldk_data_dir: &PathBuf,
) -> anyhow::Result<(RgbInfo, PathBuf)> {
    let info_file_path = ldk_data_dir.join(hex::encode(channel_id));
    let serialized_info = fs::read_to_string(&info_file_path)?;
    let info: RgbInfo = serde_json::from_str(&serialized_info)?;
    Ok((info, info_file_path))
}

/// Write RgbInfo file
pub fn write_rgb_channel_info(path: &PathBuf, rgb_info: &RgbInfo) {
    let serialized_info = serde_json::to_string(&rgb_info).expect("valid rgb info");
    fs::write(path, serialized_info).expect("able to write")
}

/// Rename RgbInfo file to channel_id
pub(crate) fn rename_rgbinfo_file(
    channel_id: &[u8; 32],
    temporary_channel_id: &[u8; 32],
    ldk_data_dir: &PathBuf,
) {
    let temporary_channel_id_path = ldk_data_dir.join(hex::encode(temporary_channel_id));
    let channel_id_path = ldk_data_dir.join(hex::encode(channel_id));
    fs::rename(temporary_channel_id_path, channel_id_path).expect("rename ok");
}

/// Update RGB channel amount
pub(crate) fn update_rgb_channel_amount(
    channel_id: &[u8; 32],
    rgb_offered_htlc: u64,
    rgb_received_htlc: u64,
    ldk_data_dir: &PathBuf,
) -> anyhow::Result<()> {
    let (mut rgb_info, info_file_path) = get_rgb_channel_info(channel_id, ldk_data_dir)?;

    if rgb_offered_htlc > rgb_received_htlc {
        let spent = rgb_offered_htlc - rgb_received_htlc;
        rgb_info.local_rgb_amount -= spent;
        rgb_info.remote_rgb_amount += spent;
    } else {
        let received = rgb_received_htlc - rgb_offered_htlc;
        rgb_info.local_rgb_amount += received;
        rgb_info.remote_rgb_amount -= received;
    }

    write_rgb_channel_info(&info_file_path, &rgb_info);
    Ok(())
}
