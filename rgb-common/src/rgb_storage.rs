//! RGB Storage interface
use std::{cell::RefCell, collections::HashMap};

use crate::types::RgbInfo;

/// A common interface for an RGB Storage
pub trait RGBStorage {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    fn get_rgb_channel_info(&self, channel_id: &str) -> anyhow::Result<RgbInfo>;

    fn get_rgb_channel_info_pending(&self, channel_id: &str) -> anyhow::Result<RgbInfo>;

    fn is_channel_rgb(&self, channel_id: &str, is_pending: bool) -> anyhow::Result<bool>;

    fn write_rgb_info(
        &self,
        channel_id: &str,
        is_pending: bool,
        info: &RgbInfo,
    ) -> anyhow::Result<()>;
}

pub struct InMemoryStorage {
    inner: RefCell<HashMap<String, String>>,
}

impl InMemoryStorage {
    fn derive_channel_db_key(&self, channel_id: &str, is_pending: bool) -> anyhow::Result<String> {
        return if is_pending {
            Ok(format!("rgb/pending/channel/{channel_id}"))
        } else {
            Ok(format!("rgb/channel/{channel_id}"))
        };
    }
}

impl RGBStorage for InMemoryStorage {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            inner: RefCell::new(HashMap::new()),
        })
    }

    fn get_rgb_channel_info(&self, channel_id: &str) -> anyhow::Result<RgbInfo> {
        let key = self.derive_channel_db_key(channel_id, false)?;
        let map = self.inner.borrow();
        let value = map
            .get(&key)
            .ok_or(anyhow::anyhow!("rgb channel with key `{key}` is not found"))?;
        let info: RgbInfo = serde_json::from_str(&value)?;
        Ok(info)
    }

    fn get_rgb_channel_info_pending(&self, channel_id: &str) -> anyhow::Result<RgbInfo> {
        let key = self.derive_channel_db_key(channel_id, true)?;
        let map = self.inner.borrow();
        let value = map
            .get(&key)
            .ok_or(anyhow::anyhow!("rgb channel with key `{key}` is not found"))?;
        let info: RgbInfo = serde_json::from_str(&value)?;
        Ok(info)
    }

    fn is_channel_rgb(&self, channel_id: &str, is_pending: bool) -> anyhow::Result<bool> {
        let key = self.derive_channel_db_key(channel_id, is_pending)?;
        let map = self.inner.borrow();
        Ok(map.contains_key(&key))
    }

    fn write_rgb_info(
        &self,
        channel_id: &str,
        is_pending: bool,
        info: &RgbInfo,
    ) -> anyhow::Result<()> {
        let key = self.derive_channel_db_key(channel_id, is_pending)?;
        // FIXME: we need a lock before production
        let mut map = self.inner.borrow_mut();
        map.insert(key, serde_json::to_string(info)?);
        Ok(())
    }
}
