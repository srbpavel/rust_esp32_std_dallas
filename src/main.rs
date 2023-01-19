mod eventloop;
mod sensor_ds;

use sensor_ds::Sensor;

use eventloop::EventLoopMessage;

use esp_idf_svc::eventloop::EspSystemEventLoop;

use esp_idf_sys as _;
use esp_idf_sys::EspError;

use esp_idf_hal::delay::Ets;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::IOPin;
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
    let pin_i = peripherals.pins.gpio6.downgrade();
    let pin_ii = peripherals.pins.gpio4.downgrade();
    //let pin_iii = peripherals.pins.gpio2.downgrade();
    //let pin_iv = peripherals.pins.gpio1.downgrade();

    let pin_driver_i = PinDriver::input_output_od(pin_i)?;
    let pin_driver_ii = PinDriver::input_output_od(pin_ii)?;
    //let pin_driver_iii = PinDriver::input_output_od(pin_iii)?;
    //let pin_driver_iv = PinDriver::input_output_od(pin_iv)?;

    let pin_i_number = pin_driver_i.pin();
    let pin_ii_number = pin_driver_ii.pin();
    //let pin_iii_number = pin_driver_iii.pin();
    //let pin_iv_number = pin_driver_iv.pin();

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
    //let mut one_wire_bus_iii = OneWire::new(pin_driver_iii).unwrap();
    //let mut one_wire_bus_iv = OneWire::new(pin_driver_iv).unwrap();

    // TX // just testing instead channel/eventloop/...
    #[allow(unused_variables)]
    #[allow(unused_mut)]
    let mut tx_buffer = String::new();

    // SENSOR
    let sensor_i = Sensor {
        pin: pin_i_number,
        delay: &mut delay,
        sysloop: sysloop.clone(),
        one_wire_bus: &mut one_wire_bus_i,
        count: 0,
    };

    vec![sensor_i].iter_mut().for_each(|sensor| {
        // ID
        warn!("SENSOR_ID: pin: {}", sensor.pin);

        if let Err(search_error) = sensor.find_devices(false) {
            error!("Error device_search: {search_error:?}");
        };
    });

    // <FREEZER> set with alarm -30 <-> 0
    let rom_to_change = one_wire_bus::Address(SINGLE_ROM);

    vec![
        (&mut one_wire_bus_i, pin_i_number),
        (&mut one_wire_bus_ii, pin_ii_number),
    ]
    .iter_mut()
    .for_each(|(bus, id)| {
        // ID
        warn!("ID: {id}");

        /*
        // FIND
        if let Err(search_error) =
            sensor_ds::find_devices(&mut delay, bus, false, &mut tx_buffer)
        {
            error!("Error device_search: {search_error:?}");
        };

        warn!("TX BUFFER: {tx_buffer}");
        */

        // LIST
        let device_list = sensor_ds::list_devices(&mut delay, bus);

        if let Some(list) = device_list {
            list.iter().for_each(|device| {
                // VIEW CONFIG
                if let Err(e) =
                    sensor_ds::view_config(&mut delay, bus, *device, false, sysloop.clone())
                {
                    error!("view_config <FREEZER>: {e:?}");
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

            /* // <FREEZER> set alarm limits
            if list.contains(&rom_to_change) {
                warn!("ROM {rom_to_change:?} available we can set_config");

                // /* // CONFIG CHANGE
                if let Err(e) = sensor_ds::set_config(&mut delay,
                                                      &mut one_wire_bus_ii,
                                                      rom_to_change, // ROM
                                                      0, // TH
                                                      -30, // TL
                                                      None, // resolution
                ) {
                    error!(" ERROR setting config: {:?}", e);
                }
                // */
            } else {
                error!("ROM {rom_to_change:?} not found {list:?}");
            }
            */
        }
    });
    //_

    sleep.delay_ms(WTD_FEEDER_DURATION);

    // LOOP
    loop {
        // FOR_EACH
        vec![
            (&mut one_wire_bus_i, pin_i_number),
            (&mut one_wire_bus_ii, pin_ii_number),
        ]
        .iter_mut()
        .for_each(|(bus, id)| {
            // ID
            warn!("ID: {id}");

            // via SINGLE
            if let Err(e) = sensor_ds::get_temperature_by_one(&mut delay, bus, false) {
                error!("TEMPERATURE SINGLE result: {e:?}");
            };

            sleep.delay_ms(WTD_FEEDER_DURATION);

            // via ALL
            if let Err(e) = sensor_ds::get_temperature(&mut delay, bus, false) {
                error!("TEMPERATURE ALL result: {e:?}");
            };

            // <FREEZER> as SINGLE DEVICE
            // VIEW
            if let Err(e) =
                sensor_ds::view_config(&mut delay, bus, rom_to_change, false, sysloop.clone())
            {
                error!("ERROR <FREEZER> {rom_to_change:X?} view_config: {e:?}");
            }

            // MEASURE
            if let Err(e) = sensor_ds::get_device_temperature(&mut delay, bus, false, rom_to_change)
            {
                error!("ERROR <FREEZER> {rom_to_change:X?} get_device_temperature: {e:?}");
            }
        });
        //_

        sleep.delay_ms(SLEEP_DURATION);
    }
}
