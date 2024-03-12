mod config_parser;

use anyhow::{anyhow, Result};
use config_parser::parse_config;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::LevelFilter;
use thiserror::Error;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self, EspSntp, SyncStatus};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use heapless::String as HLString;

const CONFIG: &str = include_str!("../tl.config");

const WIFI_SSID: &str = "Wokwi-GUEST";
const WIFI_PASS: &str = "";

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

struct TrafficLight<'a, R, Y, G>
where
    R: esp_idf_svc::hal::gpio::Pin,
    Y: esp_idf_svc::hal::gpio::Pin,
    G: esp_idf_svc::hal::gpio::Pin,
{
    pub red: PinDriver<'a, R, Output>,
    pub yellow: PinDriver<'a, Y, Output>,
    pub green: PinDriver<'a, G, Output>,
}

fn sum(states: &[State]) -> i64 {
    states.iter().map(|stage| stage.duration as i64).sum()
}

fn cum_sum(states: &[State]) -> Vec<i64> {
    let mut cum_sum = vec![0; states.len()];

    states.iter().enumerate().fold(0, |acc, (i, state)| {
        let acc = acc + state.duration;
        cum_sum[i] = acc as i64;

        acc
    });

    cum_sum
}

struct State {
    traffic_lights: Vec<Color>,
    duration: u64,
}

enum Color {
    Green,
    Yellow,
    Red,
}

trait ColorSetter {
    fn set_color(&mut self, color: &Color) -> Result<()>;
}

impl<'a, R, Y, G> ColorSetter for TrafficLight<'a, R, Y, G>
where
    R: esp_idf_svc::hal::gpio::Pin,
    Y: esp_idf_svc::hal::gpio::Pin,
    G: esp_idf_svc::hal::gpio::Pin,
{
    fn set_color(&mut self, color: &Color) -> Result<()> {
        match color {
            Color::Green => {
                let is_allowed_from_low =
                    can_yellow_go_low(set_low_safe(&mut self.red, &mut self.yellow))?;
                let is_allowed_from_high =
                    can_yellow_go_low(set_high_safe(&mut self.green, &mut self.yellow))?;

                if is_allowed_from_low && is_allowed_from_high {
                    _ = self.yellow.set_low();
                }
            }
            Color::Yellow => {
                _ = set_low_safe(&mut self.red, &mut self.yellow);
                _ = set_low_safe(&mut self.green, &mut self.yellow);

                self.yellow.set_high()?;
            }
            Color::Red => {
                let is_allowed_from_low =
                    can_yellow_go_low(set_low_safe(&mut self.green, &mut self.yellow))?;
                let is_allowed_from_high =
                    can_yellow_go_low(set_high_safe(&mut self.red, &mut self.yellow))?;

                if is_allowed_from_low && is_allowed_from_high {
                    _ = self.yellow.set_low();
                }
            }
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
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

#[derive(Error, Debug)]
enum LedSettingError {
    #[error("unable to set")]
    UnableToSet,

    #[error("unable to set yellow to high")]
    UnableToRaiseYellow,
}

fn set_low_safe<Pin1, Pin2, MODE>(
    to_set_low1: &mut PinDriver<Pin1, MODE>,
    yellow: &mut PinDriver<Pin2, MODE>,
) -> Result<(), LedSettingError>
where
    Pin1: esp_idf_svc::hal::gpio::Pin,
    Pin2: esp_idf_svc::hal::gpio::Pin,
    MODE: esp_idf_svc::hal::gpio::OutputMode,
{
    if to_set_low1.set_low().is_err() {
        yellow
            .set_high()
            .map_err(|_| LedSettingError::UnableToRaiseYellow)?;

        return Err(LedSettingError::UnableToSet);
    };

    Ok(())
}

fn set_high_safe<Pin1, Pin2, MODE>(
    to_set_high: &mut PinDriver<Pin1, MODE>,
    yellow: &mut PinDriver<Pin2, MODE>,
) -> Result<(), LedSettingError>
where
    Pin1: esp_idf_svc::hal::gpio::Pin,
    Pin2: esp_idf_svc::hal::gpio::Pin,
    MODE: esp_idf_svc::hal::gpio::OutputMode,
{
    if to_set_high.set_high().is_err() {
        yellow
            .set_high()
            .map_err(|_| LedSettingError::UnableToRaiseYellow)?;

        return Err(LedSettingError::UnableToSet);
    };

    Ok(())
}

fn can_yellow_go_low(res: Result<(), LedSettingError>) -> Result<bool> {
    match res {
        Ok(_) => Ok(true),
        Err(LedSettingError::UnableToSet) => Ok(false),
        Err(LedSettingError::UnableToRaiseYellow) => {
            Err(anyhow!("not possible to set yellow to high"))
        }
    }
}

// Uses WiFi and SNTP to sync time.
fn sync_time(wifi: &mut BlockingWifi<EspWifi<'static>>, esp_sntp: &EspSntp<'_>) -> Result<()> {
    connect_wifi(wifi)?;
    wait_until_time_is_synched(esp_sntp);

    Ok(())
}

// Connects the WiFi.
fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> Result<()> {
    let wifi_ssid: HLString<32> =
        HLString::try_from(WIFI_SSID).map_err(|_| anyhow!("ssid is more than 32 bytes"))?;
    let wifi_password: HLString<64> = HLString::try_from(WIFI_PASS)
        .map_err(|_| anyhow!("wifi password is more than 64 bytes"))?;

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: wifi_ssid,
        bssid: None,
        auth_method: AuthMethod::None,
        password: wifi_password,
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    log::info!("wifi started");

    wifi.connect()?;
    log::info!("wifi connected");

    wifi.wait_netif_up()?;
    log::info!("wifi netif up");

    Ok(())
}

// Waits to proceed until time is synched through SNTP.
fn wait_until_time_is_synched(sntp: &EspSntp) {
    while sntp.get_sync_status() != SyncStatus::Completed {
        FreeRtos::delay_ms(200);
    }
    log::info!("NTP synced")
}
