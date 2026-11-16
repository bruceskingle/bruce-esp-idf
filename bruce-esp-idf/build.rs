// Makes ESP-IDF's config visible to the code as cfgs, e.g. esp_idf_esp_https_server_enable, as esp-idf-svc does, or, for
// the simulator, the sim feature, those of the config it stands for.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(esp_idf_esp_https_server_enable)");
    if std::env::var_os("CARGO_FEATURE_SIM").is_some() {
        println!("cargo::rustc-cfg=esp_idf_esp_https_server_enable");
        return;
    }
    embuild::espidf::sysenv::output();
}
