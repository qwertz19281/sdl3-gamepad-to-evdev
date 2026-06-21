use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Context, bail};
use evdev::{AbsInfo, AbsoluteAxisCode, EvdevEnum, KeyCode, UinputAbsSetup};
use sdl3::gamepad::Button;
use sdl3_sys::gamepad::{SDL_GamepadAxis, SDL_GamepadButton};

use crate::config::{AxisMapping, ButtonMapping, SimulateGamepad, StringOrU16};


type ParsedButtonBindings = HashMap<SDL_GamepadButton,ParsedButtonBinding>;
type ParsedAxisBindings = HashMap<SDL_GamepadAxis,ParsedAxisBinding>;

pub struct ParsedButtonBinding {
    pub code: KeyCode,
}

pub struct ParsedAxisBinding {
    pub setup: UinputAbsSetup,
    pub numerator: i32,
    pub denominator: i32,
}

fn parse_button_binding(v: &ButtonMapping, exclude_dpad: bool) -> anyhow::Result<Option<ParsedButtonBinding>> {
    match v {
        ButtonMapping::Code(key) => {
            Ok(Some(ParsedButtonBinding {
                code: match_key_code(key)?
            }))
        },
        ButtonMapping::Advanced { key, dpad } => {
            if exclude_dpad && *dpad {
                return Ok(None);
            }
            Ok(Some(ParsedButtonBinding {
                code: match_key_code(key)?
            }))
        },
    }
}

fn parse_axis_binding(v: &AxisMapping, i: &SimulateGamepad) -> anyhow::Result<ParsedAxisBinding> {
    let code;
    let mut numer = 1;
    let mut denom = 1;
    // TODO can we query the sdl gamepad for default min/max of axis?
    let mut min_ = -32768;
    let mut max_ = 32767;
    let mut fuzz_ = i.default_axis_fuzz.unwrap_or(0);
    let mut flat_ = i.default_axis_flat.unwrap_or(0);
    let mut res_ = i.default_axis_res.unwrap_or(0);

    match v {
        AxisMapping::Code(axis) => code = match_axis_code(axis)?,
        AxisMapping::Advanced { key: axis, numerator, denominator, invert, min, max, fuzz, flat, res } => {
            code = match_axis_code(axis)?;
            numer = numerator.unwrap_or(1);
            denom = denominator.unwrap_or(1);
            if let Some(v) = numerator {
                numer = *v;
            }
            if let Some(v) = denominator {
                denom = *v;
            }
            if denom == 0 {
                bail!("denominator must not be zero");
            }
            if *invert {
                denom = denom.checked_neg().context("nominator overflow")?;
            }
            if let Some(v) = min {
                min_ = *v;
            }
            if let Some(v) = max {
                max_ = *v;
            }
            if let Some(v) = fuzz {
                fuzz_ = *v;
            }
            if let Some(v) = flat {
                flat_ = *v;
            }
            if let Some(v) = res {
                res_ = *v;
            }
        },
    }

    Ok(ParsedAxisBinding {
        setup: UinputAbsSetup::new(
            code,
            AbsInfo::new(0, min_, max_, fuzz_, flat_, res_)
        ),
        numerator: numer,
        denominator: denom,
    })
}

fn match_sdl_button(id: &StringOrU16) -> anyhow::Result<Button> {
    match id {
        StringOrU16::String(v) => match Button::from_string(&v) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 button id: {v}"),
        },
        StringOrU16::U16(v) => match Button::from_ll(SDL_GamepadButton(*v as _)) {
            Some(v) => Ok(v),
            None => bail!("Unknown SDL3 button id: {v}"),
        },
    }
}

fn match_key_code(id: &StringOrU16) -> anyhow::Result<KeyCode> {
    match id {
        StringOrU16::String(v) => match KeyCode::from_str(v) {
            Ok(v) => Ok(v),
            Err(e) => bail!("Unknown udev KeyCode: {v}: {e}"),
        },
        StringOrU16::U16(v) => Ok(KeyCode::from_index(*v as _)),
    }
}

fn match_axis_code(id: &StringOrU16) -> anyhow::Result<AbsoluteAxisCode> {
    match id {
        StringOrU16::String(v) => match AbsoluteAxisCode::from_str(v) {
            Ok(v) => Ok(v),
            Err(e) => bail!("Unknown udev AbsoluteAxisCode: {v}: {e}"),
        },
        StringOrU16::U16(v) => Ok(AbsoluteAxisCode::from_index(*v as _)),
    }
}
