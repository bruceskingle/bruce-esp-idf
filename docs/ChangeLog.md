# ChangeLog

# Release 0.1.0

## New

- bruce-embedded-std's platform on ESP32 boards, with ESP-IDF and the standard library: Wi-Fi, which starts as an
  access point with a captive portal until it is set up, an HTTP server with each *Feature*'s config page, settings
  in NVS, mDNS, NTP time and the task scheduler.
- `Esp32Platform::builder()`, which gives the hardware the platform does not use as the `Remainder`, and a Cargo
  feature for each board, of which a crate selects one: the CYD (ESP32-2432S028), the ESP32 DevKit V1, the ESP32-C3
  SuperMini, the Guition JC3248W535C, the Seeed XIAO ESP32-C6, the Waveshare ESP32-C6 LCD 1.47 and Touch LCD 1.47,
  and the Waveshare ESP32-S3 RLCD 4.2.
- Drivers for the boards' hardware: displays, drawn on with `embedded-graphics`, including black and white ones, push
  buttons with actions, SD cards at `/sd`, smart LEDs by RMT or SPI, the SHTC3 temperature and humidity sensor, a
  speaker which plays tones and WAV files, and IMUs for attitude and tilt.
- The record of the board each firmware image was built for, from bruce-embedded-firmware-record, so that
  bruce-embedded-flash refuses to flash it to another board, and the chip and board shown on the config page and
  `/api/info`.
- A demo for each board, in `boards/`, built and flashed with `cargo run --release` in its directory.
