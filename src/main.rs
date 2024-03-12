mod config_parser;
mod tl;
mod utils;
mod wifi;

use anyhow::Result;
use config_parser::parse_config;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::LevelFilter;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use tl::{Color, ColorSetter, TrafficLight};
use utils::{cum_sum, sum};
use wifi::sync_time;

const CONFIG: &str = include_str!("../tl.config");

const OFFSET: i64 = 0;

const LOG_MAX_LEVEL: LevelFilter = LevelFilter::Info;

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::set_max_level(LOG_MAX_LEVEL);

    let states = parse_config(CONFIG)?;
    let sum_states: i64 = sum(&states);
    let cum_sum_states = cum_sum(&states);

    let peripherals = Peripherals::take()?;

    // Leds
    let tl0_red: PinDriver<'_, Gpio16, Output> = PinDriver::output(peripherals.pins.gpio16)?;
    let tl0_yellow: PinDriver<'_, Gpio4, Output> = PinDriver::output(peripherals.pins.gpio4)?;
    let tl0_green: PinDriver<'_, Gpio0, Output> = PinDriver::output(peripherals.pins.gpio0)?;

    let mut tl0 = TrafficLight {
        red: tl0_red,
        yellow: tl0_yellow,
        green: tl0_green,
    };

    let tl1_red = PinDriver::output(peripherals.pins.gpio26)?;
    let tl1_yellow = PinDriver::output(peripherals.pins.gpio27)?;
    let tl1_green = PinDriver::output(peripherals.pins.gpio14)?;

    let mut tl1 = TrafficLight {
        red: tl1_red,
        yellow: tl1_yellow,
        green: tl1_green,
    };

    // Most of those have to stay alive in order to keep the connection and update the
    // clock. Do *not* drop them.
    let modem = peripherals.modem;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    let esp_sntp = sntp::EspSntp::new_default()?;

    sync_time(&mut wifi, &esp_sntp)?;

    main_loop(cum_sum_states, states, sum_states, vec![&mut tl0, &mut tl1])
}

struct State {
    traffic_lights: Vec<Color>,
    duration: u64,
}

fn main_loop(
    cum_sum_stages: Vec<i64>,
    states: Vec<State>,
    sum_stages: i64,
    mut tls: Vec<&mut dyn ColorSetter>,
) -> Result<()> {
    loop {
        let now = std::time::SystemTime::now();
        let elapsed_since_epoch = now.duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        for (cum_sum, state) in cum_sum_stages.iter().zip(&states) {
            if ((elapsed_since_epoch.as_secs() as i64 - OFFSET) % sum_stages) < *cum_sum {
                for (tl, color) in tls.iter_mut().zip(&state.traffic_lights) {
                    tl.set_color(color)?;
                }

                break;
            }
        }

        FreeRtos::delay_ms(100);
    }
}
