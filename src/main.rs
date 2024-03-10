use anyhow::Result;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;

// Wi-Fi
use esp_idf_svc::eventloop::*;
use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::wifi::*;
use heapless::String as HLString;

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let stages = [6, 2, 8]; // Seconds.
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

    loop {
        let now = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH)?;

        log::info!("Now: {:?}", std::time::SystemTime::now());
        log::info!("Unix Epoch: {:?}", std::time::SystemTime::UNIX_EPOCH);
        log::info!("Duration since: {:?}", now);

        if ((now.as_secs() + offset) % sum_stages) < 6 {
            _ = led2.set_low();
            _ = led3.set_low();
            _ = led1.set_high();

            log::info!("red")
        } else if ((now.as_secs() + offset) % sum_stages) < 8 {
            _ = led1.set_low();
            _ = led3.set_low();
            _ = led2.set_high();

            log::info!("yellow")
        } else if ((now.as_secs() + offset) % sum_stages) < 16 {
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
    let wifi_ssid: HLString<32> = HLString::try_from("ALOIZIO-5G").unwrap();
    let wifi_password: HLString<64> = HLString::try_from("21072107").unwrap();

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
