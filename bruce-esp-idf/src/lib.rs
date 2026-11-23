use std::fmt::Display;

use bruce_embedded_std::{
    config::{ConfigSpec, FeatureConfig},
    feature::FeatureDescriptor,
};

#[cfg(feature = "sim")]
pub use bruce_embedded_sim::svc;
#[cfg(feature = "sim")]
pub use bruce_embedded_sim::svc::sys;
/// ESP-IDF's services and C API, or, with the sim feature, the simulator's stand-ins for them under the same names, so
/// that the code which uses them compiles on a computer as it is, see sim/mod.rs and bruce-embedded-std's
/// docs/Simulator.md.
#[cfg(not(feature = "sim"))]
pub use esp_idf_svc as svc;
#[cfg(not(feature = "sim"))]
pub use esp_idf_sys as sys;

/// The platform on a desktop computer, see sim/mod.rs.
#[cfg(feature = "sim")]
pub(crate) mod sim;

#[cfg_attr(feature = "sim", path = "sim/platform.rs")]
mod esp32_platform;
pub use esp32_platform::*;

#[cfg(esp_idf_esp_https_server_enable)]
pub mod api;
#[cfg(feature = "battery")]
#[cfg_attr(feature = "sim", path = "sim/battery.rs")]
pub mod battery;
#[cfg_attr(feature = "sim", path = "sim/chip.rs")]
pub mod chip;
#[cfg_attr(feature = "sim", path = "sim/push_button.rs")]
pub mod push_button;
#[cfg(feature = "sd-card")]
#[cfg_attr(feature = "sim", path = "sim/sd_card.rs")]
pub mod sd_card;
#[cfg(feature = "shtc3")]
#[cfg_attr(feature = "sim", path = "sim/shtc3.rs")]
pub mod shtc3;
#[cfg(feature = "speaker")]
#[cfg_attr(feature = "sim", path = "sim/speaker.rs")]
pub mod speaker;

#[cfg(esp_idf_esp_https_server_enable)]
pub mod adoption;
#[cfg(esp_idf_esp_https_server_enable)]
#[cfg_attr(feature = "sim", path = "sim/identity.rs")]
mod identity;
#[cfg(esp_idf_esp_https_server_enable)]
#[cfg_attr(feature = "sim", path = "sim/memory.rs")]
mod memory;
#[cfg_attr(feature = "sim", path = "sim/panics.rs")]
pub mod panics;
mod starts;

/// The display's SPI device, on a bus which lives as long as the device, so that other devices can share it, e.g. the
/// SD card slot of the Waveshare ESP32-C6-Touch-LCD-1.47, see sd_card.rs.
#[cfg(all(any(feature = "mipi-dsi-display", feature = "display-st7305"), not(feature = "sim")))]
pub type DisplaySpi = esp_idf_hal::spi::SpiDeviceDriver<'static, &'static esp_idf_hal::spi::SpiDriver<'static>>;

pub use bruce_embedded_firmware_record::FirmwareRecord;

/// The firmware a board crate builds, which the platform cannot know itself, made with firmware!, see
/// Esp32PlatformBuilder::with_firmware.
#[derive(Clone, Copy)]
pub struct Firmware {
    pub name: &'static str,
    pub version: &'static str,
    /// The record in the image of the board it was built for, which a flasher reads before flashing, see
    /// bruce-embedded-firmware-record, or None if the board crate did not say what it builds.
    pub record: Option<&'static FirmwareRecord>,
}

/// The Firmware of the crate it is called in, its name and version, built for the board its `board-*` feature
/// chooses, with the record of them a flasher reads before flashing, placed by the linker at the start of the image,
/// e.g. `Esp32Platform::builder()...with_firmware(bruce_esp_idf::firmware!())`.
#[cfg(not(feature = "sim"))]
#[macro_export]
macro_rules! firmware {
    () => {{
        #[used]
        #[unsafe(link_section = ".rodata_custom_desc")]
        static RECORD: $crate::FirmwareRecord =
            $crate::FirmwareRecord::new($crate::board_name(), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        $crate::Firmware {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            // Through black_box, so that the record is read where it is, and so kept by the linker, which keeps only
            // what is referred to, rather than its contents copied by the compiler.
            record: Some(::core::hint::black_box(&RECORD)),
        }
    }};
}

/// The Firmware of the crate it is called in, for the simulator, which has no image for a record to be placed in.
#[cfg(feature = "sim")]
#[macro_export]
macro_rules! firmware {
    () => {{
        $crate::Firmware {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            record: None,
        }
    }};
}

/// The board's name as the device reports it to an app, see api.rs: board_name, with -sim after it in the simulator,
/// e.g. wave-esp32s3-rlcd-42-sim, so that a simulated device is told apart from a real one wherever it is shown.
pub fn reported_board_name() -> String {
    if SIMULATED {
        format!("{}-sim", board_name())
    } else {
        board_name().to_string()
    }
}

/// Whether this is the platform on a computer, in the board simulator, see sim/mod.rs, e.g. for firmware to say that a
/// board has no battery, rather than that it cannot be read.
pub const SIMULATED: bool = cfg!(feature = "sim");

/// Restarts the device, as its restart command, /api/restart and a factory reset do: unmounts the SD card first, so
/// that it is not in the middle of a write, which leaves some cards unable to answer until the power is cut, see
/// sd_card::unmount.
pub fn restart() -> ! {
    #[cfg(all(feature = "sd-card", not(feature = "sim")))]
    sd_card::unmount();
    unsafe { sys::esp_restart() }
}

/// Why the simulated device last started: it is started like a device which is powered on.
#[cfg(feature = "sim")]
pub fn reset_reason() -> &'static str {
    "PowerOn"
}

/// Why the device last started, e.g. Panic, or Brownout, which shows power dips, as /api/info says, see api.rs.
#[cfg(not(feature = "sim"))]
pub fn reset_reason() -> &'static str {
    #[allow(non_upper_case_globals)]
    match unsafe { esp_idf_sys::esp_reset_reason() } {
        esp_idf_sys::esp_reset_reason_t_ESP_RST_POWERON => "PowerOn",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_EXT => "ExternalPin",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_SW => "Software",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_PANIC => "Panic",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_INT_WDT => "InterruptWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_TASK_WDT => "TaskWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_WDT => "OtherWatchdog",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_DEEPSLEEP => "DeepSleep",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_BROWNOUT => "Brownout",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_SDIO => "Sdio",
        // Of newer chips, e.g. the ESP32-C6, which is reset over its own USB when it is flashed.
        esp_idf_sys::esp_reset_reason_t_ESP_RST_USB => "Usb",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_JTAG => "Jtag",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_EFUSE => "Efuse",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_PWR_GLITCH => "PowerGlitch",
        esp_idf_sys::esp_reset_reason_t_ESP_RST_CPU_LOCKUP => "CpuLockup",
        _ => "Unknown",
    }
}

#[cfg(all(
    feature = "boot-button",
    not(any(
        feature = "board-wave-esp32s3-rlcd-42",
        feature = "board-guition-jc3248w535c",
        feature = "board-wave-esp32s3-lcd-147"
    ))
))]
compile_error!(
    "The boot-button feature is for a board whose BOOT button the platform does not use: board-wave-esp32s3-rlcd-42, board-guition-jc3248w535c, board-wave-esp32s3-lcd-147."
);
// The QMI8658's driver is in the ahrs module, which only the tilt and ahrs features build, so a board with one needs
// either, rather than failing to compile somewhere less clear.
#[cfg(all(feature = "qmi8658", not(any(feature = "tilt", feature = "ahrs"))))]
compile_error!(
    "A board with a QMI8658 IMU, board-wave-esp32c6-touch-147 or board-wave-esp32s3-lcd-147, needs the tilt or ahrs feature."
);
#[cfg(all(feature = "key-button", not(feature = "board-wave-esp32s3-rlcd-42")))]
compile_error!("The key-button feature is for a board with a KEY button: board-wave-esp32s3-rlcd-42.");

/// The board this was built for, from its board- feature.
pub const fn board_name() -> &'static str {
    if cfg!(feature = "board-cyd-esp32-28") {
        "cyd-esp32-28"
    } else if cfg!(feature = "board-esp32-devkit-v1") {
        "esp32-devkit-v1"
    } else if cfg!(feature = "board-esp32c3-supermini") {
        "esp32c3-supermini"
    } else if cfg!(feature = "board-wave-esp32c6-lcd-147") {
        "wave-esp32c6-lcd-147"
    } else if cfg!(feature = "board-wave-esp32c6-touch-147") {
        "wave-esp32c6-touch-147"
    } else if cfg!(feature = "board-wave-esp32s3-lcd-147") {
        "wave-esp32s3-lcd-147"
    } else if cfg!(feature = "board-wave-esp32s3-rlcd-42") {
        "wave-esp32s3-rlcd-42"
    } else if cfg!(feature = "board-guition-jc3248w535c") {
        "guition-jc3248w535c"
    } else if cfg!(feature = "board-seeed-esp32c6-xiao") {
        "seeed-esp32c6-xiao"
    } else {
        "unknown"
    }
}

mod commands;
mod config_store;
#[cfg_attr(feature = "sim", path = "sim/core.rs")]
mod core;
pub mod features;
mod http;
#[cfg(feature = "led")]
mod led;
#[cfg(not(feature = "sim"))]
mod portal;
#[cfg(not(feature = "sim"))]
pub mod smart_led;
#[cfg(not(feature = "sim"))]
mod wifi;
// pub mod led_strip;
#[cfg(feature = "touch-driver")]
#[cfg_attr(feature = "sim", path = "sim/touch.rs")]
pub mod touch;

/// The attitude and tilt, from bruce-embedded-std, and the IMU chips the boards have, which give it their readings.
#[cfg(any(feature = "tilt", feature = "ahrs"))]
pub mod ahrs {
    pub use bruce_embedded_std::ahrs::*;
    pub mod qmi8658;
}

#[cfg(feature = "display")]
#[cfg_attr(feature = "sim", path = "sim/display.rs")]
mod display;
/// The board's display manager, the type of Esp32Platform::display_manager, for features outside the platform.
#[cfg(feature = "display")]
pub use display::BoardDisplayManager;

pub trait AnyhowResultExt<T> {
    fn anyhow(self) -> anyhow::Result<T>;
}

impl<T, E> AnyhowResultExt<T> for Result<T, E>
where
    E: Display,
{
    fn anyhow(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("Operation failed: {}", e))
    }
}

// pub trait DynFeature {
//     fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;
//     fn start_with_config(
//         &mut self,
//         platform: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: &ConfigSpec,
//     ) -> anyhow::Result<()>;
// }

// pub trait Feature<C: FeatureConfig>: DynFeature {
//     fn start_with_config(
//         &mut self,
//         platform: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: &ConfigSpec,
//     ) -> anyhow::Result<()> {
//         let typed_config = C::from_config_spec(config)?;
//         self.start(platform, initializer, typed_config)
//     }

//     fn init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

//     fn start(
//         &mut self,
//         platform: &mut Esp32Platform,
//         initializer: &mut Esp32PlatformInitializer,
//         config: C,
//     ) -> anyhow::Result<()>;
// }

pub trait DynFeature {
    fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

    fn start_with_config(
        &mut self,
        platform: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &ConfigSpec,
    ) -> anyhow::Result<()>;
}

pub trait Feature {
    type Config: FeatureConfig;

    fn init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor>;

    fn start(
        &mut self,
        platform: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: Self::Config,
    ) -> anyhow::Result<()>;
}

impl<F: Feature> DynFeature for F {
    fn do_init(&self, init: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        F::init(self, init)
    }

    fn start_with_config(
        &mut self,
        platform: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        config: &ConfigSpec,
    ) -> anyhow::Result<()> {
        let typed_config = F::Config::from_config_spec(config)?;
        self.start(platform, initializer, typed_config)
    }
}
