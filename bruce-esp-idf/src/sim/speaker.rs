//! The speaker, for the simulator, with the platform's speaker.rs's API. Sounds are not played on the computer yet,
//! only logged, see docs/Simulator.md.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16_000;

#[derive(Clone)]
pub struct Speaker {
    volume: Arc<AtomicU8>,
}

impl Default for Speaker {
    fn default() -> Self {
        Speaker {
            volume: Arc::new(AtomicU8::new(crate::features::speaker_settings::DEFAULT_VOLUME)),
        }
    }
}

impl Speaker {
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
    }

    pub fn play(&self, samples: &[i16]) -> anyhow::Result<()> {
        self.play_at(self.volume.load(Ordering::Relaxed), samples)
    }

    pub fn play_at(&self, volume: u8, samples: &[i16]) -> anyhow::Result<()> {
        log::debug!(
            "A sound of {} ms, at {volume}%, not played: the simulator's speaker is silent",
            samples.len() as u64 * 1000 / SAMPLE_RATE as u64
        );
        Ok(())
    }

    pub fn play_wav(&self, wav: &[u8]) -> anyhow::Result<()> {
        log::debug!(
            "A sound of {} bytes, not played: the simulator's speaker is silent",
            wav.len()
        );
        Ok(())
    }

    pub fn play_tone(&self, frequency: f32, duration: Duration) -> anyhow::Result<()> {
        self.play(&tone(frequency, duration))
    }

    pub fn set_preview_wav(&self, _wav: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn preview(&self, volume: u8) -> anyhow::Result<()> {
        self.play_at(volume, &tone(880.0, Duration::from_millis(120)))
    }
}

/// A sine wave of the frequency, for the duration, at SAMPLE_RATE.
pub fn tone(frequency: f32, duration: Duration) -> Vec<i16> {
    let count = (duration.as_secs_f32() * SAMPLE_RATE as f32) as usize;
    (0..count)
        .map(|index| {
            let phase = index as f32 * frequency / SAMPLE_RATE as f32 * std::f32::consts::TAU;
            (phase.sin() * i16::MAX as f32 * 0.5) as i16
        })
        .collect()
}
