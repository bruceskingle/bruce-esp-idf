# bruce-esp-idf
Bruce's ESP-IDF Platform: a platform for embedded applications on ESP32 boards, built on the hardware agnostic [bruce-embedded-std](https://github.com/bruceskingle/bruce-embedded-std), with a demo for each board. It includes the standard Rust library, which means that the heap and standard collections like ```Vec``` are all available for use, and uses the ```esp-idf-hal``` and ```esp-idf-svc``` crates. The platform is split in two, by one rule: bruce-embedded-std holds the hardware agnostic code, and this repository the code specific to ESP32 boards, see [Architecture](#architecture).

# The Core Platform
The core platform provides a set of services to support the running of one or more *Features*, which provide the application functionality. Each feature can be enabled or disabled and may have a configuration. The *Core Feature* provides WiFi and an HTTP server. If the *Core Feature* does not have a valid configuration (including the SSID and password for your WiFi network), then it will start WiFi an *Access Point Mode*. This means that it will create a new WiFi network (with the SSID **ESP32-Setup** and the password **password**) with a captive portal so that if you connect to this network you will be taken automatically to the config page so the necessary configuration values can be entered. Once this is done and the board is rebooted it will start WiFi in client mode, connecting to the WiFi network you have configured as a normal device.

The HTTP server is still available (on the IP address allocated by your WiFi network) which means that further configuration is possible and this HTTP server can also be used by *Features* to provide application functionality. The *Core Feature* includes an mDNS server so that you can easily find the board using a DNS name in the **.local** domain, and a task scheduler which can be used to run task processes on a predetermined schedule using [Cron](https://en.wikipedia.org/wiki/Cron) like schedules, and an [NTP](https://en.wikipedia.org/wiki/Ntpd) client which synchronies the system clock with real time.

The config page's **Commands** restart the device, or erase its saved settings, its NVS, and restart it: **Reset Settings, Keep the Wi-Fi** erases everything but the network settings, the Wi-Fi network and password, the device's name on the network and its time zone, e.g. the features' settings and an app's adoption of it, so that it comes back on the same network as a new device; **Factory Reset, Erase Everything** erases them all, so that it starts with its own Wi-Fi network for setting it up again, as a new board does. Both are the platform's, see ```reset.rs```, so every firmware on it has them. ```bruce-embedded-flash flash --erase-settings``` erases them all over USB, for a board which does not stay up long enough to use its page.

# A Simple Example Application
This is the sample application for the ESP32 DevKitV1 board:

```rust
use bruce_esp_idf::{
    Esp32Platform,
    features::{binary_clock::BinaryClock, dyndns2::DynDns2},
};

fn main() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = bruce_esp_idf::smart_led::new(
        remainder.spi3,
        remainder.gpio14, //SCLK
        remainder.gpio13, //SDO / MISO
        64,
    )?;

    let platform = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(BinaryClock::new_spi(smart_leds)))?
        .build()?;

    platform.start()
}
```
The ```Esp32Platform::builder()``` method returns  a builder for the Platform, which can be used to add *Features* to the Platform, and a ```Remainder``` struct which contains all the hardware resources provided by the board which are not used by the Platform itself. This example includes two *Features* which are provided by the platform. ```DynDns2``` is a Dynamic DNS updater which periodically checks the device's public IP address and when it changes updates a DNS entry. ```BinaryClock``` is a *Feature* which uses WS2812 Smart LEDs to display the date and time in binary.

Two implementations of the Smart LED driver are provided, the preferred approach is to use an RMT device to manage the timings for the Smart LED protocol, but the RMT devices on older ESP32 chips (including the DevKitV1 board) are unreliable, and an alternative SPI based driver is used in those cases. The ```smart_leds``` driver is given an ```SPI``` device and two ```GPIO``` pins from the ```Remainder``` struct, and this is passed to ```BinaryClock``` *Feature*. Finally the platform is created by calling ```.build()``` on the builder, and the ```start()``` method is called, which should never return.

# Why ```std``` and why ```esp-idf-svc```
There are other approaches based on bare metal (non std) architecture (e.g. using ```esp-hal```) and there are lots of great resources to help you to get started with that approach. I started out using this approach and I found the videos on the Youtube ```Rusty Bits``` channel (https://www.youtube.com/watch?v=dxgufYRcNDg) really helpful, so if you would prefer a bare metal approach that would be a great place to start. I was able to get examples to blink an LED, drive WS2812 Smart Leds and draw on the screen of a "Cheap Yellow Display" following those videos. The next thing I wanted to do was to get WiFi working to get NTP time synchronization but although there is code to support WiFi on EP32 in the ```esp-hal``` ecosystem, I was unable to get it to work reliably.

In a search for an alternative approach I came across the YouTube ```Floodplain``` channel (https://www.youtube.com/watch?v=o8yNNVFzNnM&t=9s) and based on that approach, and using the ```esp-idf-svc``` crate I was able to get WiFi working successfully. Binaries build on the standard library and using ```esp-idf-svc``` are larger than bare metal binaries, but I have not had any trouble running them on any ESP32 based board including the DevKitV1, and while this is not a pure Rust implementation (the ```esp-idf-``` crates are a thin Rust wrapper for the C based ESP IDF SDK) those underlying C implementations (especially the WiFi) are production tested mature implementations which I have found to work well.

# Architecture
This repository has the library, ```bruce-esp-idf```, and an example binary crate for each board under ```boards```, each its own Cargo project, with its own target, as each board's chip needs its own:
```
├── boards
│   ├── cyd-esp32-28
│   ├── esp32-devkit-v1
│   ├── esp32c3-supermini
│   ├── guition-jc3248w535c
│   ├── seeed-esp32c6-xiao
│   ├── wave-esp32c6-lcd-147
│   ├── wave-esp32c6-touch-147
│   ├── wave-esp32s3-dualeye-128
│   ├── wave-esp32s3-lcd-147
│   └── wave-esp32s3-rlcd-42
└── bruce-esp-idf
```
The platform is two crates, in two repositories, divided by one rule: code which would work the same on any hardware with the standard library goes in [bruce-embedded-std](https://github.com/bruceskingle/bruce-embedded-std), and code which needs an ESP32, or ESP-IDF, goes in ```bruce-esp-idf```, here. ```bruce-esp-idf``` depends on ```bruce-embedded-std```, never the other way round. It takes bruce-embedded-std and [bruce-embedded-firmware-record](https://github.com/bruceskingle/bruce-embedded-firmware-record) from crates.io, or from checkouts next to this repository while they are linked to try changes to them, see bruce-method-release.

## bruce-embedded-std
The hardware agnostic platform: *Features* and their configuration, the config page, the task scheduler, time zones, graphics such as the clocks and the colours, the mDNS responder, the attitude and tilt worked out from an IMU's readings, and the traits a platform implements, e.g. ```Platform```, ```ConfigStore```, ```HttpServerManager``` and ```DisplayManager```. It has no ESP32 dependencies, so it builds, and its tests run, on a computer. It would be shared by a platform for other hardware, e.g. a Raspberry Pi running Linux.

## bruce-esp-idf
The ESP32 platform: everything which needs ESP-IDF, e.g. Wi-Fi, the HTTP server, NVS, the SD card, and the drivers for each board's hardware, implementing ```bruce-embedded-std```'s traits, with ```Esp32Platform``` and the *Features* which use it. Support for each board, and its hardware, is enabled with ```Cargo``` features.

The device drivers here which use only one or two ESP-IDF types, e.g. ```shtc3.rs```'s ```I2cDriver```, are to become ```no_std``` crates of their own on the ```embedded-hal``` traits, usable on any hardware, rather than move into ```bruce-embedded-std```.

# Design Patterns
The platform makes use of the builder pattern as a way of trying to get the best trade off between the cost and benefits of the standard library. Most of the platform types have a builder containing collections which allow items to be added, but the ```build()``` method will usually call ```.shrink_to_fit()``` on those collections which are then treated as immutable from that point onwards.

The conventions for builders are that they
- are constructed by calling the ```builder()``` associated function on the class being constructed, e.g. ```TaskManager::builder()```
- provide chainable methods with names starting ```with_``` e.g. 
```
    TaskManager::builder()
        .with_task(task1)?
        .with_task(task2)?
        .build();
```
- provide callable methods with names starting ```add_``` e.g.
```
    let mut builder = TaskManager::builder()
        .with_task(task1)?;
    
    builder.add_task(task2)?;
    
    builder.build();
```
- have methods which either return their result directly (if there are no failure scenarios) or return an ```anyhow::Result<>``` of their result.

# Features
This crate uses multiple features to support various different ESP32 boards. There are functional features like ```mono-led``` and ```rgb-led``` which are referenced in the code and which get activated by board level features like ```board-cyd-esp32-28``` and ```board-seeed-esp32c6-xiao```. Client crates should normally select exactly one board feature, and any of the optional hardware features (see below) which their board has and which they want to use.

# Board Capabilities
The platform drives the hardware a board always has itself, such as its display or status LED, and offers the hardware a project may or may not want, because its pins could be used for something else, as optional ```Cargo``` features. Setting one of these features gives that hardware's pins to the platform, so they are no longer in the ```Remainder```, and setting it for a board which does not have that hardware is a compile error.

Two different things are called features here. A ```Cargo``` feature, such as ```key-button```, decides at compile time who owns some hardware. A *Feature*, such as ```AnalogClock```, is a piece of application functionality which can be enabled or disabled on the device's config page at run time.

## Push Buttons
A board's buttons are owned by the platform with features such as ```key-button``` and ```boot-button```. The platform watches each button from the start, and a press runs the button's *action*, if it has one, on the button's own thread, so an action may take its time, for example to show a message for a second. Presses while an action is running are not counted.

The application gives a button its action with the builder. The closure is called with the platform once it is running, before the *Features* start, so that it can take what the action needs, such as the display manager:
```rust
let platform = builder
    .with_key_button(|platform| {
        let display_manager = platform.display_manager.clone();
        Ok(Box::new(move || message::show(&display_manager, "KEY pressed")))
    })
    .build()?;
```
A *Feature* can take a button over when it starts, replacing the application's action, as it is given the platform:
```rust
fn start(&mut self, platform: &mut Esp32Platform, initializer: &mut Esp32PlatformInitializer, config: Self::Config) -> anyhow::Result<()> {
    let display_manager = platform.display_manager.clone();
    platform.key_button().set_action(Box::new(move || next_page(&display_manager)));
    Ok(())
}
```
```clear_action()``` makes a button do nothing again. A button wired to a pin of the ```Remainder``` works in the same way, with ```PushButton::new("Name", remainder.gpio4)?``` and then ```set_action()```. Buttons are taken to connect their pin to ground when pressed, using the chip's own pull-up, as board buttons usually do.

## SD Cards
With the ```sd-card``` feature the platform mounts the board's SD card at start-up, with FAT, at ```/sd```, so that it can be used with ```std::fs```. ```platform.sd_card()``` gives the mount point, or why there is no card, and ```sd_card::space()``` its size and free space. The card is only looked for at start-up, so a card put in later is found when the board next restarts. Some cards do not answer after a restart which does not cut the power: see [docs/Hardware.md](docs/Hardware.md#sd-cards) for those which have been tried.

## Displays
On a board with a display, ```platform.display_manager``` is the board's ```DisplayManager```, which *Features* draw with, using ```embedded-graphics```. On a board with two, the Waveshare ESP32-S3-DualEye-LCD-1.28's pair of round eyes, the left one is the display, which the platform and *Features* draw on, and ```right()``` gives the other, or ```eyes()``` both, each 240 × 240, drawn in memory and sent to the screens 25 times a second, or at once with ```flush()```. On a black and white display (the ```mono-display``` feature) colours are drawn as ink or paper by their brightness, so that a design for a colour display, light on dark, comes out dark on light, and *Features* offer no colour settings.

## Temperature and Humidity
On a board with a temperature and humidity sensor, such as the SHTC3, ```platform.shtc3().read()``` gives a reading. The platform adds a *Feature* of its own, **Temperature and Humidity**, which logs a reading every minute and has a **Temperature Offset** setting, in tenths of a degree. A sensor on the board is warmed by it, so it reads high: set the offset by comparing with a thermometer which can be trusted, after the board has been running for a while. The humidity is corrected with it.

## Speaker
On a board with a speaker, ```platform.speaker()``` plays sounds: ```play_tone(880.0, Duration::from_millis(120))``` for a beep, or ```play_wav(bytes)``` for a WAV file of 16-bit PCM, of any sample rate, mono or stereo, which is mixed to mono and resampled to the speaker's 16 kHz. A sound can be built into the firmware with ```include_bytes!("../sounds/chime.wav")```, or read from the SD card. Each play waits until the sound has finished, so play from a thread of its own to carry on meanwhile. The platform adds a *Feature* of its own, **Speaker**, whose **Volume (%)** setting, a slider which plays a beep at each volume it is moved to, or a sound of the firmware's own, set with ```speaker.set_preview_wav(bytes)```, is what every sound plays at, from as soon as the board starts, 100% until one is saved, or 75% on the Waveshare ESP32-S3-DualEye-LCD-1.28, whose speaker is loud; 100% is the codec's full level, and each percent less is half a dB quieter.

## Sliders
A number setting can be shown on the config page as a slider, with ```#[config(min = 0, max = 100)]``` on an ```i32``` or ```i64``` field of a *Feature*'s config. ```default = N``` is where the slider is while the setting has not been saved, and ```preview = "/path"``` a path on the device which the page POSTs each position to, as ```value=N```, while the slider is moved, so that its effect can be tried before it is saved, as the speaker's volume is. A position is sent only once the last has been answered, so that a fast drag does not queue up requests.

## Chip and Board
The config page, and ```/api/info``` where the API is enabled, say which board the firmware was built for and the chip it is running on, read from the chip itself: its model, revision, cores, flash and PSRAM. The board cannot be read from the hardware, only the chip.


# Development
The library compiles only for a board, so it is built through a board's crate, in ```boards```, e.g. ```cargo build --release``` in ```boards/seeed-esp32c6-xiao```, whose ```.cargo/config.toml``` has its target, ```MCU``` and ESP-IDF version.

```bruce-esp-idf.code-workspace``` is a VS Code workspace for working on the platform: it opens this repository and the libraries next to it, and has a task and a launch configuration for building a board's firmware and flashing it with the flasher's window. rust-analyzer can check only one board's code at a time, as the code behind each board's features differs, so ```rust-analyzer.linkedProjects``` names the board to check, with the commented out lines for the others to switch to. It also lists repositories which are the author's own setup, e.g. Bruce's Smart Home: to look at this repository alone, comment out the folders which are not checked out next to it.

# Building and Flashing
Firmware built for the wrong board drives the wrong pins, and can damage it, so each firmware image carries a record of the board it was built for, the firmware and its version, which a board crate adds with ```.with_firmware(bruce_esp_idf::firmware!())```. It starts with the magic ```bruce-embedded-std firmware 1```, and the firmware logs it as it starts, with the device's ID, its Wi-Fi MAC.

Flash with ```bruce-embedded-flash``` (from [bruce-embedded-flash](https://github.com/bruceskingle/bruce-embedded-flash), install it with ```cargo install --path bruce-embedded-flash``` in its checkout), never with espflash directly. Before it flashes, it reads the record from the new firmware and, over USB, from the firmware on the board, and flashes only if they are for the same board. If the board's firmware has no record, e.g. a new board, it asks which board it is, offering to restart it so that you can see which, and if it is for another board, or would replace other firmware, it refuses unless told otherwise. ```bruce-embedded-flash list``` lists the boards on USB without touching them, with what each was when last identified. The same checks are in a window, ```cargo run --features desktop``` in its ```bruce-embedded-flash-ui```, whose Dioxus component apps use to flash boards too, e.g. Bruce's Smart Home's Devices page.

Firmware is kept in a folder shared by every app which flashes these boards, ```~/Library/Application Support/bruce-embedded-std/firmware/``` on macOS, by the GitHub repository it comes from and its version, e.g. ```bruce-esp-idf/0.1.0/``` (bruce-embedded-flash's ```bruce-embedded-firmware```). What is built here goes in ```bruce-esp-idf/dev/```:

- ```cargo run --release``` in a board's directory builds its demo and flashes it, and its cargo runner, ```bruce-embedded-flash flash --monitor --keep-as bruce-esp-idf```, then copies it into ```bruce-esp-idf/dev/```, so the folder holds what was last flashed. A plain ```cargo build``` only builds into the board's ```target/```.
- ```./build.sh``` builds every board's demo, or those named, e.g. ```./build.sh seeed-esp32c6-xiao```, and copies each into ```bruce-esp-idf/dev/```.

Releases attach each board's firmware as an ELF file, and a repository with firmware has the GitHub topic ```bruce-embedded-firmware``` and a ```bruce-embedded-std.md``` in its root, whose first line, a heading, says what its firmware is, and the rest more about it. ```bruce-embedded-flash firmware find```, ```releases``` and ```download```, or Get Firmware in the window, find these repositories and download their releases into the shared folder.

# The Board Simulator
Firmware for a board which bruce-embedded-std's board simulator can simulate, so far ```board-wave-esp32s3-rlcd-42``` and ```board-guition-jc3248w535c```, also runs on a computer, with the ```sim``` feature in place of the default, ```espidf```: the board's screen is shown in a window, framed by the board, its buttons are clicked, and its touch screen is dragged on. See bruce-embedded-std's ```docs/Simulator.md```. ```boards/sim.sh <board>``` runs a board's demo in it, with the stable toolchain and the computer's own target, e.g. ```boards/sim.sh guition-jc3248w535c```.

The platform's code compiles for the computer as it is: it uses ESP-IDF's services and C API as ```crate::svc``` and ```crate::sys```, which are ```esp_idf_svc``` and ```esp_idf_sys``` on an ESP32, and the simulator's stand-ins for them, under the same names, with ```sim```. Only what is truly ESP-IDF's has a twin for the computer, in ```src/sim/```, chosen with ```#[cfg_attr(feature = "sim", path = ...)]```: the platform itself, which needs no Wi-Fi or setup portal, the core feature without its Wi-Fi settings, the identity, the buttons, the touch screen, the display and the speaker. Sensors a simulated board has none of read nothing. A thread's stack is given with ```bruce_embedded_std::thread_stack```, the device's size on an ESP32 and enough for a computer elsewhere.

# Supported Boards
The boards the platform supports, what each has and which of its hardware the platform drives, and the SD cards which
have been tried in them, are in [docs/Hardware.md](docs/Hardware.md).
