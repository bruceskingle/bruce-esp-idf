use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::ledc::{LedcChannel, LedcTimer, LowSpeed};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use esp_idf_hal::units::*;
use esp_idf_hal::{gpio::PinDriver, ledc::LedcDriver};

use bruce_embedded_std::Status;

#[cfg(feature = "mono-led")]
pub mod mono_led;
#[cfg(feature = "rgb-led")]
pub mod rgb_led;
#[cfg(feature = "simple-led")]
pub mod simple_led;

pub trait LedManager {
    fn set_on(&self) -> anyhow::Result<()>;
    fn set_off(&self) -> anyhow::Result<()>;
    fn set_status(&self, status: &Status) -> anyhow::Result<()>;
}
