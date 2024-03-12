use anyhow::{anyhow, Result};

use crate::{tl::Color, State};

impl TryFrom<char> for Color {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'r' => Ok(Color::Red),
            'g' => Ok(Color::Green),
            'y' => Ok(Color::Yellow),
            _ => Err(anyhow!("parse error in config")),
        }
    }
}

fn parse_line(line: &str) -> Result<State> {
    let (state_chars, duration) = line
        .split_once(' ')
        .ok_or(anyhow!("line in config not separated by space"))?;

    let duration = duration.parse()?;
    let traffic_lights = state_chars
        .chars()
        .map(Color::try_from)
        .collect::<Result<Vec<_>>>()?;

    Ok(State {
        traffic_lights,
        duration,
    })
}

pub fn parse_config(config: &str) -> Result<Vec<State>> {
    config.lines().map(parse_line).collect()
}
