use anyhow::{anyhow, Result};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::LevelFilter;
use std::ptr;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self, EspSntp, SntpConf, SyncStatus};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use esp_idf_sys::time_t;
use heapless::String as HLString;

const WIFI_SSID: &str = "Wokwi-GUEST";
const WIFI_PASS: &str = "";

const STAGES: [u64; 3] = [3, 2, 5];
const SUM_STAGES: i64 = sum(&STAGES);
const CUM_SUM_STAGES: [i64; 3] = cum_sum(&STAGES);

const OFFSET: i64 = 0;

const LOG_MAX_LEVEL: LevelFilter = LevelFilter::Info;

const fn sum(stages: &[u64]) -> i64 {
    (stages[0] + stages[1] + stages[2]) as i64
}

const fn cum_sum(stages: &[u64; 3]) -> [i64; 3] {
    [
        stages[0] as i64,
        (stages[0] + stages[1]) as i64,
        (stages[0] + stages[1] + stages[2]) as i64,
    ]
}

fn wait_until_time_is_synched(sntp: &EspSntp) {
    while sntp.get_sync_status() != SyncStatus::Completed {
        FreeRtos::delay_ms(200);
    }
    log::info!("NTP synced")
}

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let peripherals = Peripherals::take()?;

    let mut led1 = PinDriver::output(peripherals.pins.gpio16)?;
    let mut led2 = PinDriver::output(peripherals.pins.gpio4)?;
    let mut led3 = PinDriver::output(peripherals.pins.gpio0)?;

    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;
    connect_wifi(&mut wifi)?;

    let conf = SntpConf {
        operating_mode: sntp::OperatingMode::Poll,
        sync_mode: sntp::SyncMode::Smooth,
        ..Default::default()
    };
    let esp_sntp = sntp::EspSntp::new(&conf)?;

    wait_until_time_is_synched(&esp_sntp);

    let mut stage_idx = None;

    loop {
        let now = std::time::SystemTime::now();
        let elapsed_since_epoch = now.duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        if ((elapsed_since_epoch.as_secs() as i64 + OFFSET) % SUM_STAGES) < CUM_SUM_STAGES[0] {
            _ = led2.set_low();
            _ = led3.set_low();
            _ = led1.set_high();

            if let Some(i) = stage_idx {
                if i != 0 {
                    log::info!("red");
                    stage_idx = Some(0);
                }
            } else {
                stage_idx = Some(0);
            }
        } else if ((elapsed_since_epoch.as_secs() as i64 + OFFSET) % SUM_STAGES) < CUM_SUM_STAGES[1]
        {
            _ = led1.set_low();
            _ = led3.set_low();
            _ = led2.set_high();

            if let Some(i) = stage_idx {
                if i != 1 {
                    log::info!("yellow");
                    stage_idx = Some(1);
                }
            } else {
                stage_idx = Some(1);
            }
        } else if ((elapsed_since_epoch.as_secs() as i64 + OFFSET) % SUM_STAGES) < CUM_SUM_STAGES[2]
        {
            _ = led1.set_low();
            _ = led2.set_low();
            _ = led3.set_high();

            if let Some(i) = stage_idx {
                if i != 2 {
                    log::info!("green");
                    stage_idx = Some(2);
                }
            } else {
                stage_idx = Some(2);
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

fn _get_time(sntp: &EspSntp) -> Result<i64> {
    log::info!("SNTP initialized, waiting for status...");

    while sntp.get_sync_status() != SyncStatus::Completed {}

    log::info!("SNTP status received");

    let timer: *mut time_t = ptr::null_mut();
    let timestamp = unsafe { esp_idf_sys::time(timer) };

    Ok(timestamp)
}
