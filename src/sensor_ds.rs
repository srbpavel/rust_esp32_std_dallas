use crate::EventLoopMessage;
use crate::eventloop;

use std::fmt::Debug;
use std::fmt::Write;

use one_wire_bus::Address;
use one_wire_bus::OneWire;
use one_wire_bus::OneWireResult;

use ds18b20::Ds18b20;
use ds18b20::Resolution;
//use ds18b20::SensorData;

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

const WTD_FEEDER_DURATION: u16 = 10;


#[derive(Debug)]
pub enum Route {
    OneGo,
    OneByOne,
    Device(Address),
}

pub struct Measurement {
    pub address: u64,
    pub temperature: f32,
    pub raw: u16,
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
            Route::OneGo => {
                ds18b20::start_simultaneous_temp_measurement(self.one_wire_bus, delay)?;
                
                Resolution::Bits12.delay_for_measurement_time(delay);
                
                let mut search_state = None;
                
                loop {
                    //// OneWireResult<Option<(Address, SearchState)>, E>
                    let devices = self.one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;
                    
                    if let Some((device_address, state)) = devices {
                        search_state = Some(state);
                        
                        if device_address.family_code() != ds18b20::FAMILY_CODE {
                            // skip other devices
                            continue;
                        }
                        
                        let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                        let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                        // will create new struct as we need also append rom
                        result.push(Measurement {
                            address: device_address.0,
                            temperature: sensor_data.temperature,
                            raw: sensor_data.raw_temp,
                        });
                            
                        /*
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
                        */
                    } else {
                        //error!("search state is None so we break");
                        
                        break;
                    }
                }
            },
            Route::OneByOne => {
                let mut search_state = None;
        
                loop {
                    //
                    //// OneWireResult<Option<(Address, SearchState)>, E>
                    let devices = self.one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;
                    
                    if let Some((device_address, state)) = devices {
                        search_state = Some(state);
                        
                        if device_address.family_code() != ds18b20::FAMILY_CODE {
                            // skip other devices
                            continue;
                        }
                        
                        let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                        
                        sensor.start_temp_measurement(self.one_wire_bus, delay)?;
                        
                        Resolution::Bits12.delay_for_measurement_time(delay);
                
                        //info!("ROM: {device_address:?}");
                        
                        let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                        //result.push(sensor_data);
                        result.push(Measurement {
                            address: device_address.0,
                            temperature: sensor_data.temperature,
                            raw: sensor_data.raw_temp,
                        });
                        
                        /*
                        info!("RAW result: {:?}", sensor_data.raw_temp);
                        info!(
                            "BIN result: {:0>8} / {:0>8} -> '{:0>16}' --> {}°C",
                            format!("{:b}", sensor_data.scratchpad[0]),
                            format!("{:b}", sensor_data.scratchpad[1]),
                            format!("{:b}", sensor_data.raw_temp),
                            sensor_data.temperature,
                        );
                        */

                        FreeRtos {}.delay_ms(WTD_FEEDER_DURATION);
                
                    } else {
                        //error!("search state is None so we break");
                        
                        break;
                    }
                }
            },
            Route::Device(device_address) => {
                //info!("get device temperature  / {alarm}");
                //info!("ROM: {device_address:?} to measure");
                
                let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                
                sensor.start_temp_measurement(self.one_wire_bus, delay)?;
                
                Resolution::Bits12.delay_for_measurement_time(delay);
                
                let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

                //result.push(sensor_data);
                result.push(Measurement {
                    address: device_address.0,
                    temperature: sensor_data.temperature,
                    raw: sensor_data.raw_temp,
                });
                
                /*
                info!("RAW result: {:?}", sensor_data.raw_temp);
                info!(
                    "BIN result: {:0>8} / {:0>8} -> '{:0>16}' --> {}°C",
                    format!("{:b}", sensor_data.scratchpad[0]),
                    format!("{:b}", sensor_data.scratchpad[1]),
                    format!("{:b}", sensor_data.raw_temp),
                    sensor_data.temperature,
                );
                */
            },
        }

        //Ok(())
        Ok(result)
    }
    
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

    // search for devices and return list
    //
    #[allow(dead_code)]
    pub fn list_devices<D, E>(
        &mut self,
        delay: &mut D,
    ) -> Option<Vec<Address>>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        let mut address_list = Vec::new();
        
        let devices = self.one_wire_bus.devices(false, delay);
        
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
        
        /*
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
        */
        
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

    // measure all devices on bus in one go
    //
    // if search for True alarm and there is no device response -> UnexpectedResponse
    //
    //#[allow(dead_code)]
    pub fn get_temperature<D, E>(
        &mut self,
        delay: &mut D,
        alarm: bool,
    ) -> OneWireResult<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        info!("get temperature via all / {alarm}");

        ds18b20::start_simultaneous_temp_measurement(self.one_wire_bus, delay)?;

        Resolution::Bits12.delay_for_measurement_time(delay);

        let mut search_state = None;

        loop {
            //
            //// OneWireResult<Option<(Address, SearchState)>, E>

            let devices = self.one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;

            if let Some((device_address, state)) = devices {
                search_state = Some(state);

                if device_address.family_code() != ds18b20::FAMILY_CODE {
                    // skip other devices
                    continue;
                }

                let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;

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
    //#[allow(dead_code)]
    pub fn get_temperature_by_one<D, E>(
        &mut self,
        delay: &mut D,
        alarm: bool,
    ) -> OneWireResult<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        info!("get temperature by single / {alarm}");
        
        let mut search_state = None;
        
        loop {
            //
            //// OneWireResult<Option<(Address, SearchState)>, E>
            let devices = self.one_wire_bus.device_search(search_state.as_ref(), alarm, delay)?;
            
            if let Some((device_address, state)) = devices {
                search_state = Some(state);
                
                if device_address.family_code() != ds18b20::FAMILY_CODE {
                    // skip other devices
                    continue;
                }
                
                let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
                
                sensor.start_temp_measurement(self.one_wire_bus, delay)?;
                
                Resolution::Bits12.delay_for_measurement_time(delay);
                
                info!("ROM: {device_address:?}");
                
                let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;
                
                info!("RAW result: {:?}", sensor_data.raw_temp);
                info!(
                    "BIN result: {:0>8} / {:0>8} -> '{:0>16}' --> {}°C",
                    format!("{:b}", sensor_data.scratchpad[0]),
                    format!("{:b}", sensor_data.scratchpad[1]),
                    format!("{:b}", sensor_data.raw_temp),
                    sensor_data.temperature,
                );
                
                FreeRtos {}.delay_ms(WTD_FEEDER_DURATION);
                
            } else {
                error!("search state is None so we break");
                
                break;
            }
        }
        
        Ok(())
    }

    // measure temperature for given address
    //
    //#[allow(dead_code)]
    pub fn get_device_temperature<D, E>(
        &mut self,
        delay: &mut D,
        alarm: bool,
        device_address: Address,
    ) -> OneWireResult<(), E>
    where
        P: OutputPin<Error = E> + InputPin<Error = E>,
        D: DelayUs<u16> + DelayMs<u16>,
        E: Debug,
    {
        info!("get device temperature  / {alarm}");
        info!("ROM: {device_address:?} to measure");
        
        let sensor: Ds18b20 = Ds18b20::new::<E>(device_address)?;
        
        sensor.start_temp_measurement(self.one_wire_bus, delay)?;
        
        Resolution::Bits12.delay_for_measurement_time(delay);
        
        let sensor_data = sensor.read_data(self.one_wire_bus, delay)?;
        
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
}

