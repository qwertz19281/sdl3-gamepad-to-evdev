use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{Context, bail};
use evdev::{AbsInfo, AbsoluteAxisCode, BusType, EvdevEnum, KeyCode, UinputAbsSetup};
use sdl3::gamepad::{Axis, Button};
use sdl3_sys::gamepad::{SDL_GamepadAxis, SDL_GamepadButton};

use crate::config::{AxisMappingEnum, ButtonMappingEnum, Config, SimulateGamepad, StringOrU16};
use crate::none_vec;

#[derive(Debug)]
pub struct ParsedConfig {
    pub button_bindings: ParsedButtonBindings,
    pub axis_bindings: ParsedAxisBindings,
    pub evdev_bus_type: BusType,
    pub additional_axes: Vec<UinputAbsSetup>,
    pub button_lut: Vec<u16>,
    pub axis_lut: Vec<Option<Box<ParsedAxisBinding>>>,
    pub power_refresh_interval: u64,
}

impl ParsedConfig {
    pub fn parse(cfg: &Config) -> anyhow::Result<Self> {
        let evdev_bus_type = cfg.simulate_gamepad.bus_type.clone().unwrap_or(BusType::BUS_USB.0.into());
        let evdev_bus_type = match_bus_type(&evdev_bus_type).context("parsing bus type")?;

        let mut axis_exclusions = HashSet::new();
        let mut additional_axes = Vec::new();

        if cfg.behavior.dpad_to_hat0 {
            let mut add = |setup: UinputAbsSetup| {
                axis_exclusions.insert(AbsoluteAxisCode(setup.code()));
                additional_axes.push(setup);
            };

            add(UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0X,
                AbsInfo::new(0, -1, 1, 0, 0, 0),
            ));
            add(UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0Y,
                AbsInfo::new(0, -1, 1, 0, 0, 0),
            ));
        }

        let button_bindings = parse_button_bindings(&cfg.button_map, !cfg.behavior.dpad_to_dpad)?;
        let axis_bindings = parse_axis_bindings(&cfg.axis_map, &cfg.simulate_gamepad, axis_exclusions)?;

        let max_button_id = button_bindings.keys().map(|v| v.to_ll().0 ).max().unwrap_or(0).max(SDL_GamepadButton::COUNT.0);
        let mut button_lut = vec![u16::MAX; max_button_id as _];

        for (k,v) in &button_bindings {
            if k.to_ll().0 >= 0 {
                button_lut[k.to_ll().0 as usize] = v.code.0;
            }
        }

        let max_axis_id = axis_bindings.keys().map(|v| v.to_ll().0 ).max().unwrap_or(0).max(SDL_GamepadAxis::COUNT.0);
        let mut axis_lut = none_vec(max_axis_id as _);

        for (k,v) in &axis_bindings {
            if k.to_ll().0 >= 0 {
                axis_lut[k.to_ll().0 as usize] = Some(Box::new(v.clone()));
            }
        }

        let power_refresh_interval= cfg.input_gamepad.power_refresh_interval.unwrap_or(5000) as u64 * 1_000_000;

        Ok(ParsedConfig {
            button_bindings,
            axis_bindings,
            evdev_bus_type,
            additional_axes,
            button_lut,
            axis_lut,
            power_refresh_interval,
        })
    }
}

pub type ParsedButtonBindings = HashMap<Button,ParsedButtonBinding>;
pub type ParsedAxisBindings = HashMap<Axis,ParsedAxisBinding>;

#[derive(Debug)]
pub struct ParsedButtonBinding {
    pub code: KeyCode,
}

#[derive(Clone, Debug)]
pub struct ParsedAxisBinding {
    pub setup: UinputAbsSetup,
    pub in_off: i64,
    pub out_off: i64,
    pub neg_fraction: [i64;2],
    pub pos_fraction: [i64;2],
    pub clamp_out: [i32;2],
    pub out_range: [i32;2],
}

fn parse_button_bindings(cfg: &HashMap<String,ButtonMappingEnum>, exclude_dpad: bool) -> anyhow::Result<ParsedButtonBindings> {
    let mut out = HashMap::new();
    let mut out_keys = HashSet::new();

    for (k,v) in cfg {
        let key = match_sdl_button(&StringOrU16::String(k.clone()))?;
        let binding = parse_button_binding(v, exclude_dpad).with_context(|| format!("parsing button binding for {k}"))?;
        let Some(binding) = binding else {continue};

        if !out_keys.insert(binding.code) {
            bail!("{k} adds duplicate output key {}, which is (currently) not supported", binding.code.code());
        }

        out.insert(key, binding);
    }

    Ok(out)
}

fn parse_axis_bindings(cfg: &HashMap<String,AxisMappingEnum>, sg: &SimulateGamepad, mut exclusions: HashSet<AbsoluteAxisCode>) -> anyhow::Result<ParsedAxisBindings> {
    let mut out = HashMap::new();

    for (k,v) in cfg {
        let key = match_sdl_axis(&StringOrU16::String(k.clone()))?;
        let absolute = key.string().to_lowercase().contains("trigger");
        let binding: ParsedAxisBinding = parse_axis_binding(v, sg, absolute).with_context(|| format!("parsing axis binding for {k}"))?;

        if !exclusions.insert(AbsoluteAxisCode(binding.setup.code())) {
            bail!("{k} adds duplicate output key {:?}, which is (currently) not supported", AbsoluteAxisCode(binding.setup.code()));
        }

        out.insert(key, binding);
    }

    Ok(out)
}

pub fn parse_button_binding(cfg: &ButtonMappingEnum, exclude_dpad: bool) -> anyhow::Result<Option<ParsedButtonBinding>> {
    let cfg = cfg.mapping();

    if exclude_dpad && cfg.dpad {
        return Ok(None);
    }

    Ok(Some(ParsedButtonBinding {
        code: match_key_code(&cfg.key)?,
    }))
}

pub fn parse_axis_binding(cfg: &AxisMappingEnum, i: &SimulateGamepad, absolute: bool) -> anyhow::Result<ParsedAxisBinding> {
    let cfg = cfg.mapping();

    let code = match_axis_code(&cfg.key)?;

    let imin = if absolute {0} else {-32768};

    let fuzz = cfg.fuzz.or(i.default_axis_fuzz).unwrap_or(0);
    let flat = cfg.flat.or(i.default_axis_flat).unwrap_or(0);
    let res = cfg.res.or(i.default_axis_res).unwrap_or(0);
    let [min,max] = cfg.out_range.unwrap_or([imin,32767]);

    if max < min {
        bail!("axis out_range must be smaller or equal number first");
    }

    let imin = if min >= 0 {0} else {-32768};

    let [ia,ib,ic] = cfg.from_range.unwrap_or([imin,0,32767]);
    let [oa,ob,oc] = cfg.from_range.unwrap_or([min,0,max]);

    if !(ia <= ib && ib <= ic) {
        bail!("axis from_range must not be decrementing order");
    }
    if !((ia <= ib && ib <= ic) || (ia >= ib && ib >= ic)) {
        bail!("axis to_range must be in consistent order");
    }

    // transform:
    // if < ib, use neg_transform
    // if > ib, use pos_transform
    let neg_fraction = [
        (ob - oa) as i64,
        (ib - ia).max(1) as i64,
    ];
    let pos_fraction = [
        (oc - ob) as i64,
        (ic - ib).max(1) as i64,
    ];
    let in_off = ib as i64;
    let out_off = ob as i64;

    
    let clamp_out = [
        min.max(oa.min(oc)),
        max.min(oa.max(oc)),
    ];

    Ok(ParsedAxisBinding {
        setup: UinputAbsSetup::new(
            code,
            AbsInfo::new(0, min, max, fuzz, flat, res)
        ),
        in_off,
        out_off,
        neg_fraction,
        pos_fraction,
        clamp_out,
        out_range: [min,max],
    })
}

fn match_sdl_button(id: &StringOrU16) -> anyhow::Result<Button> {
    match id {
        StringOrU16::String(v) => match match_sdl_button_string(v) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 button id: {v}"),
        },
        StringOrU16::U16(v) => match Button::from_ll(SDL_GamepadButton(*v as _)) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 button id: {v}"),
        },
    }
}

fn match_sdl_axis(id: &StringOrU16) -> anyhow::Result<Axis> {
    match id {
        StringOrU16::String(v) => match match_sdl_axis_string(v) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 axis id: {v}"),
        },
        StringOrU16::U16(v) => match Axis::from_ll(SDL_GamepadAxis(*v as _)) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 axis id: {v}"),
        },
    }
}

fn match_key_code(id: &StringOrU16) -> anyhow::Result<KeyCode> {
    match id {
        StringOrU16::String(v) => match match_key_code_string(v) {
            Some(v) if v.0 == u16::MAX => bail!("udev KeyCode not supported"),
            Some(v) => Ok(v),
            None => bail!("Unknown udev KeyCode: {v}"),
        },
        StringOrU16::U16(v) => Ok(KeyCode::from_index(*v as _)),
    }
}

fn match_axis_code(id: &StringOrU16) -> anyhow::Result<AbsoluteAxisCode> {
    match id {
        StringOrU16::String(v) => match match_axis_code_string(v) {
            Some(v) => Ok(v),
            None => bail!("Unknown udev AbsoluteAxisCode: {v}"),
        },
        StringOrU16::U16(v) => Ok(AbsoluteAxisCode::from_index(*v as _)),
    }
}

pub(super) fn match_bus_type(id: &StringOrU16) -> anyhow::Result<BusType> {
    match id {
        StringOrU16::String(v) => match BusType::from_str(v) {
            Ok(v) => Ok(v),
            Err(e) => bail!("Unknown udev BusType: {v}: {e:#}"),
        },
        StringOrU16::U16(v) => Ok(BusType::from_index(*v as _)),
    }
}

fn match_key_code_string(id: &str) -> Option<KeyCode> {
    KeyCode::from_str(id).ok().or_else(||
        KeyCode::from_str(&format!("BTN_{}", id.to_uppercase())).ok()
    )
}

fn match_axis_code_string(id: &str) -> Option<AbsoluteAxisCode> {
    AbsoluteAxisCode::from_str(id).ok().or_else(||
        AbsoluteAxisCode::from_str(&format!("ABS_{}", id.to_uppercase())).ok()
    )
}

fn match_sdl_button_string(id: &str) -> Option<Button> {
    let id_lc = id.replace(['_',' '], "").to_lowercase();

    Some(match &*id_lc {
        "north" | "n" => Button::North,
        "east" | "e" => Button::East,
        "south" | "s" => Button::South,
        "west" | "w" => Button::West,
        "back" | "select" => Button::Back,
        "guide" | "steam" => Button::Guide,
        "start" => Button::Start,
        "leftstick" | "l3" => Button::LeftStick,
        "rightstick" | "r3" => Button::RightStick,
        "leftshoulder" | "l1"  => Button::LeftShoulder,
        "rightshoulder" | "r1" => Button::RightShoulder,
        "dpadup" | "dpadu" | "dpu" => Button::DPadUp,
        "dpaddown" | "dpadd" | "dpd" => Button::DPadDown,
        "dpadleft" | "dpadl" | "dpl" => Button::DPadLeft,
        "dpadright" | "dpadr" | "dpr" => Button::DPadRight,
        "misc1" => Button::Misc1,
        "misc2" => Button::Misc2,
        "misc3" => Button::Misc3,
        "misc4" => Button::Misc4,
        "misc5" => Button::Misc5,
        "misc6" => Button::Misc6,
        "rightpaddle1" => Button::RightPaddle1,
        "leftpaddle1" => Button::LeftPaddle1,
        "rightpaddle2" => Button::RightPaddle2,
        "leftpaddle2" => Button::LeftPaddle2,
        "touchpad" => Button::Touchpad,
        _ => return Button::from_string(id)
    })
}

fn match_sdl_axis_string(id: &str) -> Option<Axis> {
    let id_lc = id.replace(['_',' '], "").to_lowercase();

    Some(match &*id_lc {
        "leftx" | "lx" => Axis::LeftX,
        "rightx" | "rx" => Axis::RightX,
        "lefty" | "ly" => Axis::LeftY,
        "righty" | "ry" => Axis::RightY,
        "triggerleft" | "lefttrigger" | "ltrigger" | "triggerl" | "tl" | "lt" | "l2" => Axis::TriggerLeft,
        "triggerright" | "righttrigger" | "rtrigger" | "triggerr" | "tr" | "rt" | "r2" => Axis::TriggerRight,
        _ => return Axis::from_string(id)
    })
}
