use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{Context, bail};
use evdev::{AbsInfo, AbsoluteAxisCode, BusType, EvdevEnum, KeyCode, UinputAbsSetup};
use sdl3::gamepad::{Axis, Button};
use sdl3_sys::gamepad::{SDL_GamepadAxis, SDL_GamepadButton};

use crate::config::{AxisMappingEnum, ButtonMappingEnum, Config, SimulateGamepad, StringOrU16, TrThreshold};
use crate::{Args, none_vec};
use crate::simulated_gyro::ParsedGyroConfig;

#[derive(Debug)]
pub struct ParsedConfig {
    pub button_bindings: ParsedButtonBindings,
    pub axis_bindings: ParsedAxisBindings,
    pub evdev_bus_type: BusType,
    pub additional_buttons: Vec<KeyCode>,
    pub additional_axes: Vec<UinputAbsSetup>,
    pub button_lut: Vec<u16>,
    pub axis_lut: Vec<Option<Box<ParsedAxisBinding>>>,
    pub power_refresh_interval: u64,
    pub rumble_mul: [u64;2],
    pub parsed_gyro: Option<ParsedGyroConfig>,
    pub dpad_to_hat_axis: Option<[AbsoluteAxisCode;2]>,
    pub stickgroup: Vec<ParsedStickGroup>,
}

impl ParsedConfig {
    pub fn parse(cfg: &Config, _app_args: &Args) -> anyhow::Result<Self> {
        let evdev_bus_type = cfg.simulate_gamepad.bus_type.clone().unwrap_or(BusType::BUS_USB.0.into());
        let evdev_bus_type = match_bus_type(&evdev_bus_type).context("parsing bus type")?;

        let mut axis_exclusions = HashSet::new();
        let mut additional_axes = Vec::new();


        let mut button_exclusions = HashSet::new();
        let mut additional_buttons = Vec::new();

        let mut dpad_to_hat_axis = None;

        if cfg.behavior.dpad_to_hat {
            let abs = AbsInfo::new(0, -1, 1, 0, 0, 0);

            let mut calc_dim = |dim: usize, alt: AbsoluteAxisCode| -> anyhow::Result<AbsoluteAxisCode> {
                let axis = cfg.behavior.dpad_to_hat_axis
                    .as_ref()
                    .map(|v| match_axis_code(&v[dim]) )
                    .transpose()?
                    .unwrap_or(alt);

                let setup = UinputAbsSetup::new(axis, abs);

                axis_exclusions.insert(axis);
                additional_axes.push(setup);

                Ok(axis)
            };

            dpad_to_hat_axis = Some([
                calc_dim(0, AbsoluteAxisCode::ABS_HAT0X)?,
                calc_dim(1, AbsoluteAxisCode::ABS_HAT0Y)?,
            ]);
        }

        let mut axis_bindings = parse_axis_bindings(&cfg.axis_map, &cfg.simulate_gamepad, axis_exclusions)?;

        if cfg.behavior.simulate_digital_trigger {
            let mut add = |code: KeyCode| {
                button_exclusions.insert(code);
                additional_buttons.push(code);
            };

            for m in axis_bindings.values() {
                if let Some(v) = m.digitrigger_button {
                    add(v);
                }
            }
        }

        let button_bindings = parse_button_bindings(&cfg.button_map, !cfg.behavior.dpad_to_dpad, button_exclusions)?;

        let mut stickgroup = Vec::with_capacity(cfg.sticks.len());

        for (idx,(name,sg)) in cfg.sticks.iter().enumerate() {
            let deadzone_release = sg.deadzone_release.unwrap_or(sg.deadzone);
            let deadzone_bend = sg.deadzone_bend.unwrap_or(deadzone_release);

            if sg.deadzone >= 1. || deadzone_bend < 0. || deadzone_release > sg.deadzone || deadzone_bend > deadzone_release {
                bail!("stickgroup: must be: deadzone >= deadzone_release, deadzone_release >= deadzone_bend, deadzone_bend >= 0");
            }

            let mut pag = ParsedStickGroup {
                name: name.clone(),
                axes: [Axis::LeftX,Axis::LeftY],
                process: sg.process,
                deadzone: sg.deadzone,
                deadzone_release,
                deadzone_bend,
                in_scale: sg.in_scale.unwrap_or(1.),
                out_scale: sg.out_scale.unwrap_or(1.),
                out_clamp: sg.out_clamp.unwrap_or(1.01),
            };

            if !(
                pag.deadzone.is_finite() && pag.deadzone_release.is_finite() && pag.deadzone_bend.is_finite()
                && pag.in_scale.is_finite() && pag.out_scale.is_finite() && pag.out_clamp.is_finite()
            ) {
                bail!("stickgroup has invalid float values");
            }

            if pag.out_clamp < 0. {
                bail!("stickgroup: out_clamp must not be negative");
            }

            for dim in [false, true] {
                let id = match_sdl_axis(&sg.axis[dim as usize])?;
                pag.axes[dim as usize] = id;

                if let Some(b) = axis_bindings.get_mut(&id) {
                    b.axisgroup = Some((idx, dim));
                }
            }

            stickgroup.push(pag);
        }

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

        let rumble_mul = [
            cfg.behavior.rumble_multiplier_left.map_or(65535, |v| ((v.abs() * 65535.).round() as u64).min(65535*2)),
            cfg.behavior.rumble_multiplier_right.map_or(65535, |v| ((v.abs() * 65535.).round() as u64).min(65535*2)),
        ];

        let parsed_gyro = cfg.simulate_gamepad_gyro.as_ref().map(ParsedGyroConfig::parse).transpose()?;

        Ok(ParsedConfig {
            button_bindings,
            axis_bindings,
            evdev_bus_type,
            additional_buttons,
            additional_axes,
            button_lut,
            axis_lut,
            power_refresh_interval,
            rumble_mul,
            parsed_gyro,
            dpad_to_hat_axis,
            stickgroup,
        })
    }
}

pub type ParsedButtonBindings = HashMap<Button,ParsedButtonBinding>;
pub type ParsedAxisBindings = HashMap<Axis,ParsedAxisBinding>;

#[derive(Debug)]
pub struct ParsedButtonBinding {
    pub code: KeyCode,
    pub state: bool,
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
    pub digitrigger_button: Option<KeyCode>,
    pub digitrigger_thresh: [i32;2],
    /// (idx,dim)
    pub axisgroup: Option<(usize,bool)>,
}

#[derive(Clone, Debug)]
pub struct ParsedStickGroup {
    pub name: String,
    pub axes: [Axis;2],
    pub process: bool,
    pub deadzone: f32,
    pub deadzone_release: f32,
    pub deadzone_bend: f32,
    pub in_scale: f32,
    pub out_scale: f32,
    pub out_clamp: f32,
}

fn parse_button_bindings(cfg: &HashMap<String,ButtonMappingEnum>, exclude_dpad: bool, mut exclusions: HashSet<KeyCode>) -> anyhow::Result<ParsedButtonBindings> {
    let mut out = HashMap::new();

    for (k,v) in cfg {
        let key = match_sdl_button(&StringOrU16::String(k.clone()))?;
        let binding = parse_button_binding(v, exclude_dpad).with_context(|| format!("parsing button binding for {k}"))?;
        let Some(binding) = binding else {continue};

        if !exclusions.insert(binding.code) {
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
        state: false,
    }))
}

pub fn parse_axis_binding(cfg: &AxisMappingEnum, i: &SimulateGamepad, absolute: bool) -> anyhow::Result<ParsedAxisBinding> {
    let cfg = cfg.mapping();

    let code = match_axis_code(&cfg.key)?;

    let imin = if absolute {0} else {-32768};

    let initial = cfg.initial.unwrap_or(0);
    let fuzz = cfg.fuzz.or(i.default_axis_fuzz).unwrap_or(0);
    let flat = cfg.flat.or(i.default_axis_flat).unwrap_or(0);
    let res = cfg.res.or(i.default_axis_res).unwrap_or(0);
    let [min,max] = cfg.out_range.unwrap_or([imin,32767]);

    if max < min {
        bail!("axis out_range must be smaller or equal number first");
    }

    if initial > max || initial < min {
        bail!("axis initial value must be within out_range");
    }

    let imin = if min >= 0 {0} else {-32768};

    let [ia,ib,ic] = cfg.from_range.unwrap_or([imin,0,32767]);
    let [oa,ob,oc] = cfg.to_range.unwrap_or([min,0,max]);

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

    let digitrigger_button = cfg.digitrigger_button.map(|v| match_key_code(&v) ).transpose()?;

    let digitrigger_thresh = cfg.digitrigger_thresh.unwrap_or([TrThreshold::F64(0.8), TrThreshold::F64(0.75)]);
    let digitrigger_thresh = digitrigger_thresh.map(|v| match v {
        _ if ic <= ib + 2 => 0,
        TrThreshold::F64(v) if v.is_finite() && v.is_sign_negative() => {
            (v * (ib-ia) as f64) as i32
        },
        TrThreshold::F64(v) if v.is_finite() && v.is_sign_positive() => {
            (v * (ic-ib) as f64) as i32
        },
        TrThreshold::Abs { abs } => abs,
        _ => 0,
    });

    if digitrigger_thresh[0] == digitrigger_thresh[1]
        || digitrigger_thresh[0].signum() != digitrigger_thresh[1].signum()
        || digitrigger_thresh[1].abs() > digitrigger_thresh[0].abs()
    {
        bail!("Invalid digitrigger press and release thresholds defined. both or none must; be zero or non-zero, have same sign, and release must be < press");
    }

    Ok(ParsedAxisBinding {
        setup: UinputAbsSetup::new(
            code,
            AbsInfo::new(initial, min, max, fuzz, flat, res)
        ),
        in_off,
        out_off,
        neg_fraction,
        pos_fraction,
        clamp_out,
        out_range: [min,max],
        digitrigger_button,
        digitrigger_thresh,
        axisgroup: None,
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

pub fn match_sdl_axis(id: &StringOrU16) -> anyhow::Result<Axis> {
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

pub fn match_key_code(id: &StringOrU16) -> anyhow::Result<KeyCode> {
    match id {
        StringOrU16::String(v) => match match_key_code_string(v) {
            Some(v) if v.0 == u16::MAX => bail!("udev KeyCode not supported"),
            Some(v) => Ok(v),
            None => bail!("Unknown udev KeyCode: {v}"),
        },
        StringOrU16::U16(v) => Ok(KeyCode::from_index(*v as _)),
    }
}

pub fn match_axis_code(id: &StringOrU16) -> anyhow::Result<AbsoluteAxisCode> {
    match id {
        StringOrU16::String(v) => match match_axis_code_string(v) {
            Some(v) => Ok(v),
            None => bail!("Unknown udev AbsoluteAxisCode: {v}"),
        },
        StringOrU16::U16(v) => Ok(AbsoluteAxisCode::from_index(*v as _)),
    }
}

pub fn match_bus_type(id: &StringOrU16) -> anyhow::Result<BusType> {
    match id {
        StringOrU16::String(v) => match BusType::from_str(v) {
            Ok(v) => Ok(v),
            Err(e) => bail!("Unknown udev BusType: {v}: {e:#}"),
        },
        StringOrU16::U16(v) => Ok(BusType::from_index(*v as _)),
    }
}

pub fn match_key_code_string(id: &str) -> Option<KeyCode> {
    KeyCode::from_str(id).ok().or_else(||
        KeyCode::from_str(&format!("BTN_{}", id.to_uppercase())).ok()
    )
}

pub fn match_axis_code_string(id: &str) -> Option<AbsoluteAxisCode> {
    AbsoluteAxisCode::from_str(id).ok().or_else(||
        AbsoluteAxisCode::from_str(&format!("ABS_{}", id.to_uppercase())).ok()
    )
}

pub fn match_sdl_button_string(id: &str) -> Option<Button> {
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

pub fn match_sdl_axis_string(id: &str) -> Option<Axis> {
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
