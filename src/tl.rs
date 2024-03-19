use anyhow::{anyhow, Result};
use esp_idf_svc::hal::gpio::{Output, PinDriver, Pins};
use thiserror::Error;

pub enum Color {
    Green,
    Yellow,
    Red,
}
pub struct State {
    /// Information of the color in each traffic light, in order.
    pub traffic_lights_colors: Vec<Color>,

    /// Duration of the state in seconds.
    pub duration: u64,
}

pub trait ColorSetter {
    fn set_color(&mut self, color: &Color) -> Result<()>;
}

pub struct TrafficLight<'a, R, Y, G>
where
    R: esp_idf_svc::hal::gpio::Pin,
    Y: esp_idf_svc::hal::gpio::Pin,
    G: esp_idf_svc::hal::gpio::Pin,
{
    pub red: PinDriver<'a, R, Output>,
    pub yellow: PinDriver<'a, Y, Output>,
    pub green: PinDriver<'a, G, Output>,
}

pub fn build_traffic_lights(pins: Pins) -> Result<Vec<Box<dyn ColorSetter>>> {
    let tl0_red = PinDriver::output(pins.gpio21)?;
    let tl0_yellow = PinDriver::output(pins.gpio19)?;
    let tl0_green = PinDriver::output(pins.gpio18)?;

    let tl0 = TrafficLight {
        red: tl0_red,
        yellow: tl0_yellow,
        green: tl0_green,
    };

    let tl1_red = PinDriver::output(pins.gpio26)?;
    let tl1_yellow = PinDriver::output(pins.gpio27)?;
    let tl1_green = PinDriver::output(pins.gpio13)?;

    let tl1 = TrafficLight {
        red: tl1_red,
        yellow: tl1_yellow,
        green: tl1_green,
    };

    let tl2_red = PinDriver::output(pins.gpio32)?;
    let tl2_yellow = PinDriver::output(pins.gpio33)?;
    let tl2_green = PinDriver::output(pins.gpio25)?;

    let tl2 = TrafficLight {
        red: tl2_red,
        yellow: tl2_yellow,
        green: tl2_green,
    };

    Ok(vec![Box::new(tl0), Box::new(tl1), Box::new(tl2)])
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

fn can_yellow_go_low(res: Result<(), LedSettingError>) -> Result<bool> {
    match res {
        Ok(_) => Ok(true),
        Err(LedSettingError::UnableToSet) => Ok(false),
        Err(LedSettingError::UnableToRaiseYellow) => {
            Err(anyhow!("not possible to set yellow to high"))
        }
    }
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

#[derive(Error, Debug)]
enum LedSettingError {
    #[error("unable to set")]
    UnableToSet,

    #[error("unable to set yellow to high")]
    UnableToRaiseYellow,
}
