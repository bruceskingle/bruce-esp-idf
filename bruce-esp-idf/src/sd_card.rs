//! The SD card slot of a board which has one, e.g. the CYD, on-board hardware which the platform owns when the
//! sd-card feature is enabled. The card is mounted with FAT at MOUNT_POINT at start-up, so that features can use it with
//! std::fs, see Esp32Platform::sd_card. A card which is missing or cannot be read is not an error for the device, only
//! for the features which want it.

use std::any::Any;
use std::sync::Mutex;

use esp_idf_hal::gpio::AnyIOPin;
use esp_idf_hal::sd::spi::SdSpiHostDriver;
use esp_idf_hal::sd::{SdCardConfiguration, SdCardDriver};
use esp_idf_hal::spi::{Dma, SpiDriver, SpiDriverConfig};
use esp_idf_svc::fs::fatfs::Fatfs;
use esp_idf_svc::io::vfs::MountedFatfs;
use log::info;

pub const MOUNT_POINT: &str = "/sd";
/// How many files may be open at once.
const MAX_OPEN_FILES: usize = 4;
/// The SPI clock once the card has started, which is always at 400 kHz, half ESP-IDF's 20 MHz default. An old card in
/// the CYD failed to read and write even at 4 MHz, so 400 kHz was used, which made reading a day's records take
/// minutes. A good card is fine at this.
const SPEED_KHZ: u32 = 10_000;
/// How long the card is left powered, with its driver stopped, after it is unmounted and before the firmware restarts,
/// see unmount, for it to finish any work of its own, e.g. moving data about, which a card may do after a write.
const SETTLE_TIME: std::time::Duration = std::time::Duration::from_millis(500);

/// The mounted card, its file system and its driver, kept until it is unmounted, see unmount.
static MOUNTED: Mutex<Option<Mounted>> = Mutex::new(None);

/// What must be kept for the card to stay mounted, the file system holding the driver.
struct Mounted(#[allow(dead_code)] Box<dyn Any>);

// The drivers are not marked Send, as they hold raw handles, but they are only ever dropped, once, under MOUNTED's
// lock, from whichever thread restarts the device, which ESP-IDF allows.
unsafe impl Send for Mounted {}

/// Keeps the card mounted until it is unmounted, see unmount.
fn keep(mounted: impl Any) {
    *MOUNTED.lock().unwrap() = Some(Mounted(Box::new(mounted)));
}

/// Unmounts the card, if it is mounted, so that its files are closed, its file system written, and its driver stopped,
/// then waits SETTLE_TIME, for a restart which does not cut the power, see crate::restart. Some cards do not answer
/// after such a restart if it came in the middle of a write, see mount_mmc_4bit, so this is for the firmware's own
/// restarts; nothing can be done for a reset over USB, by the reset button, or after a crash. Anything still writing to
/// the card fails, which the store is made to survive.
pub fn unmount() {
    if MOUNTED.lock().unwrap().take().is_some() {
        info!("SD card unmounted");
        std::thread::sleep(SETTLE_TIME);
    }
}

/// Mounts the card.
#[cfg(feature = "board-cyd-esp32-28")]
pub fn mount(
    spi: esp_idf_hal::spi::SPI3<'static>,
    sclk: esp_idf_hal::gpio::Gpio18<'static>,
    mosi: esp_idf_hal::gpio::Gpio23<'static>,
    miso: esp_idf_hal::gpio::Gpio19<'static>,
    cs: esp_idf_hal::gpio::Gpio5<'static>,
) -> anyhow::Result<()> {
    let spi_driver = SpiDriver::new(
        spi,
        sclk,
        mosi,
        Some(miso),
        &SpiDriverConfig::default().dma(Dma::Auto(4096)),
    )?;
    // SD cards need pull-ups on their lines in SPI mode, which the CYD does not have, so the ESP32's own, as ESP-IDF's
    // examples do. Without them the card answers some commands and not others.
    for pin in [5, 18, 19, 23] {
        unsafe { esp_idf_sys::gpio_set_pull_mode(pin, esp_idf_sys::gpio_pull_mode_t_GPIO_PULLUP_ONLY) };
    }
    let mut config = SdCardConfiguration::new();
    config.speed_khz = SPEED_KHZ;
    let card = SdCardDriver::new_spi(
        SdSpiHostDriver::new(
            spi_driver,
            Some(cs),
            AnyIOPin::none(),
            AnyIOPin::none(),
            AnyIOPin::none(),
            None,
        )?,
        &config,
    )?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    keep(mounted);
    Ok(())
}

/// Mounts the card of a board whose slot shares the display's SPI bus, bus, with its chip select, cs. The display's device must be on the bus first, so that its chip select is
/// held high while the card starts.
#[cfg(feature = "board-wave-esp32c6-touch-147")]
pub fn mount_shared(bus: &'static SpiDriver<'static>, cs: esp_idf_hal::gpio::Gpio4<'static>) -> anyhow::Result<()> {
    // As on the CYD, see mount, the card's lines need pull-ups: MISO and its chip select, the others being the
    // display's too.
    for pin in [3, 4] {
        unsafe { esp_idf_sys::gpio_set_pull_mode(pin, esp_idf_sys::gpio_pull_mode_t_GPIO_PULLUP_ONLY) };
    }
    let mut config = SdCardConfiguration::new();
    config.speed_khz = SPEED_KHZ;
    let card = SdCardDriver::new_spi(
        SdSpiHostDriver::new(
            bus,
            Some(cs),
            AnyIOPin::none(),
            AnyIOPin::none(),
            AnyIOPin::none(),
            None,
        )?,
        &config,
    )?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    keep(mounted);
    Ok(())
}

/// Mounts the card of the Guition JC3248W535C, whose slot is on its own SPI bus, SPI3, on GPIO12 (CLK), 11 (MOSI), 13
/// (MISO) and 10 (CS).
#[cfg(feature = "board-guition-jc3248w535c")]
pub fn mount_spi3(
    spi: esp_idf_hal::spi::SPI3<'static>,
    sclk: esp_idf_hal::gpio::Gpio12<'static>,
    mosi: esp_idf_hal::gpio::Gpio11<'static>,
    miso: esp_idf_hal::gpio::Gpio13<'static>,
    cs: esp_idf_hal::gpio::Gpio10<'static>,
) -> anyhow::Result<()> {
    let spi_driver = SpiDriver::new(
        spi,
        sclk,
        mosi,
        Some(miso),
        &SpiDriverConfig::default().dma(Dma::Auto(4096)),
    )?;
    // As on the CYD, see mount, the ESP32's own pull-ups, in case the board has none.
    for pin in [10, 11, 12, 13] {
        unsafe { esp_idf_sys::gpio_set_pull_mode(pin, esp_idf_sys::gpio_pull_mode_t_GPIO_PULLUP_ONLY) };
    }
    let mut config = SdCardConfiguration::new();
    config.speed_khz = SPEED_KHZ;
    let card = SdCardDriver::new_spi(
        SdSpiHostDriver::new(
            spi_driver,
            Some(cs),
            AnyIOPin::none(),
            AnyIOPin::none(),
            AnyIOPin::none(),
            None,
        )?,
        &config,
    )?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    keep(mounted);
    Ok(())
}

/// Mounts the card of a board whose slot is wired for the SDMMC host, with one data line, as the Waveshare
/// ESP32-S3-RLCD-4.2's is, on GPIO21 (CMD), 38 (CLK) and 39 (D0).
#[cfg(feature = "board-wave-esp32s3-rlcd-42")]
pub fn mount_mmc(
    slot: esp_idf_hal::sd::mmc::SDMMC1<'static>,
    cmd: esp_idf_hal::gpio::Gpio21<'static>,
    clk: esp_idf_hal::gpio::Gpio38<'static>,
    d0: esp_idf_hal::gpio::Gpio39<'static>,
) -> anyhow::Result<()> {
    use esp_idf_hal::sd::mmc::{SdMmcHostConfiguration, SdMmcHostDriver};
    // The slot has no card detect or write protect lines. The ESP32's own pull-ups, as the board has none.
    let host = SdMmcHostDriver::new_1bit(
        slot,
        cmd,
        clk,
        d0,
        AnyIOPin::none(),
        AnyIOPin::none(),
        &SdMmcHostConfiguration::new(),
    )?;
    let card = SdCardDriver::new_mmc(host, &SdCardConfiguration::new())?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    keep(mounted);
    Ok(())
}

/// Mounts the card of the Waveshare ESP32-S3-LCD-1.47B, whose slot is wired for the SDMMC host with four data lines,
/// on GPIO15 (CMD), 14 (CLK), 16 (D0), 18 (D1), 17 (D2) and 21 (D3).
///
/// The slot is powered whenever the board is, so a restart which does not cut the power, e.g. over USB, the reset
/// button or the firmware's own, leaves the card in whatever state it was in. Some cards, e.g. a 64 GB SanDisk on 22
/// Nov 2026, then do not answer at all until the power is cut, however often they are tried: trying again four times,
/// half a second apart, did not help, so it is tried once. The firmware's own restarts unmount the card first, see
/// unmount, so that it is not in the middle of anything.
#[cfg(feature = "board-wave-esp32s3-lcd-147")]
#[allow(clippy::too_many_arguments)]
pub fn mount_mmc_4bit(
    slot: esp_idf_hal::sd::mmc::SDMMC1<'static>,
    cmd: esp_idf_hal::gpio::Gpio15<'static>,
    clk: esp_idf_hal::gpio::Gpio14<'static>,
    d0: esp_idf_hal::gpio::Gpio16<'static>,
    d1: esp_idf_hal::gpio::Gpio18<'static>,
    d2: esp_idf_hal::gpio::Gpio17<'static>,
    d3: esp_idf_hal::gpio::Gpio21<'static>,
) -> anyhow::Result<()> {
    use esp_idf_hal::sd::mmc::{SdMmcHostConfiguration, SdMmcHostDriver};
    // The slot has no card detect or write protect lines. Its lines have pull-ups on the board, and the driver turns
    // on the chip's own too, by default.
    let host = SdMmcHostDriver::new_4bits(
        slot,
        cmd,
        clk,
        d0,
        d1,
        d2,
        d3,
        AnyIOPin::none(),
        AnyIOPin::none(),
        &SdMmcHostConfiguration::new(),
    )?;
    let card = SdCardDriver::new_mmc(host, &SdCardConfiguration::new())?;
    let mounted = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT_POINT, MAX_OPEN_FILES)?;
    info!("SD card mounted at {MOUNT_POINT}");
    keep(mounted);
    Ok(())
}

/// The mounted card's size and free space, in bytes, from its FAT, None if there is no card mounted or it cannot be
/// read. FatFs counts the free space the first time, which may take a while on a large card, and keeps it up to date
/// after that.
pub fn space() -> Option<(u64, u64)> {
    let path = std::ffi::CString::new(MOUNT_POINT).ok()?;
    let (mut total, mut free) = (0u64, 0u64);
    let result = unsafe { esp_idf_sys::esp_vfs_fat_info(path.as_ptr(), &mut total, &mut free) };
    (result == esp_idf_sys::ESP_OK).then_some((total, free))
}
