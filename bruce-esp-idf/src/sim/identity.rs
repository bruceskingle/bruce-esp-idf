//! The device's identity, for the simulator, as the platform's identity.rs makes it: an ID, a P-256 key and a
//! self-signed certificate, CN=bruce-<id>, valid from 2025 to 2099, not a CA, made the first time the device starts and
//! kept in its NVS, so that its fingerprint stays the same for an app to pin. The ID, which a real device takes from
//! its MAC address, is random, made once and kept, as a computer may run several simulated devices.

use std::ffi::{CStr, CString};

use bruce_embedded_sim::svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use log::info;
use sha2::{Digest, Sha256};

/// The NVS namespace of the device's own settings, which is reserved, see config_store.rs.
pub const DEVICE_NAMESPACE: &str = "device";

const ID_KEY: &str = "sim_id";
const CERT_KEY: &str = "tls_cert";
const KEY_KEY: &str = "tls_key";
const FORMAT_KEY: &str = "tls_format";
/// As the platform's identity.rs's FORMAT, so that a certificate this made is taken as current by it too.
const FORMAT: u8 = 2;
const PEM_LEN: usize = 2048;

pub struct DeviceIdentity {
    /// 12 lower case hex digits, as a real device's MAC address is.
    pub id: String,
    pub cert_pem: &'static CStr,
    pub key_pem: &'static CStr,
    /// The SHA-256 of the certificate, as lower case hex, for an app to pin.
    pub fingerprint: String,
}

impl DeviceIdentity {
    /// The identity kept in NVS, made and saved first if there is none.
    pub fn load_or_create(nvs_partition: &EspNvsPartition<NvsDefault>) -> anyhow::Result<DeviceIdentity> {
        let nvs = EspNvs::new(nvs_partition.clone(), DEVICE_NAMESPACE, true)?;
        let id = device_id(&nvs)?;
        let mut cert_buffer = vec![0u8; PEM_LEN];
        let mut key_buffer = vec![0u8; PEM_LEN];
        let current = nvs.get_u8(FORMAT_KEY)? == Some(FORMAT);
        let saved = match (
            nvs.get_str(CERT_KEY, &mut cert_buffer)?,
            nvs.get_str(KEY_KEY, &mut key_buffer)?,
        ) {
            (Some(cert), Some(key)) if current => Some((cert.to_string(), key.to_string())),
            _ => None,
        };
        let (cert, key, der) = match saved {
            Some((cert, key)) => {
                let der = der_of(&cert)?;
                (cert, key, der)
            }
            None => {
                info!("Making the device's HTTPS key and certificate");
                let (cert, key, der) = generate(&id)?;
                nvs.set_str(CERT_KEY, &cert)?;
                nvs.set_str(KEY_KEY, &key)?;
                nvs.set_u8(FORMAT_KEY, FORMAT)?;
                (cert, key, der)
            }
        };
        let cert_pem: &'static CStr = Box::leak(CString::new(cert)?.into_boxed_c_str());
        let key_pem: &'static CStr = Box::leak(CString::new(key)?.into_boxed_c_str());
        let fingerprint = hex(&Sha256::digest(&der));
        info!("Device {id}, certificate fingerprint {fingerprint}");
        Ok(DeviceIdentity {
            id,
            cert_pem,
            key_pem,
            fingerprint,
        })
    }
}

/// The ID kept in NVS, made at random the first time, with the locally administered bit set, as a made up MAC
/// address's should be, so that it is never a real device's.
pub(crate) fn device_id(nvs: &EspNvs<NvsDefault>) -> anyhow::Result<String> {
    let mut buffer = [0u8; 16];
    if let Some(id) = nvs.get_str(ID_KEY, &mut buffer)? {
        return Ok(id.to_string());
    }
    let mut mac = [0u8; 6];
    unsafe { bruce_embedded_sim::svc::sys::esp_fill_random(mac.as_mut_ptr() as *mut _, mac.len()) };
    mac[0] = (mac[0] & 0xfc) | 0x02;
    let id = hex(&mac);
    nvs.set_str(ID_KEY, &id)?;
    Ok(id)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The certificate and key, as PEM, and the certificate's DER.
fn generate(id: &str) -> anyhow::Result<(String, String, Vec<u8>)> {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let mut params = rcgen::CertificateParams::default();
    let mut name = rcgen::DistinguishedName::new();
    name.push(rcgen::DnType::CommonName, format!("bruce-{id}"));
    params.distinguished_name = name;
    params.not_before = rcgen::date_time_ymd(2025, 1, 1);
    params.not_after = rcgen::date_time_ymd(2099, 12, 31);
    params.is_ca = rcgen::IsCa::ExplicitNoCa;
    let cert = params.self_signed(&key)?;
    Ok((cert.pem(), key.serialize_pem(), cert.der().to_vec()))
}

/// The DER of a certificate in PEM.
fn der_of(pem: &str) -> anyhow::Result<Vec<u8>> {
    use rustls_pki_types::CertificateDer;
    use rustls_pki_types::pem::PemObject;
    Ok(CertificateDer::from_pem_slice(pem.as_bytes())
        .map_err(|error| anyhow::anyhow!("The saved certificate is not PEM, {error}"))?
        .to_vec())
}
