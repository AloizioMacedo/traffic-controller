use anyhow::Result;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;

// Wrapped up by main, avoiding the boilerplate of link_patches and initialize_default.
fn main_logic() -> Result<()> {
    let stages = [6, 2, 8]; // Seconds.
    let peripherals = Peripherals::take()?;
    let mut led1 = PinDriver::output(peripherals.pins.gpio16)?;
    let mut led2 = PinDriver::output(peripherals.pins.gpio4)?;
    let mut led3 = PinDriver::output(peripherals.pins.gpio0)?;

    for (i, dur) in stages.iter().enumerate().cycle() {
        if i == 0 {
            _ = led2.set_low();
            _ = led3.set_low();
            _ = led1.set_high();

            log::info!("red");
        } else if i == 1 {
            _ = led1.set_low();
            _ = led3.set_low();
            _ = led2.set_high();

            log::info!("yellow");
        } else {
            _ = led1.set_low();
            _ = led2.set_low();
            _ = led3.set_high();

            log::info!("green");
        }

        FreeRtos::delay_ms(dur * 1000);
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
