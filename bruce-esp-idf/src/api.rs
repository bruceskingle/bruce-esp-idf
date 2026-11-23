//! The device's API, for an app, e.g. Bruce's Smart Home, to find out what the device is and manage it, over HTTPS with the
//! device's own certificate, see identity.rs, which the app pins. It runs alongside the HTTP server of the /config page
//! and captive portal, on port 443, once the device is on the Wi-Fi.
//!
//! - GET /api/info: what the device is, see info.
//! - POST /api/adopt/start, then POST /api/adopt {"code", "key"}: adoption, see adoption.rs.
//! - POST /api/release
//! - GET /api/config: every feature's config, see ConfigManager::to_json, with secrets only as set or not.
//! - POST /api/config: changes it, see ConfigManager::apply_json, which takes effect after a restart.
//! - GET /api/memory: where the RAM is, see memory.rs.
//! - GET /api/starts: when and why the device started, see starts.rs, also over plain HTTP, see get.
//! - POST /api/restart
//!
//! Features add their own, e.g. the data they capture, with on, or, for reading, with get, which also serves it over
//! plain HTTP, on port 80, for requests signed with the key, see Adoption::is_signed, as HTTPS needs RAM the device may
//! not have to spare while it is polling over HTTPS itself. /api/info is on port 80 too. Once the device has been
//! adopted, every request but /api/info and adoption's needs the adopting app's key, see Adoption::is_authorised.

use std::sync::Arc;

use crate::svc::http::Method;
use crate::svc::http::server::{Configuration, EspHttpConnection, EspHttpServer, Request};
use crate::svc::io::Write;
use crate::svc::nvs::{EspNvsPartition, NvsDefault};
use crate::svc::tls::X509;
use bruce_embedded_std::config_manager::ConfigManager;
use log::info;
use serde_json::{Value, json};

use crate::Firmware;
use crate::adoption::{Adoption, PAIRING_TIME};
use crate::http::EspHttpServerManager;
use crate::identity::DeviceIdentity;

/// The largest request body read, to protect the RAM.
const MAX_BODY: usize = 8 * 1024;

/// The port of the plain HTTP server for slow reads, see ApiServer::get_bulk, and its control port, after the HTTP
/// server's 32768 and the API's 32769.
pub const BULK_PORT: u16 = 8080;
#[cfg(not(feature = "sim"))]
const BULK_CTRL_PORT: u16 = 32770;
/// How many connections the bulk server serves at once: the app's, and a peer's or two, see CaptureDevicePlan.md.
#[cfg(not(feature = "sim"))]
const BULK_CONNECTIONS: usize = 3;

pub struct ApiServer {
    server: EspHttpServer<'static>,
    identity: Arc<DeviceIdentity>,
    adoption: Arc<Adoption>,
    /// The plain HTTP server of the slow reads, e.g. a store's change feed, on BULK_PORT, a task of its own, so that a
    /// page which takes a while does not stop the HTTP server answering everything else, as ESP-IDF's server answers
    /// one request at a time, see get_bulk. None in the simulator, whose devices share a computer's ports, or if it
    /// did not start, when they are served by the HTTP server, as before.
    bulk: Option<EspHttpServerManager<'static>>,
}

impl ApiServer {
    pub(crate) fn start(
        http: &mut EspHttpServerManager<'static>,
        nvs_partition: &EspNvsPartition<NvsDefault>,
        config_manager: Arc<ConfigManager>,
        adoption: Arc<Adoption>,
        firmware: Firmware,
    ) -> anyhow::Result<ApiServer> {
        let identity = Arc::new(DeviceIdentity::load_or_create(nvs_partition)?);
        #[cfg(not(feature = "sim"))]
        let bulk = EspHttpServerManager::on_port(BULK_PORT, BULK_CTRL_PORT, BULK_CONNECTIONS)
            .map_err(|error| {
                log::warn!("Unable to start the HTTP server for slow reads on port {BULK_PORT}, {error:#}")
            })
            .ok();
        #[cfg(feature = "sim")]
        let bulk: Option<EspHttpServerManager<'static>> = None;
        let bulk_port = bulk.as_ref().map(|_| BULK_PORT);
        let mut server = EspHttpServer::new(&Configuration {
            // Its own control port, as the HTTP server has the default one.
            ctrl_port: 32769,
            // Each HTTPS connection needs about 30 KB while it is open, see sdkconfig.defaults, so only one, which an app
            // can share by making its requests one at a time, as bruce-smart-home does, and a new one closes the oldest.
            max_open_sockets: 1,
            // It needed about 5 KB, with a TLS handshake, of the default 10 KB.
            stack_size: 7168,
            server_certificate: Some(X509::pem(identity.cert_pem)),
            private_key: Some(X509::pem(identity.key_pem)),
            ..Default::default()
        })?;

        let info_identity = identity.clone();
        let info_config = config_manager.clone();
        let info_adoption = adoption.clone();
        server.fn_handler("/api/info", Method::Get, move |request| {
            respond(
                request,
                200,
                &info(&info_identity, &info_config, &info_adoption, firmware, bulk_port),
            )
        })?;
        let info_identity = identity.clone();
        let info_config = config_manager.clone();
        let info_adoption = adoption.clone();
        http.on("/api/info", Method::Get, move |request| {
            respond(
                request,
                200,
                &info(&info_identity, &info_config, &info_adoption, firmware, bulk_port),
            )
        })?;

        let start_adoption = adoption.clone();
        server.fn_handler("/api/adopt/start", Method::Post, move |request| {
            match start_adoption.start_pairing() {
                Ok(time) => respond(request, 200, &json!({ "expiresInSeconds": time.as_secs() })),
                Err(error) => respond(request, 409, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let adopt_adoption = adoption.clone();
        server.fn_handler("/api/adopt", Method::Post, move |mut request| {
            let result = read_json(&mut request).and_then(|body| {
                let text = |name: &str| {
                    body.get(name)
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string()
                };
                adopt_adoption.adopt(&text("code"), &text("key"))
            });
            match result {
                Ok(()) => respond(request, 200, &json!({ "adopted": true })),
                Err(error) => respond(request, 403, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let release_adoption = adoption.clone();
        server.fn_handler("/api/release", Method::Post, move |request| {
            if !authorised(&release_adoption, &request) {
                return unauthorised(request);
            }
            match release_adoption.release() {
                Ok(()) => respond(request, 200, &json!({ "adopted": false })),
                Err(error) => respond(request, 500, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let get_config = config_manager.clone();
        let get_adoption = adoption.clone();
        server.fn_handler("/api/config", Method::Get, move |request| {
            if !authorised(&get_adoption, &request) {
                return unauthorised(request);
            }
            respond(request, 200, &get_config.to_json())
        })?;

        let post_config = config_manager;
        let post_adoption = adoption.clone();
        server.fn_handler("/api/config", Method::Post, move |mut request| {
            if !authorised(&post_adoption, &request) {
                return unauthorised(request);
            }
            let update = match read_json(&mut request) {
                Ok(update) => update,
                Err(error) => return respond(request, 400, &json!({ "error": format!("{error:#}") })),
            };
            match post_config.apply_json(&update) {
                Ok(()) => {
                    info!("Config changed through the API");
                    respond(
                        request,
                        200,
                        &json!({ "restartRequired": true, "config": post_config.to_json() }),
                    )
                }
                Err(error) => respond(request, 400, &json!({ "error": format!("{error:#}") })),
            }
        })?;

        let memory_adoption = adoption.clone();
        server.fn_handler("/api/memory", Method::Get, move |request| {
            if !authorised(&memory_adoption, &request) {
                return unauthorised(request);
            }
            respond(request, 200, &crate::memory::to_json())
        })?;

        let restart_adoption = adoption.clone();
        server.fn_handler("/api/restart", Method::Post, move |request| {
            if !authorised(&restart_adoption, &request) {
                return unauthorised(request);
            }
            respond(request, 200, &json!({ "restarting": true }))?;
            // After the response has gone.
            // A small stack, as the default may not fit in the RAM which is left.
            std::thread::Builder::new()
                .stack_size(bruce_embedded_std::thread_stack(4096))
                .spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    crate::restart();
                })?;
            Ok::<(), anyhow::Error>(())
        })?;

        info!(
            "API server started on port 443, pairing codes last {} s",
            PAIRING_TIME.as_secs()
        );
        Ok(ApiServer {
            server,
            identity,
            adoption,
            bulk,
        })
    }

    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    pub fn adoption(&self) -> &Arc<Adoption> {
        &self.adoption
    }

    /// Adds a handler, e.g. for a feature's data, whose uri should start /api/, which, once the device has been
    /// adopted, is only called for requests with the adopting app's key.
    pub fn on<F>(&mut self, uri: &str, method: Method, f: F) -> anyhow::Result<&mut Self>
    where
        F: for<'r> Fn(Request<&mut EspHttpConnection<'r>>) -> anyhow::Result<()> + Send + 'static,
    {
        let adoption = self.adoption.clone();
        self.server.fn_handler(uri, method, move |request| {
            if !authorised(&adoption, &request) {
                return unauthorised(request);
            }
            f(request)
        })?;
        Ok(self)
    }

    /// Adds a handler for reading, see on, which is also served over plain HTTP, by http, for requests signed with the
    /// adopting app's key, see Adoption::is_signed.
    pub fn get<F>(&mut self, http: &mut EspHttpServerManager<'static>, uri: &str, f: F) -> anyhow::Result<&mut Self>
    where
        F: for<'r> Fn(Request<&mut EspHttpConnection<'r>>) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        let adoption = self.adoption.clone();
        let signed = f.clone();
        http.on(uri, Method::Get, move |request| {
            if !adoption.is_signed("GET", request.uri(), request.header("Authorization")) {
                return unauthorised(request);
            }
            signed(request)
        })?;
        self.on(uri, Method::Get, move |request| f(request))
    }

    /// Adds a handler for reading which may take a while, e.g. a page of a store's change feed, as get does, but served
    /// over plain HTTP by the bulk server, on BULK_PORT, see ApiServer::bulk, so that it does not hold up the HTTP
    /// server; by http if there is none. /api/info gives the port as bulkPort.
    pub fn get_bulk<F>(
        &mut self,
        http: &mut EspHttpServerManager<'static>,
        uri: &str,
        f: F,
    ) -> anyhow::Result<&mut Self>
    where
        F: for<'r> Fn(Request<&mut EspHttpConnection<'r>>) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        match self.bulk.take() {
            Some(mut bulk) => {
                let result = self.get(&mut bulk, uri, f).map(|_| ());
                self.bulk = Some(bulk);
                result?;
                Ok(self)
            }
            None => self.get(http, uri, f),
        }
    }
}

fn authorised(adoption: &Adoption, request: &Request<&mut EspHttpConnection<'_>>) -> bool {
    adoption.is_authorised(request.header("Authorization"))
}

pub(crate) fn unauthorised(request: Request<&mut EspHttpConnection<'_>>) -> anyhow::Result<()> {
    respond(
        request,
        401,
        &json!({ "error": "the device has been adopted, and this needs the adopting app's key" }),
    )
}

/// What the device is: its ID, board, firmware, certificate fingerprint, whether it has been adopted, how long it has
/// been running, why it last started, and if it crashed, why, see panics.rs, its memory, and its features.
fn info(
    identity: &DeviceIdentity,
    config_manager: &ConfigManager,
    adoption: &Adoption,
    firmware: Firmware,
    bulk_port: Option<u16>,
) -> Value {
    let features: Vec<Value> = config_manager
        .features
        .values()
        .map(|feature| {
            let config = feature.inner.lock().unwrap();
            json!({ "name": feature.name, "label": config.spec.label, "enabled": config.enabled.is_enabled() })
        })
        .collect();
    json!({
        "id": identity.id,
        "hostname": config_manager.value(crate::core::CORE_FEATURE_NAME, "mdns_hostname"),
        "board": crate::reported_board_name(),
        // The chip, as it says it is, whichever board the firmware was built for.
        "chip": crate::chip::chip().to_json(),
        "firmware": { "name": firmware.name, "version": firmware.version },
        "platform": { "name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION") },
        "certificateFingerprint": identity.fingerprint,
        "adopted": adoption.is_adopted(),
        "upTimeSeconds": unsafe { crate::sys::esp_timer_get_time() } / 1_000_000,
        "resetReason": crate::reset_reason(),
        // Where the slow reads are served, see ApiServer::get_bulk; on the HTTP port if there is none.
        "bulkPort": bulk_port,
        "lastCrash": crate::panics::last(),
        "freeHeap": unsafe { crate::sys::esp_get_free_heap_size() },
        "minimumFreeHeap": unsafe { crate::sys::esp_get_minimum_free_heap_size() },
        "largestFreeBlock": unsafe { crate::sys::heap_caps_get_largest_free_block(crate::sys::MALLOC_CAP_DEFAULT) },
        "failedAllocations": crate::panics::failed_allocations(),
        "features": features,
    })
}

fn read_json(request: &mut Request<&mut EspHttpConnection<'_>>) -> anyhow::Result<Value> {
    let mut body = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let read = request.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if body.len() + read > MAX_BODY {
            anyhow::bail!("the request is longer than {MAX_BODY} bytes");
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(serde_json::from_slice(&body)?)
}

pub fn respond(request: Request<&mut EspHttpConnection<'_>>, status: u16, json: &Value) -> anyhow::Result<()> {
    let mut response = request.into_response(status, None, &[("Content-Type", "application/json")])?;
    response.write_all(&serde_json::to_vec(json)?)?;
    Ok(())
}
