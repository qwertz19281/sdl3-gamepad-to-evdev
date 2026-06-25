use evdev::{AbsoluteAxisCode, InputEvent};
use sdl3::gamepad::Button;

#[derive(Default)]
pub struct ButtonTracker {
    dpad_l: bool,
    dpad_r: bool,
    dpad_u: bool,
    dpad_d: bool,
    hatx: i32,
    haty: i32,
    dirty_hatx: bool,
    dirty_haty: bool,
}

impl ButtonTracker {
    pub fn track(&mut self, sdl_button: Button, v: bool) {
        match sdl_button {
            Button::DPadLeft => {
                self.dpad_l = v;
                self.hatx = (self.dpad_r as i32) - (self.dpad_l as i32);
                self.dirty_hatx = true;
            },
            Button::DPadRight => {
                self.dpad_r = v;
                self.hatx = (self.dpad_r as i32) - (self.dpad_l as i32);
                self.dirty_hatx = true;
            },
            Button::DPadUp => {
                self.dpad_u = v;
                self.haty = (self.dpad_d as i32) - (self.dpad_u as i32);
                self.dirty_haty = true;
            },
            Button::DPadDown => {
                self.dpad_d = v;
                self.haty = (self.dpad_d as i32) - (self.dpad_u as i32);
                self.dirty_haty = true;
            },
            _ => {},
        }
    }

    pub fn submit_to_evdev(&mut self, queue: &mut Vec<InputEvent>, [ax,ay]: [AbsoluteAxisCode;2]) {
        if self.dirty_hatx {
            self.dirty_hatx = false;
            let evdev_event = InputEvent::new(evdev::EventType::ABSOLUTE.0, ax.0, self.hatx);
            //eprintln!("hatter x {evdev_event:?}");
            queue.push(evdev_event);
        }
        if self.dirty_haty {
            self.dirty_haty = false;
            let evdev_event = InputEvent::new(evdev::EventType::ABSOLUTE.0, ay.0, self.haty);
            //eprintln!("hatter x {evdev_event:?}");
            queue.push(evdev_event);
        }
    }
}
