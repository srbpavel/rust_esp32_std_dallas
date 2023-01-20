mod eventloop;
mod sensor_ds;

use eventloop::EventLoopMessage;

use sensor_ds::Route;
use sensor_ds::Sensor;

use one_wire_bus::OneWire;

use ds18b20::Resolution;

use embedded_hal::blocking::delay::DelayMs;

use esp_idf_svc::eventloop::EspSystemEventLoop;

use esp_idf_sys as _;

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

use std::time::Instant;

const EVENTLOOP_INFO: bool = true; //false;

const SLEEP_DURATION: u16 = 60 * 1000;
const WTD_FEEDER_DURATION: u16 = 100;

// param's for setting new alarm and resolution
const ALARM_CHANGE: bool = false;
const TH: i8 = 55;
const TL: i8 = 0;
const RESOLUTION: Option<Resolution> = None;
//const RESOLUTION: Option<Resolution> = Some(Resolution::Bits12);

const SINGLE_ALARM_CHANGE: bool = false;
const SINGLE_ROM: u64 = 0x3600000CFAB44428; // <FREEZER>
const SINGLE_TH: i8 = 0;
const SINGLE_TL: i8 = -30;
const SINGLE_RESOLUTION: Option<Resolution> = None;
//const SINGLE_RESOLUTION: Option<Resolution> = Some(Resolution::Bits12);

//
fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    EspLogger::initialize_default();

    let machine_boot = EspSystemTime {}.now();
    warn!("duration since machine_boot: {machine_boot:?}");

    let sysloop = EspSystemEventLoop::take()?;
    warn!("event_loop init");
    let _subscription = sysloop.subscribe(move |msg: &EventLoopMessage| {
        if EVENTLOOP_INFO.eq(&true) {
            info!(">>> <{}> {}", msg.duration.as_secs(), msg.data);
        }
    })?;

    let mut sleep = FreeRtos {};
    let mut delay = Ets {};

    let peripherals = esp_idf_hal::peripherals::Peripherals::take().unwrap();
    // GPIO2 turn RGB sometimes!
    let pin_i = peripherals.pins.gpio6.downgrade();
    let pin_ii = peripherals.pins.gpio4.downgrade();
    // no devices here, just to verify error handling
    let pin_iii = peripherals.pins.gpio1.downgrade();

    let mut all_sensors: Vec<Sensor<_>> = vec![pin_i, pin_ii, pin_iii]
        .into_iter()
        .filter_map(|pin| match PinDriver::input_output_od(pin) {
            Ok(driver) => {
                let pin = driver.pin();

                match OneWire::new(driver) {
                    Ok(one_wire_bus) => Some(Sensor {
                        pin,
                        sysloop: sysloop.clone(),
                        one_wire_bus,
                    }),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        })
        .collect();

    // <FREEZER> set with negative
    let rom_to_change = one_wire_bus::Address(SINGLE_ROM);

    // ONCE
    all_sensors.iter_mut().for_each(|sensor| {
        // LIST
        warn!("@list devices at pin: {}", sensor.pin);
        let device_list = sensor.list_devices(&mut delay);

        if let Some(list) = device_list {
            list.iter().for_each(|device| {
                // VIEW CONFIG
                warn!("@view device config: {:x}", device.0);
                match sensor.view_config(&mut delay, *device, false) {
                    Ok(c) => info!("{c}"),
                    Err(e) => error!("view config: {e:?}"),
                }

                // SET CONFIG for ALL in LIST
                if ALARM_CHANGE.eq(&true) {
                    warn!("@set new config for all devices");
                    if let Err(e) = sensor.set_config(
                        &mut delay, *device, TH, // TH 55
                        TL, // TL 0
                        RESOLUTION,
                    ) {
                        error!("setting config for all devices: {e:?}");
                    }
                }

                // <FREEZER> set alarm limits
                warn!("@verify if device is the one to change config on");
                if device.eq(&rom_to_change) {
                    warn!("@set new config for single device {rom_to_change:?}");

                    // CONFIG CHANGE
                    if SINGLE_ALARM_CHANGE.eq(&true) {
                        if let Err(e) = sensor.set_config(
                            &mut delay,
                            rom_to_change,
                            SINGLE_TH, // TH 0
                            SINGLE_TL, // TL -30
                            SINGLE_RESOLUTION,
                        ) {
                            error!("setting config: {e:?}");
                        }
                    }
                } else {
                    warn!("{rom_to_change:?} not found");
                }
            });
        };
    });

    sleep.delay_ms(WTD_FEEDER_DURATION);

    // LOOP
    let mut cycle_counter: u64 = 0;

    loop {
        cycle_counter += 1;
        warn!("i: [{cycle_counter}]");

        // SENSOR
        all_sensors
            .iter_mut()
            .for_each(|sensor| {
                // ByOne
                warn!("@measure temperature for all sensors OneByOne");

                let start = Instant::now();
                match sensor.measure(&mut delay, false, Route::ByOne) {
                    Ok(m) => m.iter().for_each(|m| info!("{m}")),
                    Err(e) => error!("[{}] MEASURE single result: {e:?}", sensor.pin),
                }
                let end = Instant::now();
                error!("Route::ByOne duration -> {:?}",
                       end.duration_since(start),
                );

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
                warn!("@view device config: {rom_to_change:x?}"); //xxx
                match sensor.view_config(&mut delay, rom_to_change, false) {
                    Ok(c) => info!("{c}"),
                    Err(e) => error!(
                        "[{}] <FREEZER> {rom_to_change:?} view_config: {e:?}",
                        sensor.pin
                    ),
                }

                // MEASURE Device(Address)
                warn!("@measure device {rom_to_change:?}");
                match sensor.measure(&mut delay, false, Route::Device(rom_to_change)) {
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
