use std::error::Error;

use anyhow::{Context as _, bail};
use evdev::{AbsoluteAxisCode, EventSummary, EventType, FFEffectCode, FFEffectKind, InputEvent, UInputCode};
use sdl3::event::Event;
use sdl3::gamepad::Gamepad;
use sdl3::joystick::{HatState, PowerInfo, PowerLevel};
use sdl3::sensor::SensorType;
use sdl3::{EventPump, EventSubsystem, GamepadSubsystem, Sdl};
use sdl3_sys::events::{SDL_EVENT_FIRST, SDL_EVENT_LAST, SDL_GETEVENT, SDL_PeepEvents};
use sdl3_sys::joystick::SDL_JoystickID;

use crate::calibration::CalibrationState;
use crate::simulated_gyro::SimulatedGamepadGyro;
use crate::{Args, FmtOpt, FmtOptHex};
use crate::button_tracker::ButtonTracker;
use crate::config::{Config, SingleOrArray};
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
    pub input: Option<(SDL_JoystickID,Gamepad,CalibrationState<'a>)>,
    pub output: Option<SimulatedGamepad>,
    pub motion_output: Option<SimulatedGamepadGyro>,
    pub cfg: &'a Config,
    pub parsed_config: &'a ParsedConfig,
    pub app_args: &'a Args,
    pub tracker: ButtonTracker,
    pub tick: u64,
    pub last_power_check: u64,
    pub last_power_info: Option<(PowerLevel,i32)>,
}

impl LoopState<'_> {
    pub fn process_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::ControllerButtonDown { which, button, .. } | Event::ControllerButtonUp { which, button, .. } => {
                if let Some((id, _, _)) = self.input.as_mut()
                    && id.0 == which
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

                    if let Some(axes) = self.parsed_config.dpad_to_hat_axis {
                        self.tracker.track(button, down);
                        self.tracker.submit_to_evdev(&mut out.queue, axes);
                    }
                }
            }
            Event::ControllerAxisMotion { which, axis, value, .. } => {
                if let Some((id, _, calib)) = self.input.as_mut()
                    && id.0 == which
                    && let Some(out) = &mut self.output
                {
                    let mapping = self.parsed_config.axis_lut
                        .get(axis.to_ll().0 as usize)
                        .and_then(|v| v.as_deref() );

                    let mstate = out.digitrigger_state
                        .get_mut(axis.to_ll().0 as usize);

                    if let Some(m) = mapping && let Some((mstate, _prev)) = mstate {
                        if let Some((gi, dim)) = m.axisgroup
                            && calib.set_axis(value, gi, dim)
                        {} else {
                            let in_offsetted = value as i64 - m.in_off;
                            let mut scaled = in_offsetted;
                            if scaled > 0 {
                                scaled = scaled * m.pos_fraction[0] / m.pos_fraction[1];
                            } else if scaled < 0 {
                                scaled = scaled * m.neg_fraction[0] / m.neg_fraction[1];
                            }
                            let scaled = (scaled + m.out_off).clamp(m.clamp_out[0] as i64, m.clamp_out[1] as i64) as i32;

                            let evdev_event = InputEvent::new(EventType::ABSOLUTE.0, m.setup.code(), scaled);
                            out.queue.push(evdev_event);

                            if self.cfg.behavior.simulate_digital_trigger
                                && let Some(code) = m.digitrigger_button
                                && m.digitrigger_thresh[0] != 0
                            {
                                let [press, release] = [m.digitrigger_thresh[0] as i64, m.digitrigger_thresh[1] as i64];
                                // parse_config checked that press and release have same signum and sensible values
                                let down = if *mstate {
                                    in_offsetted.signum() == press.signum() && in_offsetted.abs() >= release.abs()
                                } else {
                                    in_offsetted.signum() == press.signum() && in_offsetted.abs() >= press.abs()
                                };

                                if *mstate != down {
                                    let evdev_event = InputEvent::new(EventType::KEY.0, code.0, down as _);
                                    out.queue.push(evdev_event);
                                    *mstate = down;
                                }
                            }
                        }
                    }
                }
            }
            Event::JoyHatMotion { which, hat_idx, state, .. } => {
                if let Some((id, _, _)) = self.input.as_mut()
                    && id.0 == which
                    && let Some(out) = &mut self.output
                {
                    let mapping = self.parsed_config.hat_lut
                        .get(hat_idx as usize)
                        .and_then(|v| v.as_ref() );

                    if let Some(m) = mapping {
                        let axes = match state {
                            HatState::Centered => [0,0],
                            HatState::Up => [0,-1],
                            HatState::Right => [1,0],
                            HatState::Down => [0,1],
                            HatState::Left => [-1,0],
                            HatState::RightUp => [1,-1],
                            HatState::RightDown => [1,1],
                            HatState::LeftUp => [-1,-1],
                            HatState::LeftDown => [-1,1],
                        };

                        for i in [0,1] {
                            let evdev_event = InputEvent::new(EventType::ABSOLUTE.0, m[i].0, axes[i]);
                            out.queue.push(evdev_event);
                        }
                    }
                }
            }
            Event::ControllerSensorUpdated { which, sensor, data: [ix,iy,iz], .. } => {
                if
                    self.input.as_ref().is_some_and(|(id,_,_)| id.0 == which )
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
                        ((ix * mx) as i32).clamp(out_info.minimum(), out_info.maximum()),
                        ((iy * my) as i32).clamp(out_info.minimum(), out_info.maximum()),
                        ((iz * mz) as i32).clamp(out_info.minimum(), out_info.maximum()),
                    ];

                    // if (ix * mx) as i64 != ((ix * mx) as i64).clamp(out_info.minimum() as _, out_info.maximum() as _) {
                    //     eprintln!("EXCITE DEO: {sensor:?}: {} != {}", (ix * mx) as i64, ((ix * mx) as i64).clamp(out_info.minimum() as _, out_info.maximum() as _));
                    // }

                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cx.0, ox));
                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cy.0, oy));
                    mout.queue.push(InputEvent::new(EventType::ABSOLUTE.0, cz.0, oz));
                }
            }
            Event::ControllerDeviceAdded { which, .. } => {
                self.add_controller(which)?;
            }
            Event::ControllerDeviceRemoved { which, .. } => {
                if let Some((id, _, _)) = &mut self.input && *id == which {
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
            _ => {}
        }

        Ok(())
    }

    pub fn power_check(&mut self) -> anyhow::Result<()> {
        if let Some((_,gp,_)) = &self.input {
            let PowerInfo { state, percentage } = gp.power_info();
            if self.last_power_info != Some((state,percentage)) {
                eprintln!("Battery: {percentage}% ({state:?})");
            }
            self.last_power_info = Some((state,percentage));
        }
        Ok(())
    }

    pub fn process_rumble(&mut self) -> anyhow::Result<()> {
        if !self.cfg.simulate_gamepad.enable_rumble {return Ok(());}

        let Some(out) = &mut self.output else {return Ok(())};
        let Some((_,gp,_)) = &mut self.input else {return Ok(())};

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

    pub fn add_controller(&mut self, which: u32) -> anyhow::Result<()> {
        let id = SDL_JoystickID(which);

        let name = self.gamepad_subsystem.name_for_id(id);
        let vendor = self.gamepad_subsystem.vendor_for_id(id);
        let product = self.gamepad_subsystem.product_for_id(id);
        let version = self.gamepad_subsystem.product_version_for_id(id);
        let path = self.gamepad_subsystem.path_for_id(id);
        let guid = self.gamepad_subsystem.guid_for_id(id).to_string();

        let in_cfg = &self.cfg.input_gamepad;

        let formatted_name = format!(
            "{} ({}:{} ver. {}) ({} @ {})",
            FmtOpt(&name.as_deref().ok()),
            FmtOptHex(&vendor), FmtOptHex(&product), FmtOptHex(&version),
            guid, FmtOpt(&path)
        );

        let mut formatted_serial = String::new();

        fn check_str(filter: &SingleOrArray<String>, check: &Option<String>) -> Option<bool> {
            let filter = filter.slice();
            
            if !filter.is_empty() {
                let Some(check) = check else {return None};

                return Some(filter.iter().any(|v| v.contains(check) ));
            }

            Some(true)
        }

        fn check_num<T>(filter: &SingleOrArray<T>, check: &Option<T>) -> Option<bool> where T: Eq {
            let filter = filter.slice();
            
            if !filter.is_empty() {
                let Some(check) = check else {return None};

                return Some(filter.contains(check));
            }

            Some(true)
        }

        fn check_str_result<E>(filter: &SingleOrArray<String>, check: &Result<String,E>) -> anyhow::Result<bool> where E: Error {
            let filter = filter.slice();

            if !filter.is_empty() {
                let check = match check {
                    Ok(v) => v,
                    Err(e) => bail!("{e}"),
                };

                return Ok(filter.iter().any(|v| v.contains(check) ));
            }

            Ok(true)
        }

        let mut try_open = || -> anyhow::Result<Option<Gamepad>> {
            if self.input.is_some() {
                return Ok(None);
            }

            if !check_str_result(&in_cfg.filter_name, &name).context("querying gamepad name")?
                || !check_str_result(&in_cfg.filter_path, &path).context("querying gamepad path")?
                || (!in_cfg.filter_guid.slice().is_empty() && !in_cfg.filter_guid.slice().iter().any(|v| guid.contains(v) ))
                || !check_num(&in_cfg.filter_vendor_id, &vendor).context("querying vendor id")?
                || !check_num(&in_cfg.filter_product_id, &product).context("querying product id")?
                || !check_num(&in_cfg.filter_product_version, &version).context("querying product version")?
            {
                return Ok(None);
            }

            let opened = self.gamepad_subsystem.open(id).context("querying gamepad metadata")?;

            let fw_version = opened.firmware_version();
            let serial = opened.serial_number();

            formatted_serial = format!("(snr: {}, fw: {})", FmtOpt(&serial), FmtOptHex(&fw_version));

            if !check_str(&in_cfg.filter_serial, &serial).context("querying gamepad serial")?
                || !check_num(&in_cfg.filter_fw_version, &fw_version).context("querying gamepad fw version")?
            {
                // closing it right away
                return Ok(None);
            }

            Ok(Some(opened))
        };

        match try_open() {
            Ok(Some(mut gamepad)) => {
                check_gamepad_mapping_presence(&gamepad, self.parsed_config);

                SimulatedGamepad::close(&mut self.output)?;
                SimulatedGamepadGyro::close(&mut self.motion_output)?;

                let out = SimulatedGamepad::create(self.cfg, self.parsed_config)?;

                if
                    let Some(gicfg) = &self.cfg.simulate_gamepad_gyro
                    && let Some(pgicfg) = &self.parsed_config.parsed_gyro
                    && gicfg.enable
                {
                    let out = SimulatedGamepadGyro::create(self.cfg, gicfg, self.parsed_config, pgicfg)?;

                    gamepad.sensor_set_enabled(SensorType::Accelerometer, true)?;
                    gamepad.sensor_set_enabled(SensorType::Gyroscope, true)?;

                    self.motion_output = Some(out);
                } else {
                    // Disable unused sensors to maybe reduce gamepad battery usage
                    let _ = gamepad.sensor_set_enabled(SensorType::AccelerometerLeft, false);
                    let _ = gamepad.sensor_set_enabled(SensorType::AccelerometerRight, false);
                    let _ = gamepad.sensor_set_enabled(SensorType::GyroscopeLeft, false);
                    let _ = gamepad.sensor_set_enabled(SensorType::GyroscopeRight, false);
                    let _ = gamepad.sensor_set_enabled(SensorType::Accelerometer, false);
                    let _ = gamepad.sensor_set_enabled(SensorType::Gyroscope, false);
                }

                eprintln!("Opened gamepad: {formatted_name} {formatted_serial}");

                if let Some(mapping) = &self.cfg.input_gamepad.sdl_gamepad_mapping {
                    gamepad.set_mapping(mapping).context("setting sdl gamepad mapping")?;
                }
                
                if self.app_args.verbose {
                    match gamepad.mapping() {
                        Some(mapping) => eprintln!("SDL gamepad mapping: {mapping}"),
                        None => eprintln!("Failed to retrieve SDL gamepad mapping"),
                    }
                }
                
                self.tracker = ButtonTracker::default();
                let calib = CalibrationState::create(self.cfg, self.parsed_config, self.app_args);

                self.input = Some((id, gamepad, calib));
                self.output = Some(out);
            },
            Ok(None) => eprintln!("Ignore gamepad: {formatted_name} {formatted_serial}"),
            Err(e) => eprintln!("Failed to open gamepad: {formatted_name} {formatted_serial}: {e:#}"),
        }

        Ok(())
    }

    pub fn pull_event_batch(&mut self, max: usize, wait_ms: u32) -> anyhow::Result<Vec<Event>> {
        let first = self.event_pump.wait_event_timeout_ms(wait_ms);

        let Some(first) = first else {return Ok(vec![])};

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
                bail!("SDL_PeepEvents error {result}");
            }

            unsafe {
                staging_vec.set_len(result as _);
            }

            for v in staging_vec {
                out_vec.push(Event::from_ll(v));
            }
        }

        Ok(out_vec)
    }
}

pub fn check_gamepad_mapping_presence(gp: &Gamepad, pcfg: &ParsedConfig) {
    let missing_buttons = pcfg.button_bindings.keys()
        .copied()
        .filter(|&v| !gp.has_button(v) )
        .collect::<Vec<_>>();

    let missing_axes = pcfg.axis_bindings.keys()
        .copied()
        .filter(|&v| !gp.has_axis(v) )
        .collect::<Vec<_>>();

    if !missing_buttons.is_empty() {
        eprintln!("Connected gamepad lacks configured buttons: {missing_buttons:?}");
    }
    if !missing_axes.is_empty() {
        eprintln!("Connected gamepad lacks configured axes: {missing_axes:?}");
    }
}
