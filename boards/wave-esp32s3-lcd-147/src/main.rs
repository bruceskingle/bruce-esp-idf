//! The example for the Waveshare ESP32-S3-LCD-1.47B: an analog clock on the display, and a binary clock on a strip
//! of 64 WS2812 LEDs on the header's GPIO5, as the ESP32-C6-LCD-1.47's example has.

use bruce_embedded_std::DisplayOrientation;
use bruce_esp_idf::{
    Esp32Platform,
    features::{analog_clock::AnalogClock, binary_clock::BinaryClock, dyndns2::DynDns2},
};
use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

fn main() {
    // The app level fault barrier: for now the board restarts, by the panic.
    if let Err(error) = run() {
        log::error!("Application failed: {error:#}");
        panic!("App failed");
    }
}

fn run() -> anyhow::Result<()> {
    let (builder, remainder) = Esp32Platform::builder()?;

    let smart_leds = bruce_esp_idf::smart_led::new(remainder.rmt.channel0, remainder.gpio5, 64)?;

    let runner = builder
        .with_feature(Box::new(DynDns2::new()?))?
        .with_feature(Box::new(
            AnalogClock::builder()
                .with_layout(|rect| {
                    let margin = 3;
                    let size = std::cmp::min(rect.size.width, rect.size.height) - 2 * margin as u32;
                    Rectangle {
                        top_left: Point {
                            x: rect.top_left.x + margin,
                            y: rect.top_left.y + margin,
                        },
                        size: Size {
                            width: size,
                            height: size,
                        },
                    }
                })
                .build()?,
        ))?
        .with_display_orientation(DisplayOrientation::Rotate270)?
        .with_feature(Box::new(BinaryClock::new_rmt(smart_leds)))?
        .with_firmware(bruce_esp_idf::firmware!())
        .build()?;

    runner.start()
}
