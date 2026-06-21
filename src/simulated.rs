use std::io;

use evdev::InputEvent;
use evdev::uinput::VirtualDevice;

pub struct SimulatedGamepad {
    pub dev: VirtualDevice,
    pub queue: Vec<InputEvent>,
}

impl SimulatedGamepad {
    pub fn emit(&mut self) -> io::Result<()> {
        let result = self.dev.emit(&self.queue);
        self.queue.clear();
        result
    }
}
