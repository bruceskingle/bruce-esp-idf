//! The SD card, for the simulator: a folder of the device's, see sim/mod.rs, with the platform's sd_card.rs's API.

/// Where the card is on the device, which the simulator's folder stands for, see Esp32Platform::sd_card.
pub const MOUNT_POINT: &str = "/sd";

/// The card's size and free space: those of the disk the folder is on are the computer's, not a card's, so none.
pub fn space() -> Option<(u64, u64)> {
    None
}
