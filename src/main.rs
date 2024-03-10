use anyhow::Result;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use std::ptr;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{self, EspSntp, SyncStatus};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use esp_idf_sys::time_t;
use heapless::String as HLString;

fn wait_until_time_is_synched(sntp: &EspSntp) {
    while sntp.get_sync_status() != SyncStatus::Completed {
        FreeRtos::delay_ms(200);
    }
    log::info!("NTP synced")
}

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let stages = [20, 5, 35]; // Seconds.
    let sum_stages: u64 = stages.iter().sum();

    let offset = 0;

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

    let sntp = sntp::EspSntp::new_default()?;

    wait_until_time_is_synched(&sntp);

    loop {
        let now = std::time::SystemTime::now();
        log::info!("current system time: {:?}", now);

        let now = now.duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        if ((now.as_secs() + offset) % sum_stages) < 20 {
            _ = led2.set_low();
            _ = led3.set_low();
            _ = led1.set_high();

            log::info!("red")
        } else if ((now.as_secs() + offset) % sum_stages) < 25 {
            _ = led1.set_low();
            _ = led3.set_low();
            _ = led2.set_high();

            log::info!("yellow")
        } else if ((now.as_secs() + offset) % sum_stages) < 35 {
            _ = led1.set_low();
            _ = led2.set_low();
            _ = led3.set_high();

            log::info!("green")
        }

        FreeRtos::delay_ms(500);
    }
}

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    main_logic()
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let wifi_ssid: HLString<32> = HLString::try_from("Wokwi-GUEST").unwrap();
    let wifi_password: HLString<64> = HLString::try_from("").unwrap();

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: wifi_ssid,
        bssid: None,
        auth_method: AuthMethod::None,
        password: wifi_password,
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    log::info!("Wifi started");

    wifi.connect()?;
    log::info!("Wifi connected");

    wifi.wait_netif_up()?;
    log::info!("Wifi netif up");

    Ok(())
}

fn _get_time(sntp: &EspSntp) -> Result<i64> {
    log::info!("SNTP initialized, waiting for status!");

    while sntp.get_sync_status() != SyncStatus::Completed {}

    log::info!("SNTP status received!");

    let timer: *mut time_t = ptr::null_mut();
    let timestamp = unsafe { esp_idf_sys::time(timer) };

    Ok(timestamp)
}
