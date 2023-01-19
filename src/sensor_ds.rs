use std::fmt::Debug;
use std::fmt::Write;

use one_wire_bus::Address;
use one_wire_bus::OneWire;
use one_wire_bus::OneWireResult;

use ds18b20::Ds18b20;
use ds18b20::Resolution;

use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::blocking::delay::DelayUs;

use embedded_hal::digital::v2::InputPin;
use embedded_hal::digital::v2::OutputPin;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::systime::EspSystemTime;

use crate::EventLoopMessage;

#[allow(unused_imports)]
use log::error;
#[allow(unused_imports)]
use log::info;
#[allow(unused_imports)]
use log::warn;

#[allow(dead_code)]
pub struct Sensor<'a, D, P> {
    pub pin: i32,
    pub delay: &'a mut D,
    pub sysloop: EspSystemEventLoop,
    pub one_wire_bus: &'a mut OneWire<P>,
    pub count: u32,
}

impl<D, P> Sensor<'_, D, P> {
    // P is PinDriver
    //
    #[allow(dead_code)]
    pub fn find_devices<E>(&mut self, alarm: bool) -> Result<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        let devices = self.one_wire_bus.devices(alarm, self.delay);

        for device in devices {
            match device {
                Ok(address) => {
                    self.sysloop
                        .post(
                            &EventLoopMessage::new(
                                EspSystemTime {}.now(),
                                &format!(
                                    "device ROM {address:X?}",
                                    //&format!("device ROM {address:X?} {:?}",
                                    /*
                                    self
                                    .one_wire_bus
                                    .into_inner()
                                    .pin()
                                    ,
                                    */
                                ),
                            ),
                            None,
                        )
                        .unwrap(); //? // !!! LEARN TO COMBINE ERROR's
                }
                Err(e) => {
                    error!("error device address: {e:?}");
                }
            }
        }

        Ok(())
    }
}

// search for devices and just print
// P is PinDriver
//
#[allow(dead_code)]
pub fn find_devices<P, D, E, W>(
    delay: &mut D,
    one_wire_bus: &mut OneWire<P>,
    alarm: bool,
    tx: &mut W,
) -> Result<(), std::fmt::Error>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    D: DelayUs<u16> + DelayMs<u16>,
    E: Debug,
    W: Write,
{
    writeln!(tx, "find_devices / {alarm}")?;

    /*
    pub fn devices<'a, 'b, D>(
     &'a mut self,
     only_alarming: bool,
     delay: &'b mut D
    ) -> DeviceSearch<'a, 'b, T, D>

     DeviceSearch {
      onewire: self,
      delay,
      state: None,
      finished: false,
      only_alarming,
    }

    pub struct DeviceSearch<'a, 'b, T, D> {
     onewire: &'a mut OneWire<T>,
     delay: &'b mut D,
     state: Option<SearchState>,
     finished: bool,
     only_alarming: bool,
    }

    impl<'a, 'b, T, E, D> Iterator for DeviceSearch<'a, 'b, T, D>
     where
      T: InputPin<Error = E>,
      T: OutputPin<Error = E>,
      D: DelayUs<u16>,

    type Item = OneWireResult<Address, E>;

    fn next(&mut self) -> Option<Self::Item> {
     if self.finished {
      return None;
     }

     ...Some(Ok(address))
    */

    let devices = one_wire_bus.devices(alarm, delay);

    for device in devices {
        match device {
            Ok(address) => {
                writeln!(
                    tx,
                    "device at address {:X?} with family code: {:#x?}",
                    address,
                    address.family_code(),
                )?;
            }
            Err(e) => {
                error!("error device address: {e:?}");
            }
        }
    }

    Ok(())
}

// measure all devices on bus in one go
//
// if search for True alarm and there is no device response -> UnexpectedResponse
//
#[allow(dead_code)]
pub fn get_temperature<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    one_wire_bus: &mut OneWire<P>,
    alarm: bool,
) -> OneWireResult<(), E>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: Debug,
{
    info!("get temperature via all / {alarm}");

    ds18b20::start_simultaneous_temp_measurement(one_wire_bus, delay)?;

    Resolution::Bits12.delay_for_measurement_time(delay);

    let mut search_state = None;

    loop {
        //
        //// OneWireResult<Option<(Address, SearchState)>, E>

        let devices = one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;

        if let Some((device_address, state)) = devices {
            search_state = Some(state);

            if device_address.family_code() != ds18b20::FAMILY_CODE {
                // skip other devices
                continue;
            }

            let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
            let sensor_data = sensor.read_data(one_wire_bus, delay)?;

            info!(
                "device {:?} with {}°C -> {:0>16} / {:?} / {:0>16}",
                device_address,
                sensor_data.temperature,
                format!("{:b}", sensor_data.raw_temp),
                sensor_data.scratchpad,
                format!(
                    "{:b}",
                    u16::from_le_bytes([sensor_data.scratchpad[0], sensor_data.scratchpad[1],]),
                ),
            );
        } else {
            error!("search state is None so we break");

            break;
        }
    }

    Ok(())
}

// measure all devices on bus one by one
//
#[allow(dead_code)]
pub fn get_temperature_by_one<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    one_wire_bus: &mut OneWire<P>,
    alarm: bool,
) -> OneWireResult<(), E>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: Debug,
{
    info!("get temperature by single / {alarm}");

    let mut search_state = None;

    loop {
        //
        //// OneWireResult<Option<(Address, SearchState)>, E>
        let devices = one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;

        if let Some((device_address, state)) = devices {
            search_state = Some(state);

            if device_address.family_code() != ds18b20::FAMILY_CODE {
                // skip other devices
                continue;
            }

            let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;

            sensor.start_temp_measurement(one_wire_bus, delay)?;

            Resolution::Bits12.delay_for_measurement_time(delay);

            info!("ROM: {device_address:?}");

            let sensor_data = sensor.read_data(one_wire_bus, delay)?;

            info!("RAW result: {:?}", sensor_data.raw_temp);
            info!(
                "BIN result: {:0>8} / {:0>8} -> '{:0>16}' --> {}°C",
                format!("{:b}", sensor_data.scratchpad[0]),
                format!("{:b}", sensor_data.scratchpad[1]),
                format!("{:b}", sensor_data.raw_temp),
                sensor_data.temperature,
            );

            esp_idf_hal::delay::FreeRtos {}.delay_ms(100u16);
        } else {
            error!("search state is None so we break");

            break;
        }
    }

    Ok(())
}

// OBSOLETE
// raw binary from scratchpad
#[allow(dead_code)]
fn read_raw_data<T, E>(
    address: &Address,
    onewire: &mut OneWire<T>,
    delay: &mut impl DelayUs<u16>,
) -> OneWireResult<[u8; 9], E>
where
    T: InputPin<Error = E>,
    T: OutputPin<Error = E>,
{
    let scratchpad = ds18b20::read_scratchpad(address, onewire, delay)?;

    Ok(scratchpad)
}

// OBSOLETE
#[allow(dead_code)]
fn collect_raw(scratchpad: [u8; 9]) {
    let _raw_temp = u16::from_le_bytes([scratchpad[0], scratchpad[1]]);
}

// search for devices and return list
//
#[allow(dead_code)]
pub fn list_devices<P, D, E>(delay: &mut D, one_wire_bus: &mut OneWire<P>) -> Option<Vec<Address>>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    D: DelayUs<u16> + DelayMs<u16>,
    E: Debug,
{
    let mut address_list = Vec::new();

    let devices = one_wire_bus.devices(false, delay);

    devices.into_iter().for_each(|device| match device {
        Ok(address) => {
            address_list.push(address);
        }
        Err(e) => {
            error!("error device address: {e:?}");
        }
    });

    if address_list.is_empty() {
        None
    } else {
        Some(address_list)
    }
}

// view config for given address
//
#[allow(dead_code)]
pub fn view_config<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    one_wire_bus: &mut OneWire<P>,
    device_address: Address,
    update_measurement: bool,
    sysloop: EspSystemEventLoop,
) -> OneWireResult<(), E>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: Debug,
{
    let device = Ds18b20::new(device_address)?;

    // in case we want also start new measurement
    if update_measurement.eq(&true) {
        device.start_temp_measurement(one_wire_bus, delay)?;
        Resolution::Bits12.delay_for_measurement_time(delay);
    }

    let device_data = device.read_data(one_wire_bus, delay)?;

    sysloop
        .post(
            &EventLoopMessage::new(
                EspSystemTime {}.now(),
                &format!(
                    "device data -> ROM: {device_address:?} TH: {} TL:{} resolution: {:?} {:?}",
                    device_data.alarm_temp_high,
                    device_data.alarm_temp_low,
                    device_data.resolution,
                    if update_measurement.eq(&true) {
                        Some(device_data.temperature)
                    } else {
                        None
                    },
                ),
            ),
            None,
        )
        .unwrap(); //? // !!! LEARN TO COMBINE ERROR's

    Ok(())
}

// set config for given address
//
#[allow(dead_code)]
pub fn set_config<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    one_wire_bus: &mut OneWire<P>,
    device_address: Address,
    th: i8,
    tl: i8,
    resolution: Option<Resolution>,
) -> OneWireResult<(), E>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: Debug,
{
    info!("set_config: {device_address:?} TH:{th} TL:{th} resolution:{resolution:?}");

    let device = Ds18b20::new(device_address)?;

    // read the initial config values (read from EEPROM by the device when it was first powered)
    let initial_data = device.read_data(one_wire_bus, delay)?;
    info!("initial data: {initial_data:?}");

    let resolution = match resolution {
        Some(r) => r,
        None => initial_data.resolution,
    };

    // set new alarm values and change resolution if Some
    device.set_config(
        tl,         // TL -55
        th,         // TH +125
        resolution, // RESOLUTION
        one_wire_bus,
        delay,
    )?;

    // confirm the new config is now in the scratchpad memory
    let new_data = device.read_data(one_wire_bus, delay)?;
    info!("new data: {:?}", new_data);

    // save the config to EEPROM to save it permanently
    device.save_to_eeprom(one_wire_bus, delay)?;

    // read the values from EEPROM back to the scratchpad to verify it was saved correctly
    device.recall_from_eeprom(one_wire_bus, delay)?;
    let eeprom_data = device.read_data(one_wire_bus, delay)?;

    info!("EEPROM data: {:?}", eeprom_data);

    Ok(())
}

// measure temperature for given address
//
#[allow(dead_code)]
pub fn get_device_temperature<P, E>(
    delay: &mut (impl DelayUs<u16> + DelayMs<u16>),
    one_wire_bus: &mut OneWire<P>,
    alarm: bool,
    device_address: Address,
) -> OneWireResult<(), E>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: Debug,
{
    info!("get device temperature  / {alarm}");
    info!("ROM: {device_address:?} to measure");

    let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;

    sensor.start_temp_measurement(one_wire_bus, delay)?;

    Resolution::Bits12.delay_for_measurement_time(delay);

    let sensor_data = sensor.read_data(one_wire_bus, delay)?;

    info!("RAW result: {:?}", sensor_data.raw_temp);
    info!(
        "BIN result: {:0>8} / {:0>8} -> '{:0>16}' --> {}°C",
        format!("{:b}", sensor_data.scratchpad[0]),
        format!("{:b}", sensor_data.scratchpad[1]),
        format!("{:b}", sensor_data.raw_temp),
        sensor_data.temperature,
    );

    Ok(())
}
