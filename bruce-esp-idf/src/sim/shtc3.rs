//! The temperature and humidity sensor, for the simulator, which has none: every reading is an error, as the simulator
//! never makes up a reading, see the platform's shtc3.rs, whose API it has.

use anyhow::anyhow;

pub fn set_offset_tenths(_offset: i32) {}

#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub celsius: f32,
    pub humidity: f32,
    pub sensor_celsius: f32,
    pub sensor_humidity: f32,
}

#[derive(Clone)]
pub struct Shtc3;

impl Shtc3 {
    pub fn read(&self) -> anyhow::Result<Reading> {
        Err(anyhow!("A simulated board has no temperature sensor"))
    }
}
