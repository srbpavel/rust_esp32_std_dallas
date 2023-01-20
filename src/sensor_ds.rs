use crate::eventloop;
use crate::EventLoopMessage;

use std::fmt;
use std::fmt::Debug;
//use std::fmt::Write;

use one_wire_bus::Address;
use one_wire_bus::OneWire;
use one_wire_bus::OneWireResult;

use ds18b20::Ds18b20;
use ds18b20::Resolution;
use ds18b20::SensorData;

use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::blocking::delay::DelayUs;
use embedded_hal::digital::v2::InputPin;
use embedded_hal::digital::v2::OutputPin;

use esp_idf_hal::delay::FreeRtos;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::systime::EspSystemTime;

#[allow(unused_imports)]
use log::error;
#[allow(unused_imports)]
use log::info;
#[allow(unused_imports)]
use log::warn;

const WTD_FEEDER_DURATION: u16 = 10; // test more and study watchdog !!!

#[derive(Debug)]
pub enum Route {
    OneShot,
    ByOne,
    Device(Address),
}

pub struct Measurement {
    pub pin: i32,
    //pub address: u64,
    pub address: Address,
    pub temperature: f32,
    pub raw_temp: u16,
    pub resolution: Resolution,
}

impl Measurement {
    fn new(pin: i32, address: Address, sensor: SensorData) -> Self {
        Self {
            pin,
            //address: address.0,
            address,
            temperature: sensor.temperature,
            raw_temp: sensor.raw_temp,
            resolution: sensor.resolution,
        }
    }
}

impl fmt::Display for Measurement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}], ROM {:?} with {}°C -> {:0>16} / {:?}",
            self.pin,
            self.address,
            self.temperature,
            format!("{:b}", self.raw_temp),
            self.resolution,
        )
    }
}

pub struct Sensor<'a, P> {
    pub pin: i32,
    pub sysloop: EspSystemEventLoop,
    pub one_wire_bus: &'a mut OneWire<P>,
}

impl<P> Sensor<'_, P> {
    pub fn measure<D, E>(
        &mut self,
        delay: &mut D,
        alarm: bool,
        route: Route,
    ) -> OneWireResult<Vec<Measurement>, E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        info!("measure: {route:?} / {alarm}");

        let mut result: Vec<Measurement> = vec![];

        match route {
            Route::OneShot => {
                ds18b20::start_simultaneous_temp_measurement(self.one_wire_bus, delay)?;

                Resolution::Bits12.delay_for_measurement_time(delay);

                let mut search_state = None;

                loop {
                    //// OneWireResult<Option<(Address, SearchState)>, E>
                    let devices =
                        self.one_wire_bus
                            .device_search(search_state.as_ref(), alarm, delay)?;

                    if let Some((device_address, state)) = devices {
                        search_state = Some(state);

                        if device_address.family_code() != ds18b20::FAMILY_CODE {
                            // skip other devices
                            continue;
                        }

                        let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                        let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                        result.push(Measurement::new(self.pin, device_address, sensor_data));
                    } else {
                        break;
                    }
                }
            }
            Route::ByOne => {
                let mut search_state = None;

                loop {
                    let devices =
                        self.one_wire_bus
                            .device_search(search_state.as_ref(), alarm, delay)?;

                    if let Some((device_address, state)) = devices {
                        search_state = Some(state);

                        if device_address.family_code() != ds18b20::FAMILY_CODE {
                            continue;
                        }

                        let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;

                        sensor.start_temp_measurement(self.one_wire_bus, delay)?;

                        Resolution::Bits12.delay_for_measurement_time(delay);

                        let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                        result.push(Measurement::new(self.pin, device_address, sensor_data));

                        FreeRtos {}.delay_ms(WTD_FEEDER_DURATION);
                    } else {
                        break;
                    }
                }
            }
            Route::Device(device_address) => {
                let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;

                sensor.start_temp_measurement(self.one_wire_bus, delay)?;

                Resolution::Bits12.delay_for_measurement_time(delay);

                let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                result.push(Measurement::new(self.pin, device_address, sensor_data));
            }
        }

        Ok(result)
    }

    /*
    // OBSOLETE -> TO DEL
    // P is PinDriver
    //
    pub fn find_devices<E, D, W>(
        &mut self,
        alarm: bool,
        delay: &mut D,
        tx: &mut W,
    ) -> Result<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
        W: Write,
    {
        writeln!(tx, "find_devices / {alarm}").unwrap();

        let devices = self.one_wire_bus.devices(alarm, delay);

        for device in devices {
            match device {
                Ok(address) => {
                    writeln!(
                        tx,
                        "device at address {:X?} with family code: {:#x?}",
                        address,
                        address.family_code(),
                    ).unwrap();

                    self.sysloop
                        .post(
                            &EventLoopMessage::new(
                                EspSystemTime {}.now(),
                                &format!(
                                    "device ROM {address:X?} / pin: {}",
                                    self.pin,
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
    */

    // search for devices and return list
    //
    pub fn list_devices<D, E>(&mut self, delay: &mut D) -> Option<Vec<Address>>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        let mut address_list = Vec::new();

        let devices = self.one_wire_bus.devices(false, delay);

        devices.into_iter().for_each(|device| match device {
            Ok(address) => {
                self.sysloop
                    .post(
                        &EventLoopMessage::new(
                            EspSystemTime {}.now(),
                            &format!("device ROM {address:X?} / pin: {}", self.pin),
                        ),
                        None,
                    )
                    .unwrap(); //? // !!! LEARN TO COMBINE ERROR's

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
    pub fn view_config<D, E>(
        &mut self,
        delay: &mut D,
        device_address: Address,
        update_measurement: bool,
        sysloop: EspSystemEventLoop,
    ) -> OneWireResult<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        let device = Ds18b20::new(device_address)?;

        // in case we want also start new measurement
        if update_measurement.eq(&true) {
            device.start_temp_measurement(self.one_wire_bus, delay)?;
            Resolution::Bits12.delay_for_measurement_time(delay);
        }

        let device_data = device.read_data(self.one_wire_bus, delay)?;

        if let Err(e) = eventloop::post(
            sysloop,
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
        ) {
            error!("ERROR eventloop msg: {e}");
        }

        Ok(())
    }

    // set config for given address
    //
    #[allow(dead_code)]
    pub fn set_config<D, E>(
        &mut self,
        delay: &mut D,
        device_address: Address,
        th: i8,
        tl: i8,
        resolution: Option<Resolution>,
    ) -> OneWireResult<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        info!("set_config: {device_address:?} TH:{th} TL:{th} resolution:{resolution:?}");

        let device = Ds18b20::new(device_address)?;

        // read the initial config values (read from EEPROM by the device when it was first powered)
        let initial_data = device.read_data(self.one_wire_bus, delay)?;
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
            self.one_wire_bus,
            delay,
        )?;

        // confirm the new config is now in the scratchpad memory
        let new_data = device.read_data(self.one_wire_bus, delay)?;
        info!("new data: {:?}", new_data);

        // save the config to EEPROM to save it permanently
        device.save_to_eeprom(self.one_wire_bus, delay)?;

        // read the values from EEPROM back to the scratchpad to verify it was saved correctly
        device.recall_from_eeprom(self.one_wire_bus, delay)?;
        let eeprom_data = device.read_data(self.one_wire_bus, delay)?;

        info!("EEPROM data: {:?}", eeprom_data);

        Ok(())
    }
}
