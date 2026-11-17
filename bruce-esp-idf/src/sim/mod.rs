//! What the platform is on a desktop computer, with the sim feature: the simulated board, see bruce-embedded-sim, which
//! the twins of the ESP-IDF-only modules in this folder use in place of the hardware, and the device's folder, ports
//! and address. See bruce-embedded-std's docs/Simulator.md.

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::anyhow;
use bruce_embedded_sim::svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};

/// The simulated board, with the options of its command line, made by Esp32Platform::builder.
struct Simulated {
    device: bruce_embedded_sim::Device,
    options: bruce_embedded_sim::Options,
    folder: PathBuf,
}

static SIMULATED: OnceLock<Simulated> = OnceLock::new();

/// Makes the simulated board this was built for, from its board- feature, with the options of the command line, see
/// bruce_embedded_sim::Options, and --data FOLDER, where the device keeps what a real one keeps in NVS and on its SD
/// card, by default a folder of the firmware's and the board's in the computer's folder for programs' data, see
/// firmware_name.
pub(crate) fn start() -> anyhow::Result<()> {
    if SIMULATED.get().is_some() {
        return Ok(());
    }
    let board = bruce_embedded_sim::Board::named(crate::board_name())
        .ok_or_else(|| anyhow!("The {} board cannot be simulated yet", crate::board_name()))?;
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let options = bruce_embedded_sim::Options::from_args(&mut args)?;
    let folder = match args.iter().position(|arg| arg == "--data") {
        Some(position) => PathBuf::from(args.get(position + 1).ok_or_else(|| anyhow!("--data needs a folder"))?),
        None => {
            let simulator = dirs::data_dir()
                .ok_or_else(|| anyhow!("There is no folder for programs' data, give one with --data"))?
                .join("Bruce's Embedded Simulator");
            let folder = simulator.join(firmware_name(board)).join(board.id);
            // Where the simulator kept every firmware's data before 16 November 2026, by the platform's name, which
            // the first firmware to start since takes, so that a virtual device an app has adopted keeps its identity.
            let before = simulator.join(env!("CARGO_PKG_NAME")).join(board.id);
            if !folder.exists() && before.exists() && before != folder {
                std::fs::create_dir_all(folder.parent().unwrap_or(&simulator))?;
                std::fs::rename(&before, &folder)?;
                log::info!(
                    "Moved the device's data from {} to {}",
                    before.display(),
                    folder.display()
                );
            }
            folder
        }
    };
    std::fs::create_dir_all(folder.join("sd"))?;
    log::info!("Simulating a {}, keeping its data in {}", board.title, folder.display());
    bruce_embedded_sim::svc::sys::start_timer();
    let device = bruce_embedded_sim::Device::new(board);
    // So that its window opens where it was, e.g. after it restarts itself.
    device.set_window_file(folder.join("window.txt"));
    let _ = SIMULATED.set(Simulated {
        device,
        options,
        folder,
    });
    Ok(())
}

/// The name of the firmware, for its folder of data, from its program's name, without the board's name which a launcher's
/// copy of it has after it, e.g. bruce-smart-home-capture of bruce-smart-home-capture-rlcd42, as the firmware's own name
/// is not known until it says, see Esp32PlatformBuilder::with_firmware, after the folder is needed.
fn firmware_name(board: &bruce_embedded_sim::Board) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|program| program.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "firmware".to_string());
    program
        .strip_suffix(&format!("-{}", board.id))
        .map(String::from)
        .unwrap_or(program)
}

fn simulated() -> &'static Simulated {
    SIMULATED
        .get()
        .expect("The simulated board is made by Esp32Platform::builder, before anything uses it")
}

/// The simulated board.
pub(crate) fn device() -> &'static bruce_embedded_sim::Device {
    &simulated().device
}

pub(crate) fn options() -> bruce_embedded_sim::Options {
    simulated().options.clone()
}

/// Where the device's NVS is kept.
pub(crate) fn nvs_partition() -> anyhow::Result<EspNvsPartition<NvsDefault>> {
    Ok(EspNvsPartition::at(simulated().folder.join("nvs"))?)
}

/// Where the device's SD card is, a folder.
pub(crate) fn sd_folder() -> PathBuf {
    simulated().folder.join("sd")
}

/// The device's port, for HTTP and HTTPS alike, see bruce-embedded-sim's server.rs, chosen the first time it starts,
/// from those free then, and kept in its NVS, so that an app finds it at the same address, host and port, as several
/// simulated devices may run on one computer, where a real device's 80 and 443 are not free.
pub(crate) fn port(nvs_partition: &EspNvsPartition<NvsDefault>) -> anyhow::Result<u16> {
    let nvs = EspNvs::new(nvs_partition.clone(), "simulator", true)?;
    if let Some(port) = nvs.get_u16("port")? {
        return Ok(port);
    }
    let port = std::net::TcpListener::bind("0.0.0.0:0")?.local_addr()?.port();
    nvs.set_u16("port", port)?;
    Ok(port)
}

/// The computer's address on its network, which the simulated device is reached at, found by asking the system which
/// it would send from to an address on the internet, which sends nothing, or the loopback address if it has none.
pub(crate) fn ip_address() -> Ipv4Addr {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("192.0.2.1:9")?;
            socket.local_addr()
        })
        .ok()
        .and_then(|address| match address.ip() {
            std::net::IpAddr::V4(address) => Some(address),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}
