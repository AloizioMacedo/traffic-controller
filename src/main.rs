mod config_parser;
mod tl;
mod utils;
mod wifi;

use anyhow::Result;
use config_parser::{parse_config, Config};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::LevelFilter;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use tl::{build_traffic_lights, Color, ColorSetter};
use utils::{cum_sum, sum};
use wifi::sync_time;

const CONFIG: &str = include_str!("../tl.config");

const LOG_MAX_LEVEL: LevelFilter = LevelFilter::Info;

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(LOG_MAX_LEVEL);

    let Config { states, offset } = parse_config(CONFIG)?;

    let peripherals = Peripherals::take()?;

    // Most of those have to stay alive in order to keep the connection and update the
    // clock. Do *not* drop them.
    let modem = peripherals.modem;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;
    let esp_sntp = sntp::EspSntp::new_default()?;

    sync_time(&mut wifi, &esp_sntp)?;

    let sum_states: i64 = sum(&states);
    let cum_sum_states = cum_sum(&states);

    let mut tls = build_traffic_lights(peripherals.pins)?;

    main_loop(&states, offset, sum_states, &cum_sum_states, &mut tls)
}

struct State {
    traffic_lights: Vec<Color>,
    duration: u64,
}

fn main_loop(
    states: &[State],
    offset: i64,
    sum_states: i64,
    cum_sum_states: &[i64],
    tls: &mut [Box<dyn ColorSetter>],
) -> Result<()> {
    loop {
        let now = std::time::SystemTime::now();
        let elapsed_since_epoch = now.duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        let (_, state) = cum_sum_states
            .iter()
            .zip(states)
            .find(|(cum_sum, _)| {
                (elapsed_since_epoch.as_secs() as i64 - offset) % sum_states < **cum_sum
            })
            .expect("(sum % sum_stages) should always be less than some cum_sum");

        for (tl, color) in tls.iter_mut().zip(&state.traffic_lights) {
            tl.set_color(color)?;
        }

        FreeRtos::delay_ms(100);
    }
}
