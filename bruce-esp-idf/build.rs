// Makes ESP-IDF's config visible to the code as cfgs, e.g. esp_idf_esp_https_server_enable, as esp-idf-svc does, or, for
// the simulator, the sim feature, those of the config it stands for.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(esp_idf_esp_https_server_enable)");
    // Whether the console is the chip's own USB port, rather than UART0, which frees UART0's pins, e.g. on the
    // Waveshare ESP32-S3-DualEye-LCD-1.28's header, see its Remainder.
    println!("cargo::rustc-check-cfg=cfg(esp_idf_esp_console_usb_serial_jtag)");
    if std::env::var_os("CARGO_FEATURE_SIM").is_some() {
        println!("cargo::rustc-cfg=esp_idf_esp_https_server_enable");
        return;
    }
    embuild::espidf::sysenv::output();
}
