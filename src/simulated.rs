use std::io;

use evdev::{AttributeSet, InputEvent, InputId};
use evdev::uinput::VirtualDevice;

use crate::config::Config;
use crate::parsed_config::ParsedConfig;

pub struct SimulatedGamepad {
    pub dev: VirtualDevice,
    pub queue: Vec<InputEvent>,
}

impl SimulatedGamepad {
    pub fn submit(&mut self) -> io::Result<()> {
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


        let dev = builder.build()?;

        Ok(SimulatedGamepad {
            dev,
            queue: Vec::with_capacity(8),
        })
    }
}
