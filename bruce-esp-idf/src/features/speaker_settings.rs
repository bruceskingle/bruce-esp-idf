//! The board's speaker's settings: its volume, which every sound plays at, see speaker.rs. The platform adds it on a
//! board with a speaker, see the speaker feature.

use crate::svc::http::Method;
use bruce_embedded_std::config::{ConfigSpec, ConfigSpecValue, FeatureConfig, TypedValue};
// Not needed with the simulator's Request, whose read is its own.
#[cfg_attr(feature = "sim", allow(unused_imports))]
use crate::svc::io::Read;
use log::warn;

use crate::http::EspHttpServerManager;
use crate::speaker::Speaker;
use crate::{Esp32Platform, Esp32PlatformInitializer, Feature, FeatureDescriptor};

/// The volume, in percent, until one is saved: the codec's full level, but three quarters of it on the Waveshare
/// ESP32-S3-DualEye-LCD-1.28, whose speaker is loud enough at full level to be too loud.
#[cfg(feature = "board-wave-esp32s3-dualeye-128")]
pub const DEFAULT_VOLUME: u8 = 75;
#[cfg(not(feature = "board-wave-esp32s3-dualeye-128"))]
pub const DEFAULT_VOLUME: u8 = 100;

#[derive(FeatureConfig)]
#[config(label = "Speaker")]
pub struct SpeakerConfig {
    /// In percent, see Speaker::set_volume. DEFAULT_VOLUME if not set. A slider, which plays a sound at each volume it is moved to,
    /// see PREVIEW, a beep unless the firmware chose another, see Speaker::set_preview_wav.
    #[config(
        label = "Volume (%)",
        min = 0,
        max = 100,
        default = DEFAULT_VOLUME,
        preview = "/speaker/preview"
    )]
    pub volume: Option<i32>,
}

pub struct SpeakerSettings;

impl Feature for SpeakerSettings {
    type Config = SpeakerConfig;

    fn init(&self, _initializer: &mut Esp32PlatformInitializer) -> anyhow::Result<FeatureDescriptor> {
        Ok(FeatureDescriptor {
            name: "Speaker".to_string(),
            config: SpeakerConfig::to_config_spec()?,
        })
    }

    fn start(
        &mut self,
        platform: &mut Esp32Platform,
        _initializer: &mut Esp32PlatformInitializer,
        config: SpeakerConfig,
    ) -> anyhow::Result<()> {
        platform.speaker().set_volume(volume(&config));
        Ok(())
    }
}

/// The volume config sets, in percent, DEFAULT_VOLUME if it is not set.
pub(crate) fn volume(config: &SpeakerConfig) -> u8 {
    config.volume.unwrap_or(DEFAULT_VOLUME as i32).clamp(0, 100) as u8
}

/// The config page's volume slider POSTs each volume it is moved to here, as value=N, see Slider::preview.
const PREVIEW: &str = "/speaker/preview";

/// Plays a short tone at each volume POSTed to PREVIEW, without saving it, so that the slider's volume can be heard
/// before it is saved. Added with the config page, so that it works while the device is being set up too.
pub(crate) fn add_preview(server_manager: &mut EspHttpServerManager<'static>, speaker: Speaker) -> anyhow::Result<()> {
    server_manager.on(PREVIEW, Method::Post, move |mut request| {
        let mut body = [0u8; 32];
        let mut length = 0;
        while length < body.len() {
            match request.read(&mut body[length..])? {
                0 => break,
                read => length += read,
            }
        }
        let volume = url::form_urlencoded::parse(&body[..length])
            .find(|(name, _)| name == "value")
            .and_then(|(_, value)| value.parse::<i32>().ok());
        match volume {
            Some(volume) => {
                if let Err(error) = speaker.preview(volume.clamp(0, 100) as u8) {
                    warn!("Unable to play the volume's preview: {error:#}");
                }
                request.into_ok_response()?;
            }
            None => {
                request.into_status_response(400)?;
            }
        }
        Ok(())
    })?;
    Ok(())
}
