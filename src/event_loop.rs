use anyhow::Context;
use evdev::InputEvent;
use sdl3::gamepad::Gamepad;
use sdl3::joystick::{PowerInfo, PowerLevel};
use sdl3::sensor::SensorType;
use sdl3::{EventPump, GamepadSubsystem, Sdl};
use sdl3::event::Event;
use sdl3_sys::joystick::SDL_JoystickID;
use sdl3_sys::timer::SDL_GetTicksNS;

use crate::{FmtOpt, FmtOptHex};
use crate::button_tracker::ButtonTracker;
use crate::config::Config;
use crate::parsed_config::ParsedConfig;
use crate::simulated::SimulatedGamepad;

pub fn entry(cfg: &Config, parsed_config: &ParsedConfig) -> anyhow::Result<()> {
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
        tracker: ButtonTracker::default(),
        tick: 0,
        last_power_check: 0,
        last_power_info: None,
    };

    eprintln!("SDL initialized");

    let wait_timeout_ms = cfg.input_gamepad.wait_timeout_ms.unwrap_or(10);

    loop {
        let event = ls.event_pump.wait_event_timeout_ms(wait_timeout_ms);

        if let Some(event) = event {
            ls.tick = event.get_timestamp();
            ls.process_event(event)?;
        } else {
            ls.tick = unsafe { SDL_GetTicksNS() };
        }

        if let Some(out) = &mut ls.output {
            out.submit()?;
        }

        if ls.parsed_config.power_refresh_interval != 0 && ls.tick >= ls.last_power_check + ls.parsed_config.power_refresh_interval {
            if let Err(e) = ls.power_check() {
                eprintln!("Failed to check gamepad power: {e:#}");
            }
            ls.last_power_check = ls.tick;
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
    tracker: ButtonTracker,
    tick: u64,
    last_power_check: u64,
    last_power_info: Option<(PowerLevel,i32)>,
}

impl LoopState<'_> {
    fn process_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::ControllerDeviceAdded { which, .. } => {
                let id = SDL_JoystickID(which);

                let name = self.gamepad_subsystem.name_for_id(id);
                let vendor = self.gamepad_subsystem.vendor_for_id(id);
                let product = self.gamepad_subsystem.product_for_id(id);
                let version = self.gamepad_subsystem.product_version_for_id(id);

                let in_cfg = &self.cfg.input_gamepad;

                let formatted_name = format!(
                    "{} ({}:{} ver. {})",
                    FmtOpt(name.as_deref().ok()),
                    FmtOptHex(vendor), FmtOptHex(product), FmtOptHex(version)
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

                        // Disable currently not supported sensors to maybe reduce gamepad battery usage
                        let _ = v.sensor_set_enabled(SensorType::AccelerometerLeft, false);
                        let _ = v.sensor_set_enabled(SensorType::AccelerometerRight, false);
                        let _ = v.sensor_set_enabled(SensorType::GyroscopeLeft, false);
                        let _ = v.sensor_set_enabled(SensorType::GyroscopeRight, false);
                        let _ = v.sensor_set_enabled(SensorType::Accelerometer, false);
                        let _ = v.sensor_set_enabled(SensorType::Gyroscope, false);

                        self.input = Some((id, v));
                        self.output = Some(out);

                        self.tracker = ButtonTracker::default();

                        eprintln!("Opened gamepad: {formatted_name}");
                    },
                    Ok(None) => eprintln!("Ignore gamepad: {formatted_name}"),
                    Err(e) => eprintln!("Failed to open gamepad: {formatted_name}: {e:#}"),
                }

            }
            Event::ControllerDeviceRemoved { which, .. } => {
                if let Some((id, _)) = &mut self.input && *id == which {
                    eprintln!("Gamepad disconnected");
                    SimulatedGamepad::close(&mut self.output)?;
                    self.input = None;
                }
            }
            Event::Quit { .. } => {
                eprintln!("Quitting");
                self.exit = true;
            }
            Event::ControllerButtonDown { which, button, .. } | Event::ControllerButtonUp { which, button, .. } => {
                if
                    self.input.as_ref().is_some_and(|(id,_)| id.0 == which )
                    && let Some(out) = &mut self.output
                {
                    let down = matches!(event, Event::ControllerButtonDown { .. });

                    let mapping = self.parsed_config.button_lut
                        .get(button.to_ll().0 as usize)
                        .copied()
                        .filter(|&c| c != u16::MAX );

                    if let Some(m) = mapping {
                        let evdev_event = InputEvent::new(evdev::EventType::KEY.0, m, down as _);
                        out.queue.push(evdev_event);
                    }

                    if self.cfg.behavior.dpad_to_hat0 {
                        self.tracker.track(button, down);
                        self.tracker.submit_to_evdev(&mut out.queue);
                    }
                }
            }
            Event::ControllerAxisMotion { which, axis, value, .. } => {
                if
                    self.input.as_ref().is_some_and(|(id,_)| id.0 == which )
                    && let Some(out) = &mut self.output
                {
                    let mapping = self.parsed_config.axis_lut
                        .get(axis.to_ll().0 as usize)
                        .and_then(|v| v.as_deref() );

                    if let Some(m) = mapping {
                        let mut scaled = value as i64 - m.in_off;
                        if scaled > 0 {
                            scaled = scaled * m.pos_fraction[0] / m.pos_fraction[1];
                        } else if scaled < 0 {
                            scaled = scaled * m.neg_fraction[0] / m.neg_fraction[1];
                        }
                        let scaled = (scaled + m.out_off).clamp(m.clamp_out[0] as i64, m.clamp_out[1] as i64) as i32;

                        let evdev_event = InputEvent::new(evdev::EventType::ABSOLUTE.0, m.setup.code(), scaled);
                        out.queue.push(evdev_event);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn power_check(&mut self) -> anyhow::Result<()> {
        if let Some((_,gp)) = &self.input {
            let PowerInfo { state, percentage } = gp.power_info();
            if self.last_power_info != Some((state,percentage)) {
                eprintln!("Battery: {percentage}% ({state:?})");
            }
            self.last_power_info = Some((state,percentage));
        }
        Ok(())
    }
}
