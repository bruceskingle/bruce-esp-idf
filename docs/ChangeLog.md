# ChangeLog

# Release 0.2.0

## New

- **The board simulator.** With the `sim` feature, in place of `espidf`, and the feature of a board bruce-embedded-sim
  simulates, the Waveshare ESP32-S3-RLCD-4.2 or the Guition JC3248W535C, the platform builds for a computer, so that a
  board's firmware runs in a window showing its display, see bruce-embedded-std's docs/Simulator.md. Its HTTP and HTTPS
  server shares one port per simulated device, kept in its settings, its settings are files in its own data folder,
  and its sensors and battery read nothing, rather than made-up values. `boards/sim.sh` runs a board's demo in it.
- `bruce_esp_idf::svc` and `sys`, ESP-IDF's services and C API, or the simulator's stand-ins for them under the same
  names, so that code which uses them builds for both.
- `SIMULATED`, whether the platform is the simulator, e.g. to say that a board has no battery rather than that it
  cannot be read, and `reported_board_name`, the board's name as the device reports it, with `-sim` after it in the
  simulator, so that a simulated device is told apart from a real one.
- `Esp32PlatformBuilder::with_required_feature`, for a feature which is always enabled, as the platform's own are, e.g.
  one a firmware's screen is laid out for, so that it is not missing from a new device.
- `Esp32Platform::address`, the address an app reaches the device at: its IP address on a board, with its port in the
  simulator.

## Improved

- Threads are given their stack sizes through bruce-embedded-std's `thread_stack`, so that they are as small as they
  were on a board, and large enough on a computer.

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
