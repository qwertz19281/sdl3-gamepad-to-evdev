use anyhow::Context as _;
use evdev::{AbsoluteAxisCode, EventSummary, EventType, FFEffectCode, FFEffectKind, InputEvent, UInputCode};
use sdl3::event::Event;
use sdl3::gamepad::Gamepad;
use sdl3::joystick::{PowerInfo, PowerLevel};
use sdl3::sensor::SensorType;
use sdl3::{EventPump, EventSubsystem, GamepadSubsystem, Sdl};
use sdl3_sys::events::{SDL_EVENT_FIRST, SDL_EVENT_LAST, SDL_GETEVENT, SDL_PeepEvents};
use sdl3_sys::joystick::SDL_JoystickID;

use crate::simulated_gyro::SimulatedGamepadGyro;
use crate::{FmtOpt, FmtOptHex};
use crate::button_tracker::ButtonTracker;
use crate::config::Config;
use crate::parsed_config::ParsedConfig;
use crate::simulated::{SimpleRumbleSlot, SimulatedGamepad};


pub struct LoopState<'a> {
    #[allow(unused)]
    pub sdl_context: Sdl,
    pub gamepad_subsystem: GamepadSubsystem,
    pub event_pump: EventPump,
    #[allow(unused)]
    pub event_subsystem: EventSubsystem,
    pub exit: bool,
    pub input: Option<(SDL_JoystickID,Gamepad)>,
    pub output: Option<SimulatedGamepad>,
    pub motion_output: Option<SimulatedGamepadGyro>,
    pub cfg: &'a Config,
    pub parsed_config: &'a ParsedConfig,
    pub tracker: ButtonTracker,
    pub tick: u64,
    pub last_power_check: u64,
    pub last_power_info: Option<(PowerLevel,i32)>,
}

impl LoopState<'_> {
    pub fn process_event(&mut self, event: Event) -> anyhow::Result<()> {
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
                        SimulatedGamepadGyro::close(&mut self.motion_output)?;

                        let out = SimulatedGamepad::create(self.cfg, self.parsed_config)?;

                        if
                            let Some(gicfg) = &self.cfg.simulate_gamepad_gyro
                            && let Some(pgicfg) = &self.parsed_config.parsed_gyro
                            && gicfg.enable
                        {
                            let out = SimulatedGamepadGyro::create(self.cfg, gicfg, self.parsed_config, pgicfg)?;

                            v.sensor_set_enabled(SensorType::Accelerometer, true)?;
                            v.sensor_set_enabled(SensorType::Gyroscope, true)?;

                            self.motion_output = Some(out);
                        } else {
                            // Disable unusede sensors to maybe reduce gamepad battery usage
                            let _ = v.sensor_set_enabled(SensorType::AccelerometerLeft, false);
                            let _ = v.sensor_set_enabled(SensorType::AccelerometerRight, false);
                            let _ = v.sensor_set_enabled(SensorType::GyroscopeLeft, false);
                            let _ = v.sensor_set_enabled(SensorType::GyroscopeRight, false);
                            let _ = v.sensor_set_enabled(SensorType::Accelerometer, false);
                            let _ = v.sensor_set_enabled(SensorType::Gyroscope, false);
                        }

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
                    SimulatedGamepadGyro::close(&mut self.motion_output)?;
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
                        let evdev_event = InputEvent::new(EventType::KEY.0, m, down as _);
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

                        let evdev_event = InputEvent::new(EventType::ABSOLUTE.0, m.setup.code(), scaled);
                        out.queue.push(evdev_event);
                    }
                }
            }
            Event::ControllerSensorUpdated { which, sensor, data: [ix,iy,iz], .. } => {
                if
                    self.input.as_ref().is_some_and(|(id,_)| id.0 == which )
                    //&& let Some(out) = &mut self.output
                    && let Some(mout) = &mut self.motion_output
                    && let Some(gicfg) = &self.parsed_config.parsed_gyro
                    && let Some(([mx,my,mz], cx,cy,cz, out_info)) = match sensor {
                        SensorType::Accelerometer => Some((
                            gicfg.accel_mul,
                            AbsoluteAxisCode::ABS_X, AbsoluteAxisCode::ABS_Y, AbsoluteAxisCode::ABS_Z,
                            gicfg.accel_info
                        )),
                        SensorType::Gyroscope => Some((
                            gicfg.gyro_mul,
                            AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY, AbsoluteAxisCode::ABS_RZ,
                            gicfg.gyro_info
                        )),
                        _ => None,
                    }
                {
                    let [ox,oy,oz] = [
                        ((ix * mx) as i64).clamp(out_info.minimum() as _, out_info.maximum() as _) as i32,
                        ((iy * my) as i64).clamp(out_info.minimum() as _, out_info.maximum() as _) as i32,
                        ((iz * mz) as i64).clamp(out_info.minimum() as _, out_info.maximum() as _) as i32,
                    ];

                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cx.0, ox));
                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cy.0, oy));
                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cz.0, oz));
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn power_check(&mut self) -> anyhow::Result<()> {
        if let Some((_,gp)) = &self.input {
            let PowerInfo { state, percentage } = gp.power_info();
            if self.last_power_info != Some((state,percentage)) {
                eprintln!("Battery: {percentage}% ({state:?})");
            }
            self.last_power_info = Some((state,percentage));
        }
        Ok(())
    }

    pub fn pull_event_batch(&mut self, max: usize, wait_ms: u32) -> Vec<Event> {
        let first = self.event_pump.wait_event_timeout_ms(wait_ms);

        let Some(first) = first else {return vec![]};

        let mut staging_vec = Vec::with_capacity(max-1);
        let mut out_vec = Vec::with_capacity(max);

        out_vec.push(first);

        if max > 1 {
            let result = unsafe {
                SDL_PeepEvents(
                    staging_vec.as_mut_ptr(),
                    max as i32 - 1,
                    SDL_GETEVENT,
                    SDL_EVENT_FIRST.into(),
                    SDL_EVENT_LAST.into(),
                )
            };

            if result < 0 {
                panic!("SDL_PeepEvents error {result}");
            }

            unsafe {
                staging_vec.set_len(result as _);
            }

            for v in staging_vec {
                out_vec.push(Event::from_ll(v));
            }
        }

        out_vec
    }

    pub fn process_rumble(&mut self) -> anyhow::Result<()> {
        if !self.cfg.simulate_gamepad.enable_rumble {return Ok(());}

        let Some(out) = &mut self.output else {return Ok(())};
        let Some((_,gp)) = &mut self.input else {return Ok(())};

        out.fill_in_queue()?;

        for e in out.in_queue.drain(..) {
            //eprintln!("EVENT: {e:?}");
            match e.destructure() {
                EventSummary::ForceFeedback(_ffevent, FFEffectCode::FF_GAIN, v) => {
                    out.ff_gain = v;
                },
                EventSummary::ForceFeedback(_ffevent, slot, v) if (slot.0 as usize) < out.ff_slot.len() => {
                    if v > 0 {
                        if let Some(slot) = &out.ff_slot[slot.0 as usize] {
                            let mut duration = slot.duration as u32;
                            if duration == 0 {
                                duration = 65535;
                            }
                            let left = (slot.left as u64 * self.parsed_config.rumble_mul[0] * out.ff_gain as u64 / (65535*65535)).min(65535) as u16;
                            let right = (slot.right as u64 * self.parsed_config.rumble_mul[1] * out.ff_gain as u64 / (65535*65535)).min(65535) as u16;
                            gp.set_rumble(left, right, duration)?;
                        } else {
                            gp.set_rumble(0, 0, 0)?;
                        }
                    } else {
                        gp.set_rumble(0, 0, 0)?;
                    }
                },
                EventSummary::UInput(uinput_event, UInputCode::UI_FF_UPLOAD, _) => {
                    let uploaded = out.dev.process_ff_upload(uinput_event)?;
                    let effect = uploaded.effect();
                    
                    if let Some(slot) = out.ff_slot.get_mut(uploaded.effect_id() as usize) {
                        if let FFEffectKind::Rumble { strong_magnitude, weak_magnitude } = effect.kind {
                            *slot = Some(SimpleRumbleSlot {
                                left: strong_magnitude,
                                right: weak_magnitude,
                                duration: effect.replay.length,
                            });
                        } else {
                            *slot = None;
                        }
                    }
                },
                EventSummary::UInput(uinput_event, UInputCode::UI_FF_ERASE, _) => {
                    let uploaded = out.dev.process_ff_erase(uinput_event)?;

                    if let Some(slot) = out.ff_slot.get_mut(uploaded.effect_id() as usize) {
                        *slot = None;
                    }
                },
                _ => {}
            }
        }

        Ok(())
    }
}
