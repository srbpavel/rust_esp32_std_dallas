mod eventloop;
mod sensor_ds;

use sensor_ds::Route;
use sensor_ds::Sensor;

use eventloop::EventLoopMessage;

use one_wire_bus::OneWire;

use ds18b20::Resolution;

use embedded_hal::blocking::delay::DelayMs;

use esp_idf_svc::eventloop::EspSystemEventLoop;

use esp_idf_sys as _;
use esp_idf_sys::EspError;

use esp_idf_hal::delay::Ets;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::IOPin;
use esp_idf_hal::gpio::PinDriver;

use esp_idf_svc::log::EspLogger;
use esp_idf_svc::systime::EspSystemTime;

#[allow(unused_imports)]
use log::error;
#[allow(unused_imports)]
use log::info;
#[allow(unused_imports)]
use log::warn;

const SLEEP_DURATION: u16 = 60 * 1000;
const WTD_FEEDER_DURATION: u16 = 100;

const ALARM_CHANGE: bool = false;
const TH: i8 = 55;
const TL: i8 = 0;
const RESOLUTION: Option<Resolution> = None;
//const RESOLUTION: Option<Resolution> = Some(Resolution::Bits12);

const SINGLE_ALARM_CHANGE: bool = false;
const SINGLE_ROM: u64 = 0x3600000CFAB44428;
const SINGLE_TH: i8 = 0;
const SINGLE_TL: i8 = -30;
const SINGLE_RESOLUTION: Option<Resolution> = None;
//const SINGLE_RESOLUTION: Option<Resolution> = Some(Resolution::Bits12);

//
fn main() -> Result<(), EspError> {
    esp_idf_sys::link_patches();
    EspLogger::initialize_default();

    let machine_boot = EspSystemTime {}.now();
    warn!("duration since machine_boot: {machine_boot:?}");

    let sysloop = EspSystemEventLoop::take()?;
    warn!("event_loop init");
    let _subscription = sysloop.subscribe(move |msg: &EventLoopMessage| {
        info!("[{}] {}", msg.duration.as_secs(), msg.data.trim());
    })?;

    let mut sleep = FreeRtos {};
    let mut delay = Ets {};

    let peripherals = esp_idf_hal::peripherals::Peripherals::take().unwrap();
    let pin_i = peripherals.pins.gpio6.downgrade();
    let pin_ii = peripherals.pins.gpio4.downgrade();
    // no devices here, just to verify error handling
    let pin_iii = peripherals.pins.gpio2.downgrade();

    let pin_driver_i = PinDriver::input_output_od(pin_i)?;
    let pin_driver_ii = PinDriver::input_output_od(pin_ii)?;
    let pin_driver_iii = PinDriver::input_output_od(pin_iii)?;

    let pin_i_number = pin_driver_i.pin();
    let pin_ii_number = pin_driver_ii.pin();
    let pin_iii_number = pin_driver_iii.pin();

    // BUS
    //
    //// impl<T, E> OneWire<T>
    ////  where
    ////   T: InputPin<Error = E>,
    ////   T: OutputPin<Error = E>,
    ////
    //// pub fn new(pin: T) -> OneWireResult<OneWire<T>, E>
    ////
    //// type OneWireResult<T, E> = Result<T, OneWireError<E>>;
    let mut one_wire_bus_i = OneWire::new(pin_driver_i).unwrap();
    let mut one_wire_bus_ii = OneWire::new(pin_driver_ii).unwrap();
    let mut one_wire_bus_iii = OneWire::new(pin_driver_iii).unwrap();

    /* TO DEL
    // TX // just testing instead channel/eventloop/...
    //let mut tx_buffer = String::new();
     */

    // SENSOR
    let mut sensor_i = Sensor {
        pin: pin_i_number,
        sysloop: sysloop.clone(),
        one_wire_bus: &mut one_wire_bus_i,
    };

    let mut sensor_ii = Sensor {
        pin: pin_ii_number,
        sysloop: sysloop.clone(),
        one_wire_bus: &mut one_wire_bus_ii,
    };

    let mut sensor_iii = Sensor {
        pin: pin_iii_number,
        sysloop: sysloop.clone(),
        one_wire_bus: &mut one_wire_bus_iii,
    };

    // <FREEZER> set with negative
    let rom_to_change = one_wire_bus::Address(SINGLE_ROM);

    // ONCE
    vec![&mut sensor_i, &mut sensor_ii, &mut sensor_iii]
        .iter_mut()
        .for_each(|sensor| {
            /* TO DEL
            if let Err(search_error) = sensor.find_devices(false, &mut delay, &mut tx_buffer) {
                error!("Error device_search: {search_error:?}");
            };
            */

            // LIST
            warn!("@list devices at pin: {}", sensor.pin);
            let device_list = sensor.list_devices(&mut delay);

            if let Some(list) = device_list {
                list.iter().for_each(|device| {
                    // VIEW CONFIG
                    warn!("@view device config");
                    if let Err(e) = sensor.view_config(&mut delay, *device, false, sysloop.clone())
                    {
                        error!("view_config <FREEZER>: {e:?}");
                    }
                    // SET CONFIG for ALL in LIST
                    if ALARM_CHANGE.eq(&true) {
                        warn!("@set new config for all devices");
                        if let Err(e) = sensor.set_config(
                            &mut delay, *device, TH, // TH 55
                            TL, // TL 0
                            RESOLUTION,
                        ) {
                            println!(" ERROR setting config: {e:?}");
                        }
                    }

                    // <FREEZER> set alarm limits
                    warn!("@verify if device is the one to change config on");
                    if device.eq(&rom_to_change) {
                        warn!("@set new config for single device");

                        // CONFIG CHANGE
                        if SINGLE_ALARM_CHANGE.eq(&true) {
                            if let Err(e) = sensor.set_config(
                                &mut delay,
                                rom_to_change,
                                SINGLE_TH, // TH 0
                                SINGLE_TL, // TL -30
                                SINGLE_RESOLUTION,
                            ) {
                                error!(" ERROR setting config: {e:?}");
                            }
                        }
                    } else {
                        error!("ROM {rom_to_change:?} not found {list:?}");
                    }
                });
            };
        });

    sleep.delay_ms(WTD_FEEDER_DURATION);

    // LOOP
    let mut cycle_counter: u64 = 0;

    loop {
        cycle_counter += 1;
        warn!("### i: {cycle_counter}");

        // SENSOR
        vec![&mut sensor_i, &mut sensor_ii]
            .iter_mut()
            .for_each(|sensor| {
                // ID
                // do we want it also with each measurement?
                //warn!("SENSOR_ID: pin: {}", sensor.pin);

                // ByOne
                warn!("@measure temperature for all sensors OneByOne");
                match sensor.measure(&mut delay, false, Route::ByOne) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!("[{}] MEASURE single result: {e:?}", sensor.pin),
                }

                sleep.delay_ms(WTD_FEEDER_DURATION);

                // OneShot
                let mut alarm = false;
                warn!("@measure temperature for all sensors in OneShot + alarm {alarm}");
                match sensor.measure(&mut delay, alarm, Route::OneShot) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!("[{}] MEASURE all result: {e:?} / {alarm}", sensor.pin),
                }

                alarm = true;
                warn!("@measure temperature for all sensors in OneShot + alarm {alarm} -> will return Unexpeted if no device with alarm");
                match sensor.measure(&mut delay, alarm, Route::OneShot) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!("[{}] MEASURE all result: {e:?} / {alarm}", sensor.pin),
                }

                // <FREEZER> as SINGLE DEVICE
                // VIEW
                warn!("@view device {rom_to_change:?} config");
                if let Err(e) =
                    sensor.view_config(&mut delay, rom_to_change, false, sysloop.clone())
                {
                    error!(
                        "[{}] <FREEZER> {rom_to_change:?} view_config: {e:?}",
                        sensor.pin
                    );
                }

                // Device(Address)
                let mut alarm = false;
                warn!("@measure device {rom_to_change:?} + alarm {alarm}");
                match sensor.measure(&mut delay, alarm, Route::Device(rom_to_change)) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!(
                        "[{}] <FREEZER> {rom_to_change:?} measure: {e:?}",
                        sensor.pin
                    ),
                }

                alarm = true;
                warn!("@measure device {rom_to_change:?} + alarm {alarm}");
                match sensor.measure(&mut delay, alarm, Route::Device(rom_to_change)) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!(
                        "[{}] <FREEZER> {rom_to_change:?} measure: {e:?}",
                        sensor.pin
                    ),
                }
            });

        sleep.delay_ms(SLEEP_DURATION);
    }
}
