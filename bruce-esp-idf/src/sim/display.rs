//! The board's display, for the simulator: the simulated board's, in the window, which behaves as the real one, see
//! bruce-embedded-sim's display.rs.

/// The display manager of the board this is built for, as the platform's display/mod.rs has it.
#[cfg(feature = "mono-display")]
pub type BoardDisplayManager = bruce_embedded_sim::MonoDisplayManager;
#[cfg(not(feature = "mono-display"))]
pub type BoardDisplayManager = bruce_embedded_sim::ColourDisplayManager;

/// The simulated board's display manager.
pub(crate) fn display_manager() -> anyhow::Result<BoardDisplayManager> {
    #[cfg(feature = "mono-display")]
    return crate::sim::device().mono_display_manager();
    #[cfg(not(feature = "mono-display"))]
    return crate::sim::device().colour_display_manager();
}
