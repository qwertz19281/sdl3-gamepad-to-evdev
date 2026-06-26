use anyhow::{Context, bail};
use sdl3::gamepad::Button;

use crate::Args;
use crate::config::Config;
use crate::parsed_config::ParsedConfig;

pub struct CalibrationState {
    pub states: Vec<AxisGroup>,
}

//#[derive(Default)]
pub struct AxisGroup {
    pub current: [i16;2],
    pub dirty: bool,
    pub name: String,
}

impl CalibrationState {
    pub fn create(_cfg: &Config, pcfg: &ParsedConfig, app_args: &Args) -> Self {
        let mut states = Vec::with_capacity(pcfg.stickgroup.len());

        for g in &pcfg.stickgroup {
            let ag = AxisGroup {
                current: [0;2],
                dirty: false,
                name: g.name.clone(),
            };

            states.push(ag);
        }

        Self {
            states,
        }
    }

    pub fn set_axis(&mut self, value: i16, group_idx: usize, dim: bool) {
        if let Some(g) = self.states.get_mut(group_idx) {
            g.current[dim as usize] = value;
            g.dirty = true;
        }
    }

    pub fn submit(&mut self) {
        for g in &mut self.states {
            if g.dirty {

            }
        }
    }
}

impl AxisGroup {
    
}
