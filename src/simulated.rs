use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd as _;

use evdev::{AttributeSet, FFEffectCode, InputEvent, InputId};
use evdev::uinput::VirtualDevice;

use crate::config::Config;
use crate::none_vec;
use crate::parsed_config::ParsedConfig;

pub struct SimulatedGamepad {
    pub dev: VirtualDevice,
    pub queue: Vec<InputEvent>,
    pub in_queue: Vec<InputEvent>,
    pub ff_gain: i32,
    pub ff_slot: Vec<Option<SimpleRumbleSlot>>,
}

impl SimulatedGamepad {
    pub fn submit(&mut self) -> io::Result<()> {
        if self.queue.is_empty() {return Ok(());}
        let result = self.dev.emit(&self.queue);
        self.queue.clear();
        result
    }

    pub fn close(v: &mut Option<Self>) -> io::Result<()> {
        if let Some(v) = v {
            v.submit()?;
        }
        Ok(())
    }
}

impl SimulatedGamepad {
    pub fn create(cfg: &Config, parsed: &ParsedConfig) -> anyhow::Result<SimulatedGamepad> {
        let mut keys = AttributeSet::new();

        for v in parsed.button_bindings.values() {
            keys.insert(v.code);
        }

        let mut builder = VirtualDevice::builder()?
            .name(&cfg.simulate_gamepad.name)
            .input_id(InputId::new(
                parsed.evdev_bus_type,
                cfg.simulate_gamepad.vendor_id,
                cfg.simulate_gamepad.product_id,
                cfg.simulate_gamepad.version,
            ))
            .with_keys(&keys)?;

        for v in parsed.axis_bindings.values() {
            builder = builder.with_absolute_axis(&v.setup)?;
        }

        for v in &parsed.additional_axes {
            builder = builder.with_absolute_axis(v)?;
        }

        if cfg.simulate_gamepad.enable_rumble {
            let mut ff = AttributeSet::new();
            ff.insert(FFEffectCode::FF_RUMBLE);

            builder = builder
                .with_ff_effects_max(16)
                .with_ff(&ff)?;
        }

        let dev = builder.build()?;

        // O_NONBLOCK is needed for polling rumble events without blocking the same thread
        use nix::fcntl;
        fcntl::fcntl(dev.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))?;

        Ok(SimulatedGamepad {
            dev,
            queue: Vec::with_capacity(8),
            in_queue: Vec::with_capacity(32),
            ff_slot: none_vec(16),
            ff_gain: 65536,
        })
    }

    pub fn fill_in_queue(&mut self) -> io::Result<()> {
        let poll_events = match self.dev.fetch_events() {
            Ok(v) => v,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {return Ok(())},
            Err(e) => return Err(e),
        };

        self.in_queue.extend(poll_events);

        Ok(())
    }
}

#[derive(Debug)]
pub struct SimpleRumbleSlot {
    pub left: u16,
    pub right: u16,
    pub duration: u16,
}
