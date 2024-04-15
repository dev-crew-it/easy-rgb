//! Create macros
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>

macro_rules! howmuchfees {
    ($cln:expr) => {{
        // Estimate the fee
        let fees: Value = $cln
            .state
            .call("estimatefees", json::json!({}))
            .map_err(|err| error!("{err}"))?;
        log::trace!("estimated fee: {fees}");
        let minimum = fees
            .get("feerate_floor")
            .ok_or(error!("not able to find the feerate_floor in: `{fees}`"))?;
        minimum.as_i64().unwrap_or_default()
    }};
}
pub(super) use howmuchfees;
