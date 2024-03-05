mod comm;
mod proxy;
mod rgb_manager;

use lightning as ldk;
use reqwest::blocking::Client as BlockingClient;

pub use anyhow;
pub use bitcoin;
// Re-exporting RGB dependencies under a single module.
pub use rgb;
pub use rgb::interface::rgb20 as asset20;
pub use rgb_core as core;
pub use rgb_lib as lib;
pub use rgb_manager::RGBManager;
pub use rgbstd as std;
pub use rgbwallet as wallet;
