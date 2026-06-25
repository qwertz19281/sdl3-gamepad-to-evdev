use std::{array, io};

use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, InputId, MiscCode, PropType, UinputAbsSetup};
use evdev::uinput::VirtualDevice;

use crate::config::{Config, SimulateGamepadGyro};
use crate::parsed_config::ParsedConfig;

pub struct SimulatedGamepadGyro {
    pub dev: VirtualDevice,
    pub queue: Vec<InputEvent>,
}

impl SimulatedGamepadGyro {
    pub fn submit(&mut self) -> io::Result<()> {
        if self.queue.is_empty() {return Ok(());}
        let result = self.dev.emit(&self.queue);
        self.queue.clear();
        result
    }

    pub fn close(v: &mut Option<Self>) -> io::Result<()> {
        if let Some(v) = v {
            v.submit()?;
        }
        Ok(())
    }
}

impl SimulatedGamepadGyro {
    pub fn create(_cfg: &Config, gicfg: &SimulateGamepadGyro, parsed: &ParsedConfig, parsed_gyro: &ParsedGyroConfig) -> anyhow::Result<SimulatedGamepadGyro> {
        let abs_x_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_X,
            parsed_gyro.accel_info,
        );
        let abs_y_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_Y,
            parsed_gyro.accel_info,
        );
        let abs_z_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_Z,
            parsed_gyro.accel_info,
        );

        let abs_rx_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RX,
            parsed_gyro.gyro_info,
        );
        let abs_ry_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RY,
            parsed_gyro.gyro_info,
        );
        let abs_rz_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RZ,
            parsed_gyro.gyro_info,
        );

        let mut misc = AttributeSet::new();
        // ds4 timestamp is in microseconds
        misc.insert(MiscCode::MSC_TIMESTAMP);

        let mut props = AttributeSet::new();
        props.insert(PropType::ACCELEROMETER);

        let builder = VirtualDevice::builder()?
            .name(&gicfg.name)
            .input_id(InputId::new(
                parsed.evdev_bus_type,
                gicfg.vendor_id,
                gicfg.product_id,
                gicfg.version,
            ))
            .with_absolute_axis(&abs_x_setup)?
            .with_absolute_axis(&abs_y_setup)?
            .with_absolute_axis(&abs_z_setup)?
            .with_absolute_axis(&abs_rx_setup)?
            .with_absolute_axis(&abs_ry_setup)?
            .with_absolute_axis(&abs_rz_setup)?
            .with_msc(&misc)?
            .with_properties(&props)?;

        let dev = builder.build()?;

        Ok(SimulatedGamepadGyro {
            dev,
            queue: Vec::with_capacity(8),
        })
    }
}

#[derive(Debug)]
pub struct ParsedGyroConfig {
    pub accel_info: AbsInfo,
    pub gyro_info: AbsInfo,
    pub accel_mul: [f32; 3],
    pub gyro_mul: [f32; 3],
}

impl ParsedGyroConfig {
    pub fn parse(gicfg: &SimulateGamepadGyro) -> anyhow::Result<Self> {
        let [amin,amax] = gicfg.accel_out_range.unwrap_or([-32768,32768]);
        let afuzz = gicfg.accel_fuzz.unwrap_or(16);
        let aflat = gicfg.accel_flat.unwrap_or(0);
        let ares = gicfg.accel_res.unwrap_or(8192);

        let [gmin,gmax] = gicfg.gyro_out_range.unwrap_or([-2097152,2097152]);
        let gfuzz = gicfg.gyro_fuzz.unwrap_or(16);
        let gflat = gicfg.gyro_flat.unwrap_or(0);
        let gres = gicfg.gyro_flat.unwrap_or(1024);
        
        let accel_info = AbsInfo::new(0, amin, amax, afuzz, aflat, ares);
        let gyro_info = AbsInfo::new(0, gmin, gmax, gfuzz, gflat, gres);

        let calc_accel_mul = |dim: usize| -> f32 {
            gicfg.accel_mul_raw.map(|v| v[dim] ).unwrap_or_else(|| {
                let accel_mul = gicfg.accel_mul.map_or(1., |v| v[dim] );
                (ares as f64 * accel_mul / 9.80665) as f32
            })
        };
        let calc_gyro_mul = |dim: usize| -> f32 {
            gicfg.gyro_mul_raw.map(|v| v[dim] ).unwrap_or_else(|| {
                let gyro_mul = gicfg.gyro_mul.map_or(1., |v| v[dim] );
                (gres as f64 * gyro_mul * 180. / std::f64::consts::PI) as f32
            })
        };

        let accel_mul: [f32; 3] = array::from_fn(calc_accel_mul);
        let gyro_mul: [f32; 3] = array::from_fn(calc_gyro_mul);

        Ok(Self {
            accel_info,
            gyro_info,
            accel_mul,
            gyro_mul,
        })
    }
}
