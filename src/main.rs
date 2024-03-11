use anyhow::{anyhow, Result};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::LevelFilter;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self, EspSntp, SyncStatus};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use heapless::String as HLString;

const N_STATES: usize = 6;
const N_TLS: usize = 2;

const WIFI_SSID: &str = "Wokwi-GUEST";
const WIFI_PASS: &str = "";

const STATES: [State; N_STATES] = [
    State {
        traffic_lights: [Color::Red, Color::Green],
        duration: 4,
    },
    State {
        traffic_lights: [Color::Red, Color::Yellow],
        duration: 2,
    },
    State {
        traffic_lights: [Color::Red, Color::Red],
        duration: 2,
    },
    State {
        traffic_lights: [Color::Green, Color::Red],
        duration: 8,
    },
    State {
        traffic_lights: [Color::Yellow, Color::Red],
        duration: 2,
    },
    State {
        traffic_lights: [Color::Red, Color::Red],
        duration: 2,
    },
];

const OFFSET: i64 = 0;

const LOG_MAX_LEVEL: LevelFilter = LevelFilter::Info;

fn sum(states: &[State]) -> i64 {
    states.iter().map(|stage| stage.duration as i64).sum()
}

fn cum_sum(states: &[State]) -> [i64; N_STATES] {
    let mut cum_sum: [i64; N_STATES] = [0; N_STATES];

    states
        .iter()
        .enumerate()
        .take(N_STATES)
        .fold(0, |acc, (i, state)| {
            let acc = acc + state.duration;
            cum_sum[i] = acc as i64;

            acc
        });

    cum_sum
}

struct State {
    traffic_lights: [Color; N_TLS],
    duration: u64,
}

enum Color {
    Green,
    Yellow,
    Red,
}

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let sum_stages: i64 = sum(&STATES);
    let cum_sum_stages: [i64; N_STATES] = cum_sum(&STATES);

    let peripherals = Peripherals::take()?;

    let mut tl0_red = PinDriver::output(peripherals.pins.gpio16)?;
    let mut tl0_yellow = PinDriver::output(peripherals.pins.gpio4)?;
    let mut tl0_green = PinDriver::output(peripherals.pins.gpio0)?;

    let mut tl1_red = PinDriver::output(peripherals.pins.gpio26)?;
    let mut tl1_yellow = PinDriver::output(peripherals.pins.gpio27)?;
    let mut tl1_green = PinDriver::output(peripherals.pins.gpio14)?;

    // Most of those have to stay alive in order to keep the connection and update the
    // clock. Do *not* drop them.
    let modem = peripherals.modem;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    let esp_sntp = sntp::EspSntp::new_default()?;

    sync_time(&mut wifi, &esp_sntp)?;

    // Main flow for the traffic light is below.

    loop {
        let now = std::time::SystemTime::now();
        let elapsed_since_epoch = now.duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        for (cum_sum, state) in cum_sum_stages.iter().zip(&STATES) {
            if ((elapsed_since_epoch.as_secs() as i64 + OFFSET) % sum_stages) < *cum_sum {
                for (i, tl_color) in state.traffic_lights.iter().enumerate() {
                    match i {
                        0 => match tl_color {
                            Color::Green => {
                                _ = tl0_red.set_low();
                                _ = tl0_yellow.set_low();
                                _ = tl0_green.set_high();
                            }
                            Color::Yellow => {
                                _ = tl0_red.set_low();
                                _ = tl0_green.set_low();
                                _ = tl0_yellow.set_high();
                            }
                            Color::Red => {
                                _ = tl0_green.set_low();
                                _ = tl0_yellow.set_low();
                                _ = tl0_red.set_high();
                            }
                        },
                        1 => match tl_color {
                            Color::Green => {
                                _ = tl1_red.set_low();
                                _ = tl1_yellow.set_low();
                                _ = tl1_green.set_high();
                            }
                            Color::Yellow => {
                                _ = tl1_red.set_low();
                                _ = tl1_green.set_low();
                                _ = tl1_yellow.set_high();
                            }
                            Color::Red => {
                                _ = tl1_green.set_low();
                                _ = tl1_yellow.set_low();
                                _ = tl1_red.set_high();
                            }
                        },
                        _ => unreachable!(),
                    };
                }

                break;
            }
        }

        FreeRtos::delay_ms(100);
    }
}

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::set_max_level(LOG_MAX_LEVEL);

    main_logic()
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
