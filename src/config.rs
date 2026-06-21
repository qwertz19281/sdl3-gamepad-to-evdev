use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub input_gamepad: InputGamepad,
    pub simulate_gamepad: SimulateGamepad,
    pub behavior: Behavior,
    pub button_map: HashMap<String,ButtonMapping>,
    pub axis_map: HashMap<String,AxisMapping>,
}

#[derive(Debug, Deserialize)]
pub struct InputGamepad {
    #[serde(default)]
    pub filter_name: Option<String>,
    #[serde(default)]
    pub filter_vendor_id: Option<u16>,
    #[serde(default)]
    pub filter_product_id: Option<u16>,
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
}

#[derive(Debug, Deserialize)]
pub struct Behavior {
    #[serde(default)]
    pub dpad_to_dpad: bool,
    #[serde(default = "default_true")]
    pub dpad_to_hat0: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ButtonMapping {
    Code(StringOrU16),
    Advanced {
        key: StringOrU16,
        #[serde(default)]
        dpad: bool,
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AxisMapping {
    Code(StringOrU16),
    Advanced {
        key: StringOrU16,
        #[serde(default)]
        numerator: Option<i32>,
        #[serde(default)]
        denominator: Option<i32>,
        #[serde(default)]
        invert: bool,
        #[serde(default)]
        min: Option<i32>,
        #[serde(default)]
        max: Option<i32>,
        #[serde(default)]
        fuzz: Option<i32>,
        #[serde(default)]
        flat: Option<i32>,
        #[serde(default)]
        res: Option<i32>,
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StringOrU16 {
    String(String),
    U16(u16),
}

fn default_true() -> bool {
    true
}
