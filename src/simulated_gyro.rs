use std::io;

use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, InputId, MiscCode, PropType, UinputAbsSetup};
use evdev::uinput::VirtualDevice;

use crate::config::{Config, SimulateGamepadGyro};
use crate::parsed_config::ParsedConfig;

pub struct SimulatedGamepadGyro {
    pub dev: VirtualDevice,
    pub queue: Vec<InputEvent>,
    pub accel_info: AbsInfo,
    pub gyro_info: AbsInfo,
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
    pub fn create(cfg: &Config, gicfg: &SimulateGamepadGyro, parsed: &ParsedConfig) -> anyhow::Result<SimulatedGamepadGyro> {
        let [amin,amax] = gicfg.accel_out_range.unwrap_or([-32768,32768]);
        let afuzz = gicfg.accel_fuzz.unwrap_or(16);
        let aflat = gicfg.accel_flat.unwrap_or(0);
        let ares = gicfg.accel_res.unwrap_or(8092);

        let [gmin,gmax] = gicfg.gyro_out_range.unwrap_or([-2097152,2097152]);
        let gfuzz = gicfg.gyro_fuzz.unwrap_or(16);
        let gflat = gicfg.gyro_flat.unwrap_or(0);
        let gres = gicfg.gyro_flat.unwrap_or(1024);
        
        let accel_info = AbsInfo::new(0, amin, amax, afuzz, aflat, ares);
        let gyro_info = AbsInfo::new(0, gmin, gmax, gfuzz, gflat, gres);

        let abs_x_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_X,
            accel_info,
        );
        let abs_y_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_Y,
            accel_info,
        );
        let abs_z_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_Z,
            accel_info,
        );

        let abs_rx_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RX,
            gyro_info,
        );
        let abs_ry_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RY,
            gyro_info,
        );
        let abs_rz_setup = UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_RZ,
            gyro_info,
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
            accel_info,
            gyro_info,
        })
    }
}
