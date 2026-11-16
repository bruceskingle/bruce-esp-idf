//! Where the RAM is, for the simulator: the computer's memory is not the device's, so there is nothing to report, see
//! the platform's memory.rs.

use serde_json::{Value, json};

pub fn to_json() -> Value {
    json!({ "simulated": true })
}
