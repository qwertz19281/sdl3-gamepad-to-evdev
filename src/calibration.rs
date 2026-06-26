use evdev::{EventType, InputEvent};

use crate::Args;
use crate::config::Config;
use crate::parsed_config::{ParsedAxisBinding, ParsedConfig};
use crate::simulated::SimulatedGamepad;

pub struct CalibrationState<'a> {
    pub states: Vec<AxisGroup<'a>>,
}

//#[derive(Default)]
pub struct AxisGroup<'a> {
    pub current: [i16;2],
    pub prev_out: Option<[i32;2]>,
    pub axes: [&'a ParsedAxisBinding;2],
    pub dirty: bool,
    pub in_deadzone: bool,
    pub deadzone: f32,
    pub deadzone_release: f32,
    pub deadzone_bend: f32,
    pub deadzone_bendscale: f32,
    pub out_scale: [f32;2],
    pub out_off: [f32;2],
    pub process: bool,
    pub name: String,
}

impl<'a> CalibrationState<'a> {
    pub fn create(_cfg: &Config, pcfg: &'a ParsedConfig, _app_args: &Args) -> Self {
        let mut states = Vec::with_capacity(pcfg.stickgroup.len());

        for g in &pcfg.stickgroup {
            let axes = g.axes.map(|id| pcfg.axis_bindings.get(&id).unwrap() );
            let ag = AxisGroup {
                current: [0;2],
                axes,
                dirty: false,
                in_deadzone: false,
                prev_out: None,
                deadzone: g.deadzone,
                deadzone_release: g.deadzone_release,
                deadzone_bend: g.deadzone_bend,
                deadzone_bendscale: (1. - g.deadzone_bend).recip(),
                out_scale: axes.map(|s| s.out_range[1] as f32 - s.out_range[0] as f32),
                out_off: axes.map(|s|s.out_range[0] as f32 ),
                process: g.process,
                name: g.name.clone(),
            };

            states.push(ag);
        }

        Self {
            states,
        }
    }

    pub fn set_axis(&mut self, value: i16, group_idx: usize, dim: bool) -> bool {
        if let Some(g) = self.states.get_mut(group_idx) && g.process {
            g.current[dim as usize] = value;
            g.dirty = true;
            return true;
        }
        false
    }

    pub fn submit(&mut self, queue: &mut SimulatedGamepad) {
        for g in &mut self.states {
            if g.dirty {
                let [r,t] = to_polar(
                    g.current[0] as f32 / 32767.,
                    g.current[1] as f32 / 32767.,
                );


                let mut r = r.clamp(0., 1.01);
                if g.in_deadzone {
                    if r < g.deadzone_release {
                        r = 0.;
                        g.in_deadzone = false;
                    }
                } else {
                    if r < g.deadzone {
                        r = 0.
                    } else {
                        g.in_deadzone = true;
                    }
                }
                let r = 1. - ((1. - r) * g.deadzone_bendscale);
                let r = r.clamp(0., 1.01);


                let out = to_cartesian(r, t);
                let mut iout = [0,0];
                
                for i in [0,1] {
                    let out = (out[i] + 1.) * 0.5;
                    let out = out * g.out_scale[i];
                    iout[i] = (out.round() as i32).clamp(g.axes[i].clamp_out[0], g.axes[i].clamp_out[1]);
                }

                if g.prev_out != Some(iout) {
                    for i in [0,1] {
                        let evdev_event = InputEvent::new(EventType::ABSOLUTE.0, g.axes[i].setup.code(), iout[i]);
                        queue.queue.push(evdev_event);
                    }

                    g.prev_out = Some(iout);
                }
            }
        }
    }
}

impl AxisGroup<'_> {
    
}

fn to_polar(x: f32, y: f32) -> [f32;2] {
    [
        (x.powi(2) + y.powi(2)).sqrt(),
        y.atan2(x)
    ]
}

fn to_cartesian(r: f32, t: f32) -> [f32;2] {
    [
        r * t.cos(),
        r * t.sin(),
    ]
}
