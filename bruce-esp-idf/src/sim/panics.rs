//! The last crash, for the simulator, which keeps none, as the computer's own reports a program's crash, see the
//! platform's panics.rs.

pub fn install() {}

pub fn last() -> Option<&'static str> {
    None
}

pub fn failed_allocations() -> u32 {
    0
}
