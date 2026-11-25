//! The example for the Waveshare ESP32-S3-DualEye-LCD-1.28, without touch: an analog clock on the left screen, LCD1,
//! the platform's display, and an eye on the right, LCD2, see RightEye, which shows that both screens work, which way
//! up the right one is, by the word TOP, and that its colours are right, a brown iris. BOOT plays a beep on the
//! speaker, and it plays a chime as it starts, sounds/Startup.wav, the RLCD-4.2 example's, built into the firmware.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bruce_embedded_std::config::{ConfigSpec, FeatureConfig};
use bruce_embedded_std::feature::FeatureDescriptor;
use bruce_embedded_std::graphics::DisplayManager;
use bruce_embedded_std::platform::PlatformInitializer;
use bruce_embedded_std::task::scheduler::ScheduledTask;
use bruce_esp_idf::{
    BoardDisplayManager, Esp32Platform, Esp32PlatformInitializer, Feature, features::analog_clock::AnalogClock,
};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::{Alignment, Text},
};
use log::error;

/// Played as the board starts.
static STARTUP_SOUND: &[u8] = include_bytes!("../sounds/Startup.wav");

fn main() {
    // The app level fault barrier: for now the board restarts, by the panic.
    if let Err(error) = run() {
        log::error!("Application failed: {error:#}");
        panic!("App failed");
    }
}

fn run() -> anyhow::Result<()> {
    let (builder, _remainder) = Esp32Platform::builder()?;

    let runner = builder
        .with_feature(Box::new(AnalogClock::builder().build()?))?
        .with_feature(Box::new(RightEye))?
        .with_boot_button(|platform| {
            let speaker = platform.speaker();
            Ok(Box::new(move || {
                speaker.play_tone(880.0, Duration::from_millis(120))?;
                Ok(())
            }))
        })
        .with_firmware(bruce_esp_idf::firmware!())
        .build()?;

    // At once, while the board joins the network, so in a thread of its own, at the volume set on the /config page.
    let speaker = runner.platform.speaker();
    std::thread::Builder::new().stack_size(8192).spawn(move || {
        if let Err(error) = speaker.play_wav(STARTUP_SOUND) {
            error!("Unable to play the startup sound: {error:#}");
        }
    })?;

    runner.start()
}

#[derive(FeatureConfig)]
#[config(label = "Right Eye")]
pub struct RightEyeConfig {}

/// Draws an eye on the right screen, see the module comment.
pub struct RightEye;

impl Feature for RightEye {
    type Config = RightEyeConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor {
            name: "RightEye".to_string(),
            config: RightEyeConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        platform: &mut Esp32Platform,
        initializer: &mut Esp32PlatformInitializer,
        _config: RightEyeConfig,
    ) -> anyhow::Result<()> {
        initializer.add_task(
            Box::new(DrawTask {
                display_manager: platform.display_manager.clone(),
                drawn: None,
            }),
            "* * * * * *",
        )?;
        Ok(())
    }
}

/// Draws the eye whenever the screens have been cleared, e.g. when the platform has started, or after adoption's
/// pairing code, see DisplayManager::generation.
struct DrawTask {
    display_manager: Arc<Mutex<BoardDisplayManager>>,
    drawn: Option<u32>,
}

impl ScheduledTask<Esp32Platform> for DrawTask {
    fn name(&self) -> &str {
        "Right Eye"
    }

    fn run(&mut self, _platform: &mut Esp32Platform) -> anyhow::Result<()> {
        let mut display_manager = self.display_manager.lock().unwrap();
        if display_manager.overlay_active() || self.drawn == Some(display_manager.generation()) {
            return Ok(());
        }
        self.drawn = Some(display_manager.generation());
        let display = display_manager.right();
        let centre = display.bounding_box().center();
        let fill = |colour| PrimitiveStyle::with_fill(colour);
        // The white, the iris, brown, the pupil, and a highlight, up and to the left.
        Circle::with_center(centre, 220)
            .into_styled(fill(Rgb565::WHITE))
            .draw(display)?;
        Circle::with_center(centre, 100)
            .into_styled(fill(Rgb565::new(18, 22, 4)))
            .draw(display)?;
        Circle::with_center(centre, 44)
            .into_styled(fill(Rgb565::BLACK))
            .draw(display)?;
        Circle::with_center(centre + Point::new(-14, -14), 14)
            .into_styled(fill(Rgb565::WHITE))
            .draw(display)?;
        let text = MonoTextStyle::new(&FONT_10X20, Rgb565::BLACK);
        Text::with_alignment("TOP", Point::new(centre.x, 44), text, Alignment::Center).draw(display)?;
        Ok(())
    }
}
