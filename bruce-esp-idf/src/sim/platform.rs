//! Esp32Platform on a desktop computer, with the sim feature: the same builder, runner and platform as
//! esp32_platform.rs, so that a board's main and its features compile as they are, with the simulated board, see
//! sim/mod.rs, in place of the hardware. The computer is already on its network and has the time, so the platform
//! starts its features at once, with no Wi-Fi to join and no setup portal, and the device's API on its own ports.

use std::net::Ipv4Addr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use bruce_embedded_std::config::Config;
use bruce_embedded_std::config_manager::{ConfigManager, ConfigManagerBuilder};
#[cfg(feature = "display")]
use bruce_embedded_std::graphics::DisplayManager;
use bruce_embedded_std::http_server::HttpServerManager;
use bruce_embedded_std::platform::{Platform, PlatformInitializer};
use bruce_embedded_std::task::scheduler::{ScheduledTask, TaskScheduler, TaskSchedulerBuilder};
use bruce_embedded_std::{InitStatus, Status, problem::ProblemManager};
use chrono::Local;
use log::{error, info};

use crate::commands::EspCommands;
use crate::config_store::EspConfigStoreFactory;
use crate::core::Core;
use crate::http::EspHttpServerManager;
use crate::svc::http::Method;
use crate::svc::nvs::{EspNvsPartition, NvsDefault};
use crate::{DynFeature, Feature};

pub struct Esp32PlatformInitializer {
    task_manager_builder: TaskSchedulerBuilder<Esp32Platform>,
}

impl Esp32PlatformInitializer {
    fn new() -> Self {
        Self {
            task_manager_builder: TaskScheduler::builder(),
        }
    }
}

impl PlatformInitializer for Esp32PlatformInitializer {
    type Platform = Esp32Platform;

    fn add_task(
        &mut self,
        task_initializer: Box<dyn ScheduledTask<Esp32Platform>>,
        schedule_spec: &str,
    ) -> anyhow::Result<()> {
        self.task_manager_builder.add_task(task_initializer, schedule_spec)?;
        Ok(())
    }
}

/// The board's hardware the platform does not use itself: none, as a simulated board has only what the platform gives.
pub struct Remainder {}

type OnRunning = Box<dyn FnOnce(&mut Esp32Platform) -> anyhow::Result<()>>;

pub struct Esp32PlatformBuilder {
    nvs_partition: EspNvsPartition<NvsDefault>,
    ap_mode: Arc<Mutex<bool>>,
    config_manager_builder: ConfigManagerBuilder,
    features: Vec<FeatureHolder>,
    initializer: Esp32PlatformInitializer,
    core_feature: Core,
    core_feature_name: String,
    core_feature_config: Config,
    wifi_sender: Sender<Ipv4Addr>,
    firmware: crate::Firmware,
    #[cfg(feature = "key-button")]
    key_button: crate::push_button::PushButton,
    #[cfg(feature = "boot-button")]
    boot_push_button: crate::push_button::PushButton,
    #[cfg(feature = "speaker")]
    speaker: crate::speaker::Speaker,
    on_running: Vec<OnRunning>,
}

impl Esp32PlatformBuilder {
    fn new() -> anyhow::Result<(Self, Remainder)> {
        let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).try_init();
        crate::sim::start()?;
        let nvs_partition = crate::sim::nvs_partition()?;
        crate::starts::count(&nvs_partition);
        let port = crate::sim::port(&nvs_partition)?;
        bruce_embedded_sim::svc::server::set_default_ports(port, port);
        let problem_manager = ProblemManager::new();
        let ap_mode = Arc::new(Mutex::new(false));

        // The device's name on the network, which the core feature needs, and the user gives a real device when it is
        // set up: bruce-<board>-<the end of its ID>, the first time it starts, which can be changed on /config.
        {
            let core = crate::svc::nvs::EspNvs::new(nvs_partition.clone(), crate::core::CORE_FEATURE_NAME, true)?;
            if core.get_str("mdns_hostname", &mut [0; 64])?.is_none() {
                let device =
                    crate::svc::nvs::EspNvs::new(nvs_partition.clone(), crate::identity::DEVICE_NAMESPACE, true)?;
                let id = crate::identity::device_id(&device)?;
                core.set_str(
                    "mdns_hostname",
                    &format!("bruce-{}-{}", crate::sim::device().board.id, &id[id.len() - 4..]),
                )?;
            }
            // Its time zone, the computer's, if the platform has it, otherwise UTC, which can be changed likewise.
            if core.get_str("time_zone", &mut [0; 64])?.is_none() {
                let zone = iana_time_zone::get_timezone()
                    .ok()
                    .and_then(|zone| zone.parse::<bruce_embedded_std::tz::TimeZone>().ok())
                    .map_or("UTC", |zone| zone.to_str());
                core.set_str("time_zone", zone)?;
            }
        }

        let config_store_factory = EspConfigStoreFactory::new(nvs_partition.clone(), problem_manager.clone())?;
        let mut config_manager_builder = ConfigManager::builder(
            Box::new(config_store_factory),
            problem_manager.clone(),
            ap_mode.clone(),
            Box::new(EspCommands {}),
        )?;

        let mut initializer = Esp32PlatformInitializer::new();
        let (wifi_sender, wifi_receiver): (Sender<Ipv4Addr>, Receiver<Ipv4Addr>) = mpsc::channel();
        let core_feature = Core::new(wifi_receiver)?;
        let descriptor = core_feature.init(&mut initializer)?;
        let core_feature_name = descriptor.name.clone();
        let (core_feature_config, _core_config_valid) = config_manager_builder.add_feature(descriptor, true)?;

        let builder = Esp32PlatformBuilder {
            nvs_partition,
            ap_mode,
            config_manager_builder,
            features: Vec::new(),
            initializer,
            core_feature,
            core_feature_name,
            core_feature_config,
            wifi_sender,
            firmware: crate::Firmware {
                name: "unknown",
                version: "",
                record: None,
            },
            #[cfg(feature = "key-button")]
            key_button: crate::push_button::PushButton::new("KEY"),
            #[cfg(feature = "boot-button")]
            boot_push_button: crate::push_button::PushButton::new("BOOT"),
            #[cfg(feature = "speaker")]
            speaker: crate::speaker::Speaker::default(),
            on_running: Vec::new(),
        };
        #[cfg(feature = "shtc3")]
        let builder = {
            let mut builder = builder;
            builder.internal_add_feature(
                Box::new(crate::features::temperature_humidity::TemperatureHumidity),
                true,
            )?;
            builder
        };
        #[cfg(any(feature = "display-axs15231b", feature = "mipi-dsi-display"))]
        let builder = {
            let mut builder = builder;
            // The screen's settings, which the simulator keeps but does not act on, as its window is always lit.
            builder.internal_add_feature(Box::new(crate::features::screen::ScreenSettings), true)?;
            builder
        };
        #[cfg(feature = "speaker")]
        let builder = {
            let mut builder = builder;
            builder.internal_add_feature(Box::new(crate::features::speaker_settings::SpeakerSettings), true)?;
            if let Some(holder) = builder.features.last() {
                use bruce_embedded_std::config::FeatureConfig;
                match crate::features::speaker_settings::SpeakerConfig::from_config_spec(&holder.config.spec) {
                    Ok(config) => builder
                        .speaker
                        .set_volume(crate::features::speaker_settings::volume(&config)),
                    Err(error) => error!("Unable to read the speaker's volume: {error:#}"),
                }
            }
            builder
        };
        Ok((builder, Remainder {}))
    }

    /// The orientation of the display, which a simulated board shows as the real one is, so it is not changed.
    #[cfg(feature = "mipi-dsi-display")]
    pub fn with_display_orientation(
        self,
        _orientation: bruce_embedded_std::DisplayOrientation,
    ) -> anyhow::Result<Self> {
        Ok(self)
    }

    pub fn with_firmware(mut self, firmware: crate::Firmware) -> Self {
        info!("{} {}, simulated", firmware.name, firmware.version);
        self.firmware = firmware;
        self
    }

    pub fn with_feature(mut self, feature: Box<dyn DynFeature>) -> anyhow::Result<Self> {
        self.internal_add_feature(feature, false)?;
        Ok(self)
    }

    /// Adds a feature which is always enabled, as the platform's own are, e.g. one the firmware's screen is laid out
    /// for, such as a capture device's clock, so that it is not missing from a new device until it is enabled.
    pub fn with_required_feature(mut self, feature: Box<dyn DynFeature>) -> anyhow::Result<Self> {
        self.internal_add_feature(feature, true)?;
        Ok(self)
    }

    #[cfg(feature = "key-button")]
    pub fn with_key_button(
        mut self,
        on_start: impl FnOnce(&mut Esp32Platform) -> anyhow::Result<crate::push_button::Action> + 'static,
    ) -> Self {
        self.on_running.push(Box::new(move |platform| {
            let action = on_start(platform)?;
            platform.key_button().set_action(action);
            Ok(())
        }));
        self
    }

    #[cfg(feature = "boot-button")]
    pub fn with_boot_button(
        mut self,
        on_start: impl FnOnce(&mut Esp32Platform) -> anyhow::Result<crate::push_button::Action> + 'static,
    ) -> Self {
        self.on_running.push(Box::new(move |platform| {
            let action = on_start(platform)?;
            platform.boot_button().set_action(action);
            Ok(())
        }));
        self
    }

    fn internal_add_feature(&mut self, feature: Box<dyn DynFeature>, internal: bool) -> anyhow::Result<()> {
        let descriptor = feature.do_init(&mut self.initializer)?;
        let name = descriptor.name.clone();
        let (config, _valid) = self.config_manager_builder.add_feature(descriptor, internal)?;
        self.features.push(FeatureHolder { feature, config, name });
        Ok(())
    }

    pub fn build(self) -> anyhow::Result<Esp32PlatformRunner> {
        #[cfg(feature = "display")]
        let mut display_manager = crate::display::display_manager()?;
        #[cfg(feature = "display")]
        display_manager.set_status(&Status::Initializing(InitStatus::Starting))?;
        #[cfg(feature = "touch-driver")]
        let touch_driver = crate::touch::TouchDriver::start()?;

        let mut server_manager = EspHttpServerManager::new()?;
        server_manager.init_common_pages()?;

        let config_manager = Arc::new(self.config_manager_builder.build());
        config_manager.set_about(format!(
            "{} {}, built for a {} board, running on a {}",
            self.firmware.name,
            self.firmware.version,
            crate::board_name(),
            crate::chip::chip().describe()
        ));
        let adoption = crate::adoption::Adoption::load(&self.nvs_partition, config_manager.clone())?;
        ConfigManager::create_pages(&config_manager, &mut server_manager)?;
        #[cfg(feature = "speaker")]
        crate::features::speaker_settings::add_preview(&mut server_manager, self.speaker.clone())?;

        let firmware = self.firmware;
        server_manager.on("/", Method::Get, move |request| {
            let mut response = request.into_ok_response()?;
            let time = Local::now().format("%Y-%m-%d %H:%M:%S");
            let board = crate::board_name();
            let crate::Firmware { name, version, .. } = firmware;
            crate::svc::io::Write::write_all(
                &mut response,
                format!(
                    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{name}</title>
    <link rel="stylesheet" href="/main.css">
</head>
<body>
    <div class="page">
        <h1>{name}</h1>
        <section class="feature">
            <p>{name} {version} on a simulated {board}</p>
            <p>The time is {time}</p>
        </section>
        <p><a href="/config">Settings</a></p>
    </div>
</body>
</html>
"#
                )
                .as_bytes(),
            )?;
            Ok(())
        })?;

        Ok(Esp32PlatformRunner {
            platform: Esp32Platform {
                #[cfg(feature = "key-button")]
                key_button: self.key_button,
                #[cfg(feature = "boot-button")]
                boot_push_button: self.boot_push_button,
                #[cfg(feature = "speaker")]
                speaker: self.speaker,
                on_running: self.on_running,
                config_manager,
                server_manager,
                features: self.features,
                ap_mode: self.ap_mode,
                nvs_partition: self.nvs_partition,
                firmware: self.firmware,
                mdns_txt: Default::default(),
                ip_address: None,
                wifi_sender: self.wifi_sender,
                api_server: None,
                adoption,
                #[cfg(feature = "display")]
                display_manager: Arc::new(Mutex::new(display_manager)),
                #[cfg(feature = "touch-driver")]
                touch_driver,
            },
            initializer: self.initializer,
            core_feature_holder: FeatureHolder {
                feature: Box::new(self.core_feature),
                config: self.core_feature_config,
                name: self.core_feature_name,
            },
        })
    }
}

struct FeatureHolder {
    feature: Box<dyn DynFeature>,
    config: Config,
    name: String,
}

pub struct Esp32PlatformRunner {
    pub platform: Esp32Platform,
    initializer: Esp32PlatformInitializer,
    core_feature_holder: FeatureHolder,
}

impl Esp32PlatformRunner {
    /// Runs the platform, on a thread of its own, and the simulated board's window on this one, until the window is
    /// closed, when the program ends, as the real platform's start never returns.
    pub fn start(self) -> anyhow::Result<()> {
        let Esp32PlatformRunner {
            mut platform,
            initializer,
            core_feature_holder,
        } = self;
        // SAFETY: the platform, its features, and everything they hold, are moved whole to the firmware's thread, and
        // this one only runs the window from now on, see run_device.
        unsafe {
            bruce_embedded_sim::run_device(crate::sim::device().clone(), crate::sim::options(), move || {
                platform.start(initializer, core_feature_holder)
            })
        }
    }
}

pub struct Esp32Platform {
    /// The facts of the device's mDNS service's TXT record, which the platform sets, see crate::mdns_services, and the
    /// firmware may add to, see mdns_txt.
    mdns_txt: bruce_embedded_std::mdns::MdnsTxt,
    #[cfg(feature = "key-button")]
    key_button: crate::push_button::PushButton,
    #[cfg(feature = "boot-button")]
    boot_push_button: crate::push_button::PushButton,
    #[cfg(feature = "speaker")]
    speaker: crate::speaker::Speaker,
    on_running: Vec<OnRunning>,
    pub config_manager: Arc<ConfigManager>,
    pub server_manager: EspHttpServerManager<'static>,
    features: Vec<FeatureHolder>,
    pub ap_mode: Arc<Mutex<bool>>,
    nvs_partition: EspNvsPartition<NvsDefault>,
    firmware: crate::Firmware,
    ip_address: Option<Ipv4Addr>,
    /// Gives the core feature's mDNS responder the address, as the Wi-Fi does on the device.
    wifi_sender: Sender<Ipv4Addr>,
    pub api_server: Option<crate::api::ApiServer>,
    adoption: Arc<crate::adoption::Adoption>,
    #[cfg(feature = "display")]
    pub display_manager: Arc<Mutex<crate::display::BoardDisplayManager>>,
    #[cfg(feature = "touch-driver")]
    pub touch_driver: crate::touch::TouchDriver,
}

impl Platform for Esp32Platform {}

impl Esp32Platform {
    pub fn builder() -> anyhow::Result<(Esp32PlatformBuilder, Remainder)> {
        Esp32PlatformBuilder::new()
    }

    pub fn api_get<F>(&mut self, uri: &str, f: F) -> anyhow::Result<()>
    where
        F: for<'r> Fn(
                crate::svc::http::server::Request<&mut crate::svc::http::server::EspHttpConnection<'r>>,
            ) -> anyhow::Result<()>
            + Send
            + Sync
            + 'static,
    {
        if let Some(api_server) = self.api_server.as_mut() {
            api_server.get(&mut self.server_manager, uri, f)?;
        }
        Ok(())
    }

    /// Adds a handler for reading which may take a while, as the device's does, see ApiServer::get_bulk; served by the
    /// simulator's one HTTP server, as its devices share a computer's ports.
    pub fn api_get_bulk<F>(&mut self, uri: &str, f: F) -> anyhow::Result<()>
    where
        F: for<'r> Fn(
                crate::svc::http::server::Request<&mut crate::svc::http::server::EspHttpConnection<'r>>,
            ) -> anyhow::Result<()>
            + Send
            + Sync
            + 'static,
    {
        if let Some(api_server) = self.api_server.as_mut() {
            api_server.get_bulk(&mut self.server_manager, uri, f)?;
        }
        Ok(())
    }

    /// Adds a handler for reading which needs access, see ApiServer::get_as, to the API, if it is running, so also over
    /// plain HTTP, e.g. for what only those who manage the device may read.
    pub fn api_get_as<F>(&mut self, uri: &str, access: crate::access::Access, f: F) -> anyhow::Result<()>
    where
        F: for<'r> Fn(
                crate::svc::http::server::Request<&mut crate::svc::http::server::EspHttpConnection<'r>>,
            ) -> anyhow::Result<()>
            + Send
            + Sync
            + 'static,
    {
        if let Some(api_server) = self.api_server.as_mut() {
            api_server.get_as(&mut self.server_manager, uri, access, f)?;
        }
        Ok(())
    }

    /// Adds a handler of POSTs which needs access, given the request's body, see ApiServer::post, to the API, if it is
    /// running, over HTTPS only.
    pub fn api_post<F>(&mut self, uri: &str, access: crate::access::Access, f: F) -> anyhow::Result<()>
    where
        F: for<'r> Fn(
                crate::svc::http::server::Request<&mut crate::svc::http::server::EspHttpConnection<'r>>,
                Vec<u8>,
            ) -> anyhow::Result<()>
            + Send
            + 'static,
    {
        if let Some(api_server) = self.api_server.as_mut() {
            api_server.post(uri, access, f)?;
        }
        Ok(())
    }

    /// Has authoriser decide the API requests in its scheme, e.g. those signed by the site's principals, see
    /// access.rs; the adopting app's key goes on working alongside it.
    pub fn set_authoriser(&self, authoriser: std::sync::Arc<dyn crate::access::Authoriser>) {
        self.adoption.set_authoriser(authoriser);
    }

    /// The NVS partition, for firmware to keep what must not be on an SD card in a namespace of its own, e.g. a
    /// private key.
    pub fn nvs_partition(&self) -> crate::svc::nvs::EspNvsPartition<crate::svc::nvs::NvsDefault> {
        self.nvs_partition.clone()
    }

    pub fn device_id(&self) -> Option<String> {
        self.api_server
            .as_ref()
            .map(|api_server| api_server.identity().id.clone())
    }

    pub fn hostname(&self) -> Option<String> {
        self.config_manager
            .value(crate::core::CORE_FEATURE_NAME, "mdns_hostname")
    }

    pub fn ip_address(&self) -> Option<Ipv4Addr> {
        self.ip_address
    }

    /// The address an app reaches the device at, its IP address and its port, for HTTP and HTTPS alike, see
    /// sim::port, as an app is given it, e.g. 192.168.1.190:60486.
    pub fn address(&self) -> Option<String> {
        let port = crate::sim::port(&self.nvs_partition).ok()?;
        self.ip_address.map(|address| format!("{address}:{port}"))
    }

    /// The facts of the device's mDNS service's TXT record, e.g. for the firmware to add its own, as `key=value`, each
    /// change announced at once, see crate::mdns_services.
    pub fn mdns_txt(&self) -> &bruce_embedded_std::mdns::MdnsTxt {
        &self.mdns_txt
    }

    /// The firmware's name and version, see with_firmware.
    pub fn firmware(&self) -> crate::Firmware {
        self.firmware
    }

    pub fn is_adopted(&self) -> bool {
        self.adoption.is_adopted()
    }

    #[cfg(feature = "speaker")]
    pub fn speaker(&self) -> crate::speaker::Speaker {
        self.speaker.clone()
    }

    /// The board's battery, which a simulated board has none of.
    #[cfg(feature = "battery")]
    pub fn battery(&self) -> Option<Arc<crate::battery::Battery>> {
        None
    }

    #[cfg(feature = "shtc3")]
    pub fn shtc3(&self) -> crate::shtc3::Shtc3 {
        crate::shtc3::Shtc3
    }

    #[cfg(feature = "key-button")]
    pub fn key_button(&self) -> &crate::push_button::PushButton {
        &self.key_button
    }

    #[cfg(feature = "boot-button")]
    pub fn boot_button(&self) -> &crate::push_button::PushButton {
        &self.boot_push_button
    }

    /// Where the SD card is: the simulated device's folder for it.
    pub fn sd_card(&self) -> Result<&'static str, String> {
        #[cfg(feature = "sd-card")]
        {
            static FOLDER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            return Ok(FOLDER.get_or_init(|| crate::sim::sd_folder().display().to_string()));
        }
        #[cfg(not(feature = "sd-card"))]
        Err("this firmware has no SD card support".to_string())
    }

    pub fn set_status(&mut self, status: Status) -> anyhow::Result<()> {
        self.show_status(status, &[])
    }

    #[allow(unused_variables)]
    fn show_status(&mut self, status: Status, details: &[String]) -> anyhow::Result<()> {
        #[cfg(feature = "display")]
        self.display_manager.lock().unwrap().show_status(&status, details)?;
        Ok(())
    }

    fn whereabouts(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(address) = self.address() {
            lines.push(format!("Address {address}"));
        }
        if let Some(name) = self.hostname().filter(|name| !name.is_empty()) {
            lines.push(format!("Name {name}"));
        }
        lines
    }

    fn start_feature(&mut self, mut feature_holder: FeatureHolder, initializer: &mut Esp32PlatformInitializer) {
        if feature_holder.config.enabled.is_enabled() {
            match feature_holder
                .feature
                .start_with_config(self, initializer, &feature_holder.config.spec)
            {
                Ok(_) => info!("Started Feature {}", feature_holder.name),
                Err(error) => error!("FAILED to start Feature {}: {}", feature_holder.name, error),
            }
        } else {
            info!("Feature {} is disabled", feature_holder.name)
        }
        self.features.push(feature_holder);
    }

    /// Starts the device as the real one does once it has joined the Wi-Fi and has the time: the API, the start's
    /// record, the app's own set up, the features, then their tasks, which never end.
    fn start(
        &mut self,
        mut initializer: Esp32PlatformInitializer,
        core_feature_holder: FeatureHolder,
    ) -> anyhow::Result<()> {
        let ip_address = crate::sim::ip_address();
        self.ip_address = Some(ip_address);
        let _ = self.wifi_sender.send(ip_address);
        self.show_status(Status::Initializing(InitStatus::StartingFeatures), &self.whereabouts())?;
        if let Some(address) = self.address() {
            info!("Simulated device at {address}, its address for an app");
            // For the window's title, and for the user to copy, see bruce-embedded-sim's Device::set_address.
            crate::sim::device().set_address(address);
        }

        match crate::api::ApiServer::start(
            &mut self.server_manager,
            &self.nvs_partition,
            self.config_manager.clone(),
            self.adoption.clone(),
            self.firmware,
        ) {
            Ok(api_server) => {
                #[cfg(feature = "display")]
                initializer.add_task(
                    Box::new(crate::adoption::AdoptionOverlay::new(
                        &self.display_manager,
                        self.adoption.clone(),
                        &api_server.identity().fingerprint,
                    )),
                    "* * * * * *",
                )?;
                self.api_server = Some(api_server);
            }
            Err(error) => error!("Unable to start the API server: {error:#}"),
        }

        crate::starts::record(&self.nvs_partition, self.firmware);
        {
            let nvs_partition = self.nvs_partition.clone();
            let firmware = self.firmware;
            self.api_get_as("/api/starts", crate::access::Access::Manage, move |request| {
                let mut response = request.into_response(200, None, &[("Content-Type", "application/json")])?;
                crate::starts::write_json(&nvs_partition, firmware, &mut |bytes| {
                    Ok(crate::svc::io::Write::write_all(&mut response, bytes)?)
                })
            })?;
        }

        for on_running in std::mem::take(&mut self.on_running) {
            if let Err(error) = on_running(self) {
                error!("FAILED to set up {error:#}");
            }
        }

        let features = std::mem::take(&mut self.features);
        self.features = Vec::with_capacity(features.len() + 1);
        self.start_feature(core_feature_holder, &mut initializer);
        for feature_holder in features {
            self.start_feature(feature_holder, &mut initializer);
        }

        let mut task_manager = initializer.task_manager_builder.build();
        self.set_status(Status::Running)?;
        task_manager.run(self)
    }
}
