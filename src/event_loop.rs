use evdev::{EventType, InputEvent, MiscCode};
use sdl3_sys::timer::SDL_GetTicksNS;

use crate::Args;
use crate::event_processing::LoopState;
use crate::button_tracker::ButtonTracker;
use crate::config::Config;
use crate::parsed_config::ParsedConfig;

pub fn entry(cfg: &Config, parsed_config: &ParsedConfig, app_args: &Args) -> anyhow::Result<()> {
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");

    for (k, v) in &cfg.sdl_hints {
        sdl3::hint::set(k, v);
    }

    let sdl_context = sdl3::init()?;
    let gamepad_subsystem = sdl_context.gamepad()?;
    let event_pump = sdl_context.event_pump()?;
    let event_subsystem = sdl_context.event()?;

    let mut ls = LoopState {
        sdl_context,
        gamepad_subsystem,
        event_pump,
        event_subsystem,
        cfg,
        parsed_config,
        app_args,
        exit: false,
        output: None,
        motion_output: None,
        input: None,
        tracker: ButtonTracker::default(),
        tick: 0,
        last_power_check: 0,
        last_power_info: None,
    };

    eprintln!("SDL initialized");

    let wait_timeout_ms = cfg.input_gamepad.wait_timeout_ms.unwrap_or(5);
    let wait_timeout_ms_idle = cfg.input_gamepad.wait_timeout_ms_idle.unwrap_or(5000);
    let max_batch_size = cfg.input_gamepad.input_event_batch_size.unwrap_or(22);

    // let mut total_counter = 0u64;
    // let mut batch_counters = vec![0u64; 65];

    loop {
        let wait_timeout_ms = if ls.input.is_some() {wait_timeout_ms} else {wait_timeout_ms_idle};

        let events = ls.pull_event_batch(max_batch_size, wait_timeout_ms)?;

        // total_counter += events.len() as u64;
        // batch_counters[events.len()] += 1;

        ls.tick = events.first().map_or(unsafe { SDL_GetTicksNS() }, |e| e.get_timestamp() );

        for e in events {
            ls.process_event(e)?;
        }

        if let Some(out) = &mut ls.output {
            if let Some((_,_,calib)) = &mut ls.input {
                calib.submit(out);
            }
            if !out.queue.is_empty() && cfg.simulate_gamepad.emit_timestamp {
                out.queue.push(InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, (ls.tick / 1000) as i32));
            }
            out.submit()?;
        }
        if let Some(out) = &mut ls.motion_output {
            if !out.queue.is_empty() {
                out.queue.push(InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, (ls.tick / 1000) as i32));
            }
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

        ls.process_rumble()?;
    }

    // eprintln!("Total events: {total_counter}");
    // eprintln!("Batch sizes: {batch_counters:?}");

    Ok(())
}
