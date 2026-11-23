//! Erasing the device's saved settings, its NVS, from the config page's commands, see commands.rs: either everything,
//! a factory reset, so that it starts as a new board, or everything but its Wi-Fi network and password, so that it
//! stays on the network, e.g. to be adopted again. Erasing only the core feature's settings, as the factory reset did
//! before 23 November 2026, left the other features' settings and the adoption, so a capture device kept crashing on a
//! feature it had been set up with.

use log::info;

use crate::core::CORE_FEATURE_NAME;

/// What is erased, see erase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reset {
    /// Everything but the network settings: the core feature's, its Wi-Fi network and password, name on the network
    /// and time zone, and ESP-IDF's own Wi-Fi and radio namespaces, see KEPT_NAMESPACES.
    KeepWifi,
    /// Everything.
    Everything,
}

/// The namespaces kept by Reset::KeepWifi: the core feature's, which are what a device needs to join the network and be
/// found on it, its Wi-Fi network and password, its name on the network and its time zone, all of which it needs to
/// be valid, without which the device would start its own network for setting it up rather than join this one, as it
/// did on 23 November 2026, when only the Wi-Fi was kept; and ESP-IDF's own, the Wi-Fi driver's saved state, and the
/// radio's calibration.
const KEPT_NAMESPACES: [&str; 3] = [CORE_FEATURE_NAME, "nvs.net80211", "phy"];

/// Erases the device's settings, see Reset, which takes effect when it restarts, as the running features have read
/// theirs already.
pub fn erase(reset: Reset) -> anyhow::Result<()> {
    info!("Erasing the device's settings, {reset:?}");
    erase_settings(reset)
}

/// On a device, by ESP-IDF's NVS functions: the whole partition, or each namespace found by iterating its entries.
#[cfg(not(feature = "sim"))]
fn erase_settings(reset: Reset) -> anyhow::Result<()> {
    use crate::sys::{ESP_OK, esp, nvs_close, nvs_commit, nvs_erase_all, nvs_open};
    if reset == Reset::Everything {
        // ESP-IDF closes the partition first, as everything is erased.
        return Ok(esp!(unsafe { crate::sys::nvs_flash_erase() })?);
    }
    // Each namespace with anything in it, as they are, before any is erased.
    let mut entries: Vec<String> = Vec::new();
    let mut iterator: crate::sys::nvs_iterator_t = std::ptr::null_mut();
    let partition = c"nvs";
    let mut result = unsafe {
        crate::sys::nvs_entry_find(
            partition.as_ptr(),
            std::ptr::null(),
            crate::sys::nvs_type_t_NVS_TYPE_ANY,
            &mut iterator,
        )
    };
    while result == ESP_OK {
        let mut info = crate::sys::nvs_entry_info_t::default();
        unsafe { crate::sys::nvs_entry_info(iterator, &mut info) };
        let text = |chars: &[core::ffi::c_char]| {
            let bytes: Vec<u8> = chars.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
            String::from_utf8_lossy(&bytes).to_string()
        };
        entries.push(text(&info.namespace_name));
        result = unsafe { crate::sys::nvs_entry_next(&mut iterator) };
    }
    unsafe { crate::sys::nvs_release_iterator(iterator) };
    let mut namespaces = entries;
    namespaces.sort();
    namespaces.dedup();
    for namespace in namespaces {
        if KEPT_NAMESPACES.contains(&namespace.as_str()) {
            continue;
        }
        let name = std::ffi::CString::new(namespace.clone())?;
        let mut handle = 0;
        esp!(unsafe { nvs_open(name.as_ptr(), crate::sys::nvs_open_mode_t_NVS_READWRITE, &mut handle) })?;
        let erased: anyhow::Result<()> = esp!(unsafe { nvs_erase_all(handle) }).map_err(Into::into);
        let committed = esp!(unsafe { nvs_commit(handle) });
        unsafe { nvs_close(handle) };
        match erased.and(committed.map_err(Into::into)) {
            Ok(()) => info!("Erased the settings of {namespace}"),
            Err(error) => log::warn!("Unable to erase the settings of {namespace}, {error:#}"),
        }
    }
    Ok(())
}

/// In the simulator, its NVS is a folder of a file a namespace, see bruce-embedded-sim's nvs.rs, of which its own, of
/// the port it is found at, is kept either way, so that an app still finds it.
#[cfg(feature = "sim")]
fn erase_settings(reset: Reset) -> anyhow::Result<()> {
    use crate::svc::nvs::EspNvs;
    let partition = crate::sim::nvs_partition()?;
    for entry in std::fs::read_dir(partition.folder())?.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(namespace) = file.strip_suffix(".json") else {
            continue;
        };
        if namespace == "simulator" || (reset == Reset::KeepWifi && KEPT_NAMESPACES.contains(&namespace)) {
            continue;
        }
        EspNvs::new(partition.clone(), namespace, true)?.erase_all()?;
        info!("Erased the settings of {namespace}");
    }
    Ok(())
}
