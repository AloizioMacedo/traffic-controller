use anyhow::Result;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let peripherals = Peripherals::take()?;
    let mut led = PinDriver::output(peripherals.pins.gpio4)?;

    for x in (0..2).cycle() {
        if x == 0 {
            log::info!("High!");
            _ = led.set_high();
        } else {
            log::info!("Low!");
            _ = led.set_low();
        }

        FreeRtos::delay_ms(1000);
    }

    Ok(())
}

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    main_logic()
}
