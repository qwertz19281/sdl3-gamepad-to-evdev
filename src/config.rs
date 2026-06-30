use std::collections::HashMap;
use std::slice;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub input_gamepad: InputGamepad,
    pub simulate_gamepad: SimulateGamepad,
    #[serde(default)]
    pub simulate_gamepad_gyro: Option<SimulateGamepadGyro>,
    pub behavior: Behavior,
    //#[serde(alias = "foo", alias = "bar")]
    #[serde(default, alias = "buttons")]
    pub button_map: HashMap<String,ButtonMappingEnum>,
    #[serde(default, alias = "axes")]
    pub axis_map: HashMap<String,AxisMappingEnum>,
    #[serde(default)]
    pub sdl_hints: HashMap<String,String>,
    #[serde(default)]
    pub sticks: HashMap<String,StickGroup>,
    #[serde(default, alias = "hats")]
    pub hat_map: HashMap<u8,[StringOrU16;2]>,
}

#[derive(Debug, Deserialize)]
pub struct InputGamepad {
    #[serde(default)]
    pub filter_name: SingleOrArray<String>,
    #[serde(default)]
    pub filter_vendor_id: SingleOrArray<u16>,
    #[serde(default)]
    pub filter_product_id: SingleOrArray<u16>,
    #[serde(default)]
    pub filter_product_version: SingleOrArray<u16>,
    #[serde(default)]
    pub filter_fw_version: SingleOrArray<u16>,
    #[serde(default)]
    pub filter_serial: SingleOrArray<String>,
    #[serde(default)]
    pub filter_guid: SingleOrArray<String>,
    #[serde(default)]
    pub filter_path: SingleOrArray<String>,
    #[serde(default)]
    pub wait_timeout_ms: Option<u32>,
    #[serde(default)]
    pub wait_timeout_ms_idle: Option<u32>,
    #[serde(default)]
    pub power_refresh_interval: Option<u32>,
    /// currently defaults to 32
    #[serde(default)]
    pub input_event_batch_size: Option<usize>,
    #[serde(default)]
    pub sdl_gamepad_mapping: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SimulateGamepad {
    pub name: String,
    #[serde(default)]
    pub bus_type: Option<StringOrU16>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    #[serde(default)]
    pub default_axis_fuzz: Option<i32>,
    #[serde(default)]
    pub default_axis_flat: Option<i32>,
    #[serde(default)]
    pub default_axis_res: Option<i32>,
    /// TODO not yet implemented
    #[serde(default)]
    pub keep_open_out_gamepad: Option<bool>,
    #[serde(default)]
    pub emit_timestamp: bool,
    #[serde(default)]
    pub enable_rumble: bool, // TODO move to behavior?
}

#[derive(Debug, Deserialize)]
pub struct SimulateGamepadGyro {
    pub enable: bool,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    #[serde(default)]
    pub accel_mul: Option<[f64;3]>,
    #[serde(default)]
    pub gyro_mul: Option<[f64;3]>,
    #[serde(default)]
    pub accel_mul_raw: Option<[f32;3]>,
    #[serde(default)]
    pub gyro_mul_raw: Option<[f32;3]>,
    #[serde(default)]
    pub accel_out_range: Option<[i32;2]>,
    #[serde(default)]
    pub gyro_out_range: Option<[i32;2]>,
    #[serde(default)]
    pub accel_fuzz: Option<i32>,
    #[serde(default)]
    pub accel_flat: Option<i32>,
    #[serde(default)]
    pub accel_res: Option<i32>,
    #[serde(default)]
    pub gyro_fuzz: Option<i32>,
    #[serde(default)]
    pub gyro_flat: Option<i32>,
    #[serde(default)]
    pub gyro_res: Option<i32>,
}


#[derive(Debug, Deserialize)]
pub struct Behavior {
    #[serde(default)]
    pub dpad_to_dpad: bool,
    #[serde(default)]
    pub dpad_to_hat: bool,
    /// defaults to hat0
    #[serde(default)]
    pub dpad_to_hat_axis: Option<[StringOrU16;2]>,
    #[serde(default)]
    pub rumble_multiplier_left: Option<f64>,
    #[serde(default)]
    pub rumble_multiplier_right: Option<f64>,
    #[serde(default)]
    pub simulate_digital_trigger: bool,
}

#[derive(Debug, Deserialize)]
pub struct StickGroup {
    /// which x and y axes should be grouped
    pub axis: [StringOrU16;2],
    /// whether deadzone and scale parameters declared here should be applied. default: false
    #[serde(default)]
    pub process: bool,
    #[serde(default)]
    /// deadzone in units 0.0 - 1.0
    pub deadzone: f32,
    #[serde(default)]
    pub deadzone_release: Option<f32>,
    #[serde(default)]
    pub deadzone_bend: Option<f32>,
    /// scaling applied to stick radius before deadzone transform. default: 1.0
    #[serde(default)]
    pub in_scale: Option<f32>,
    /// scaling applied to stick radius after deadzone transform. default: 1.0
    #[serde(default)]
    pub out_scale: Option<f32>,
    /// clamp maximum relative output radius. default: 1.01
    #[serde(default)]
    pub out_clamp: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ButtonMappingEnum {
    Code(StringOrU16),
    Advanced(ButtonMapping),
}

#[derive(Clone, Debug, Deserialize)]
pub struct ButtonMapping {
    #[serde(alias = "code")]
    pub key: StringOrU16,
    #[serde(default)]
    pub dpad: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum AxisMappingEnum {
    Code(StringOrU16),
    Advanced(AxisMapping),
}

#[derive(Clone, Debug, Deserialize)]
pub struct AxisMapping {
    #[serde(alias = "code")]
    pub key: StringOrU16,
    /// from_range will not be used if stick correction is active
    #[serde(default)]
    pub from_range: Option<[i32;3]>,
    #[serde(default)]
    pub to_range: Option<[i32;3]>,
    #[serde(default)]
    pub out_range: Option<[i32;2]>,
    #[serde(default)]
    pub initial: Option<i32>,
    #[serde(default)]
    pub fuzz: Option<i32>,
    #[serde(default)]
    pub flat: Option<i32>,
    #[serde(default)]
    pub res: Option<i32>,
    #[serde(default)]
    pub digitrigger_button: Option<StringOrU16>,
    #[serde(default)]
    pub digitrigger_thresh: Option<[TrThreshold;2]>,
}

impl ButtonMappingEnum {
    pub fn mapping(&self) -> ButtonMapping {
        match self {
            Self::Code(key) => ButtonMapping {
                key: key.clone(),
                dpad: false,
            },
            Self::Advanced(button_mapping) => button_mapping.clone(),
        }
    }
}

impl AxisMappingEnum {
    pub fn mapping(&self) -> AxisMapping {
        match self {
            Self::Code(key) => AxisMapping {
                key: key.clone(),
                from_range: None,
                to_range: None,
                out_range: None,
                initial: None,
                fuzz: None,
                flat: None,
                res: None,
                digitrigger_button: None,
                digitrigger_thresh: None,
            },
            Self::Advanced(axis_mapping) => axis_mapping.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum StringOrU16 {
    String(String),
    U16(u16),
}

impl From<u16> for StringOrU16 {
    fn from(value: u16) -> Self {
        StringOrU16::U16(value)
    }
}

impl From<String> for StringOrU16 {
    fn from(value: String) -> Self {
        StringOrU16::String(value)
    }
}

impl From<&str> for StringOrU16 {
    fn from(value: &str) -> Self {
        StringOrU16::String(value.to_owned())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum TrThreshold {
    F64(f64),
    Abs { abs: i32 },
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum SingleOrArray<T> {
    Single(T),
    Vec(Vec<T>),
    #[default]
    Empty,
}

impl<T> SingleOrArray<T> {
    pub fn slice(&self) -> &[T] {
        match self {
            Self::Single(v) => slice::from_ref(v),
            Self::Vec(v) => v,
            Self::Empty => &[],
        }
    }
}

#[allow(unused)]
fn default_true() -> bool {
    true
}
