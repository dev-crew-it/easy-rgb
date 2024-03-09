//! RGB Storage interface
use std::collections::HashMap;

use crate::types::RgbInfo;

/// A common interface for an RGB Storage
pub trait RGBStorage {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    fn get_rgb_channel_info(&self, channel_id: &str) -> anyhow::Result<RgbInfo>;

    fn get_rgb_channel_info_pending(&self, channel_id: &str) -> anyhow::Result<RgbInfo>;

    fn is_channel_rgb(&self, channel_id: &str, is_pending: bool) -> anyhow::Result<bool>;
}

pub struct InMemoryStorage {
    inner: HashMap<String, String>,
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
            inner: HashMap::new(),
        })
    }

    fn get_rgb_channel_info(&self, channel_id: &str) -> anyhow::Result<RgbInfo> {
        let key = self.derive_channel_db_key(channel_id, false)?;
        let value = self
            .inner
            .get(&key)
            .ok_or(anyhow::anyhow!("rgb channel with key `{key}` is not found"))?;
        let info: RgbInfo = serde_json::from_str(&value)?;
        Ok(info)
    }

    fn get_rgb_channel_info_pending(&self, channel_id: &str) -> anyhow::Result<RgbInfo> {
        let key = self.derive_channel_db_key(channel_id, true)?;
        let value = self
            .inner
            .get(&key)
            .ok_or(anyhow::anyhow!("rgb channel with key `{key}` is not found"))?;
        let info: RgbInfo = serde_json::from_str(&value)?;
        Ok(info)
    }

    fn is_channel_rgb(&self, channel_id: &str, is_pending: bool) -> anyhow::Result<bool> {
        let key = self.derive_channel_db_key(channel_id, is_pending)?;
        Ok(self.inner.contains_key(&key))
    }
}
