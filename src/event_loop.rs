use anyhow::Context;
use sdl3::gamepad::Gamepad;
use sdl3::sensor::SensorType;
use sdl3::{EventPump, GamepadSubsystem, Sdl};
use sdl3::event::Event;
use sdl3_sys::joystick::SDL_JoystickID;

use crate::config::Config;
use crate::parsed_config::ParsedConfig;
use crate::simulated::SimulatedGamepad;

pub fn main(cfg: &Config, parsed_config: &ParsedConfig) -> anyhow::Result<()> {
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");

    for (k, v) in &cfg.sdl_hints {
        sdl3::hint::set(k, v);
    }

    let sdl_context = sdl3::init()?;
    let gamepad_subsystem = sdl_context.gamepad()?;
    let event_pump = sdl_context.event_pump()?;

    let mut ls = LoopState {
        sdl_context,
        gamepad_subsystem,
        event_pump,
        cfg,
        parsed_config,
        exit: false,
        output: None,
        input: None,
    };

    eprintln!("SDL initialized");

    let wait_timeout_ms = cfg.input_gamepad.wait_timeout_ms.unwrap_or(10);

    loop {
        let event = ls.event_pump.wait_event_timeout_ms(wait_timeout_ms);

        let Some(event) = event else {continue};

        ls.process_event(event)?;

        if let Some(out) = &mut ls.output {
            out.submit()?;
        }

        if ls.exit {
            break;
        }
    }

    Ok(())
}

struct LoopState<'a> {
    sdl_context: Sdl,
    gamepad_subsystem: GamepadSubsystem,
    event_pump: EventPump,
    exit: bool,
    input: Option<(SDL_JoystickID,Gamepad)>,
    output: Option<SimulatedGamepad>,
    cfg: &'a Config,
    parsed_config: &'a ParsedConfig,
}

impl LoopState<'_> {
    fn process_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::ControllerDeviceAdded { timestamp, which } => {
                let id = SDL_JoystickID(which);

                let name = self.gamepad_subsystem.name_for_id(id);
                let vendor = self.gamepad_subsystem.vendor_for_id(id);
                let product = self.gamepad_subsystem.product_for_id(id);
                let version = self.gamepad_subsystem.product_version_for_id(id);

                let in_cfg = &self.cfg.input_gamepad;

                let formatted_name = format!(
                    "{} ({}:{} ver. {})",
                    name.as_deref().unwrap_or("??"),
                    vendor.map_or_else(|| "??".to_owned(), |v| format!("{v:#06X}")),
                    product.map_or_else(|| "??".to_owned(), |v| format!("{v:#06X}")),
                    version.map_or_else(|| "??".to_owned(), |v| format!("{v:#06X}")),
                );

                let try_open = || -> anyhow::Result<Option<Gamepad>> {
                    if let Some(v) = &in_cfg.filter_name {
                        let name = name?;

                        if !name.contains(v) {
                            return Ok(None);
                        }
                    }

                    if !in_cfg.filter_vendor_id.slice().is_empty() {
                        let v = vendor.context("Failed to query vendor id")?;

                        if !in_cfg.filter_vendor_id.slice().contains(&v) {
                            return Ok(None);
                        }
                    }

                    if !in_cfg.filter_product_id.slice().is_empty() {
                        let v = product.context("Failed to query product id")?;

                        if !in_cfg.filter_product_id.slice().contains(&v) {
                            return Ok(None);
                        }
                    }

                    if !in_cfg.filter_product_version.slice().is_empty() {
                        let v = version.context("Failed to query product version")?;

                        if !in_cfg.filter_product_version.slice().contains(&v) {
                            return Ok(None);
                        }
                    }

                    if self.input.is_some() {
                        return Ok(None);
                    }

                    Ok(Some(self.gamepad_subsystem.open(id)?))
                };

                match try_open() {
                    Ok(Some(v)) => {
                        SimulatedGamepad::close(&mut self.output)?;

                        let out = SimulatedGamepad::create(self.cfg, self.parsed_config)?;

                        let _ = v.sensor_set_enabled(SensorType::AccelerometerLeft, false);
                        let _ = v.sensor_set_enabled(SensorType::AccelerometerRight, false);
                        let _ = v.sensor_set_enabled(SensorType::GyroscopeLeft, false);
                        let _ = v.sensor_set_enabled(SensorType::GyroscopeRight, false);
                        let _ = v.sensor_set_enabled(SensorType::Accelerometer, false);
                        let _ = v.sensor_set_enabled(SensorType::Gyroscope, false);

                        self.input = Some((id, v));
                        self.output = Some(out);
                    },
                    Ok(None) => eprintln!("Ignore gamepad {formatted_name}"),
                    Err(e) => eprintln!("Failed to open gamepad {formatted_name}: {e}"),
                }

            }
            Event::ControllerDeviceRemoved { timestamp, which } => {
                if let Some((id, gp)) = &mut self.input && *id == which {
                    SimulatedGamepad::close(&mut self.output)?;
                    self.input = None;
                }
            }
            Event::Quit { .. } => {
                eprintln!("Quitting");
                self.exit = true;
            }
            _ => {}
        }

        Ok(())
    }
}
