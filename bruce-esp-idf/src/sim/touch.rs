//! The touch screen, for the simulator: touched by the mouse held down on the screen in the simulator's window, see
//! the platform's touch.rs and touch/axs15231b.rs, whose API it has.

use std::sync::Arc;

use bruce_embedded_std::listener::{Listener, ListenerManager};

pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

pub struct TouchDriver {
    listener_manager: Arc<ListenerManager<TouchPoint>>,
}

impl TouchDriver {
    /// Gives each touch of the simulated screen to the listeners, as the real driver gives each it reads.
    pub fn start() -> anyhow::Result<TouchDriver> {
        let listener_manager = Arc::new(ListenerManager::new());
        let listeners = listener_manager.clone();
        crate::sim::device().on_input(move |input| {
            if let bruce_embedded_sim::Input::Touch(point) = input {
                listeners.emit(&TouchPoint {
                    x: point.x.max(0) as u16,
                    y: point.y.max(0) as u16,
                });
            }
        });
        Ok(TouchDriver { listener_manager })
    }

    pub fn add_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.add_listener(listener);
    }

    pub fn remove_listener(&self, listener: &Arc<dyn Listener<TouchPoint>>) {
        self.listener_manager.remove_listener(listener);
    }
}
