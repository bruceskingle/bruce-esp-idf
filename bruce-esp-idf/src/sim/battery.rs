//! The battery, for the simulator, which has none, so that Esp32Platform::battery is None, see the platform's
//! battery.rs, whose API it has.

use anyhow::anyhow;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatteryReading {
    pub volts: f32,
    pub percent: u8,
}

pub struct Battery;

impl Battery {
    pub fn read(&self) -> anyhow::Result<BatteryReading> {
        Err(anyhow!("A simulated board has no battery"))
    }
}
