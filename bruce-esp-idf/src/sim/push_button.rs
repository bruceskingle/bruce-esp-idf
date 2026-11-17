//! A button on the board, for the simulator: pressed by a click on it in the simulator's window, see the platform's
//! push_button.rs, whose API it has.

use std::sync::{Arc, Mutex};

use log::info;

/// What a press does, see PushButton::set_action.
pub type Action = Box<dyn FnMut() -> anyhow::Result<()> + Send>;

#[derive(Clone)]
pub struct PushButton {
    name: &'static str,
    action: Arc<Mutex<Option<Action>>>,
}

impl PushButton {
    /// The board's button called name, e.g. KEY, as the simulated board names it, with no action until one is set.
    pub fn new(name: &'static str) -> Self {
        let action: Arc<Mutex<Option<Action>>> = Arc::new(Mutex::new(None));
        let pressed_action = action.clone();
        crate::sim::device().on_input(move |input| {
            if let bruce_embedded_sim::Input::Button {
                name: pressed,
                pressed: true,
            } = input
                && pressed.eq_ignore_ascii_case(name)
            {
                info!("{name} pressed");
                if let Some(action) = pressed_action.lock().unwrap().as_mut()
                    && let Err(error) = action()
                {
                    log::warn!("The action of {name} failed, {error:#}");
                }
            }
        });
        PushButton { name, action }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn set_action(&self, action: Action) {
        *self.action.lock().unwrap() = Some(action);
    }

    pub fn clear_action(&self) {
        *self.action.lock().unwrap() = None;
    }
}
