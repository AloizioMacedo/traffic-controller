use anyhow::{anyhow, Result};
use esp_idf_svc::{
    hal::delay::FreeRtos,
    sntp::{EspSntp, SyncStatus},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use heapless::String as HLString;

const WIFI_SSID: &str = "Wokwi-GUEST";
const WIFI_PASS: &str = "";

// Uses WiFi and SNTP to sync time.
pub fn sync_time(wifi: &mut BlockingWifi<EspWifi<'static>>, esp_sntp: &EspSntp<'_>) -> Result<()> {
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

    let wifi_configuration = Configuration::Client(ClientConfiguration {
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
