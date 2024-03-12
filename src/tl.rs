use anyhow::{anyhow, Result};
use esp_idf_svc::hal::gpio::{Output, PinDriver};
use thiserror::Error;

pub enum Color {
    Green,
    Yellow,
    Red,
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
