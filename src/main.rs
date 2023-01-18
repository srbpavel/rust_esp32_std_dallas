mod eventloop;
mod sensor_ds;

use eventloop::EventLoopMessage;

use esp_idf_svc::eventloop::EspSystemEventLoop;

use esp_idf_sys as _;
use esp_idf_sys::EspError;

use esp_idf_hal::delay::Ets;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::PinDriver;

use embedded_hal::blocking::delay::DelayMs;

use esp_idf_svc::log::EspLogger;
use esp_idf_svc::systime::EspSystemTime;

use one_wire_bus::OneWire;

#[allow(unused_imports)]
use log::error;
#[allow(unused_imports)]
use log::info;
#[allow(unused_imports)]
use log::warn;

const SLEEP_DURATION: u16 = 60 * 1000;
const WTD_FEEDER_DURATION: u16 = 100;
//const SINGLE_ROM: u64 = 3891110133793834024;
const SINGLE_ROM: u64 = 0x3600000CFAB44428;

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
    let pin_i = peripherals.pins.gpio6;
    let pin_ii = peripherals.pins.gpio4;

    let pin_driver_i = PinDriver::input_output_od(pin_i)?;
    let pin_driver_ii = PinDriver::input_output_od(pin_ii)?;

    let pin_i_number = pin_driver_i.pin();
    let pin_ii_number = pin_driver_ii.pin();

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

    // TX // just testing instead channel/eventloop/...
    let mut tx_buffer = String::new();

    // FIND DEVICE
    if let Err(search_error) =
        sensor_ds::find_devices(&mut delay, &mut one_wire_bus_i, false, &mut tx_buffer)
    {
        error!("Error device_search: {search_error:?}");
    };

    warn!("TX BUFFER: {tx_buffer}");

    // <FREEZER> set with alarm -30 <-> 0
    let rom_to_change = one_wire_bus::Address(SINGLE_ROM);

    // LIST
    let device_list_i = sensor_ds::list_devices(&mut delay, &mut one_wire_bus_i);
    let device_list_ii = sensor_ds::list_devices(&mut delay, &mut one_wire_bus_ii);

    // VIEW
    if let Some(list) = device_list_i {
        list.iter().for_each(|device| {
            if let Err(e) = sensor_ds::view_config(
                &mut delay,
                &mut one_wire_bus_i,
                *device,
                true,
                sysloop.clone(),
            ) {
                error!("view_config: {e:?}");
            }
        });
    }

    if let Some(list) = device_list_ii {
        list.iter().for_each(|device| {
            // VIEW CONFIG for ALL in LIST
            if let Err(e) = sensor_ds::view_config(
                &mut delay,
                &mut one_wire_bus_ii,
                *device,
                false,
                sysloop.clone(),
            ) {
                error!("view_config: {e:?}");
            }
            /* // SET CONFIG for ALL in LIST
            if let Err(e) = sensor_ds::set_config(
                &mut delay,
                &mut one_wire_bus_ii,
                *device, // ROM
                55, // TH
                0, // TL
                None, // resolution will not change
                // Some(Resolution::Bits12), // resoulution
            ) {
                println!(" ERROR setting config: {:?}", e);
            }
            */
        });

        // /* // <FREEZER> set alarm limits
        if list.contains(&rom_to_change) {
            info!("ROM {rom_to_change:?} available we can set_config");

            /* // CONFIG CHANGE
            if let Err(e) = sensor_ds::set_config(&mut delay,
                                                  &mut one_wire_bus_ii,
                                                  rom_to_change, // ROM
                                                  0, // TH
                                                  -30, // TL
                                                  None, // resolution
            ) {
                error!(" ERROR setting config: {:?}", e);
            }
            */
        } else {
            error!("ROM {rom_to_change:?} not found {list:?}");
        }
        // */
    }

    sleep.delay_ms(WTD_FEEDER_DURATION);

    // LOOP
    loop {
        info!("DS_1: {:?}", pin_i_number);

        // /* // via SINGLE
        if let Err(e) = sensor_ds::get_temperature_by_one(&mut delay, &mut one_wire_bus_i, false) {
            error!("TEMPERATURE SINGLE result: {e:?}");
        };
        // */
        // /* // ALL in one go
        if let Err(e) = sensor_ds::get_temperature(&mut delay, &mut one_wire_bus_i, false) {
            error!("TEMPERATURE ALL result: {e:?}");
        };
        // */
        // this feed's watchdog so next measurement does not trigger it
        sleep.delay_ms(WTD_FEEDER_DURATION);

        info!("DS_2: {:?}", pin_ii_number);
        /* // via SINGLE
        let temperature_result = sensor_ds::get_temperature_by_one(
            &mut delay,
            &mut one_wire_bus_ii,
            false,
        );
        println!("   TEMPERATURE SINGLE result: {temperature_result:?}");

        sleep.delay_ms(WTD_FEEDER_DURATION);
        */

        // /* // ALL in one go
        if let Err(e) = sensor_ds::get_temperature(&mut delay, &mut one_wire_bus_ii, false) {
            error!("TEMPERATURE ALL result Error: {e:?}");
        };

        // /* // ALL in one go only with ALARM
        if let Err(e) = sensor_ds::get_temperature(&mut delay, &mut one_wire_bus_ii, true) {
            error!("TEMPERATURE ALL result Error: {e:?}");
        };
        // */
        // <FREEZER> READ SINGLE DEVICE
        if let Err(e) = sensor_ds::view_config(
            &mut delay,
            &mut one_wire_bus_ii,
            rom_to_change,
            false,
            sysloop.clone(),
        ) {
            error!("ERROR {rom_to_change:X?} view_config: {e:?}");
        }

        if let Err(e) = sensor_ds::get_device_temperature(
            &mut delay,
            &mut one_wire_bus_ii,
            false,
            rom_to_change,
        ) {
            error!("ERROR {rom_to_change:X?} get_device_temperature: {e:?}");
        }
        //_

        sleep.delay_ms(SLEEP_DURATION);
    }
}
