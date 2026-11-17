//! What the device runs on, for the simulator: the computer, which has no chip's model, flash or PSRAM to report, see
//! the platform's chip.rs.

use serde_json::{Value, json};

#[derive(Default)]
pub struct Chip {
    pub model: String,
    pub revision: String,
    pub cores: u8,
    pub flash_bytes: u64,
    pub psram_bytes: u64,
}

pub fn chip() -> Chip {
    Chip {
        model: "simulator".to_string(),
        revision: String::new(),
        cores: std::thread::available_parallelism().map_or(1, |cores| cores.get().min(255) as u8),
        flash_bytes: 0,
        psram_bytes: 0,
    }
}

impl Chip {
    /// As a real chip's, see the platform's chip.rs, every field of which an app reads.
    pub fn to_json(&self) -> Value {
        json!({
            "model": self.model,
            "revision": self.revision,
            "cores": self.cores,
            "flashBytes": self.flash_bytes,
            "psramBytes": self.psram_bytes,
            "simulated": true,
        })
    }

    pub fn describe(&self) -> String {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH) + " computer, simulated"
    }
}
