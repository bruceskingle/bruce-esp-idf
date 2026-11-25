//! Who may make each API request, for the firmware to decide, see Authoriser, so that the platform needs nothing of
//! how it does, e.g. bruce-db's Security Contexts in the capture firmware. A request in the authoriser's scheme is
//! decided by it; any other by the adopting app's key, as before, see Adoption, so that a device adopted before keeps
//! working until the app moves it over.

use serde_json::Value;

/// What a request needs to be allowed to do, see Authoriser::allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Read the device's data, e.g. its store, as any of the site's members may.
    Read,
    /// Read the device's config, memory and starts, change its settings, restart or release it, as only those who
    /// manage the device may.
    Manage,
}

/// A request to decide, see Authoriser::allows.
pub struct SignedRequest<'a> {
    pub method: &'a str,
    /// Its path and query, as signed.
    pub uri: &'a str,
    /// Its whole Authorization header, which starts with the authoriser's scheme.
    pub authorization: &'a str,
    /// Its body, empty for a GET.
    pub body: &'a [u8],
}

/// Decides the requests whose Authorization header starts with its scheme, given to the platform by the firmware, see
/// Esp32Platform::set_authoriser.
pub trait Authoriser: Send + Sync {
    /// The start of the Authorization headers it decides, e.g. `Bruce-Signed `.
    fn scheme(&self) -> &str;

    /// Ok if the request may do what access says, otherwise why not: a Forbidden error if it is signed by someone who
    /// may not, which is answered 403, any other if it could not tell who, 401.
    fn allows(&self, request: &SignedRequest, access: Access) -> anyhow::Result<()>;

    /// What /api/info gives as `security`, e.g. the device's public keys, for an app to find out how to reach it.
    fn info(&self) -> Value;

    /// Called when the device has been released, e.g. to forget its site.
    fn released(&self) {}
}

/// Why a request signed as it should be may not do what it asked, see Authoriser::allows, answered 403, Forbidden,
/// rather than 401, which says the device could not tell who asked, so that a client does not take its signature to
/// be the trouble, e.g. and fall back to another way of asking.
#[derive(Debug)]
pub struct Forbidden(pub String);

impl std::fmt::Display for Forbidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Forbidden {}
