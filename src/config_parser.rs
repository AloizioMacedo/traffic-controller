use anyhow::{anyhow, Result};

use crate::{tl::Color, State};

const MAX_NUM_TLS: usize = 4;

pub struct Config {
    pub states: Vec<State>,
    pub offset: i64,
}

impl TryFrom<char> for Color {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'r' => Ok(Color::Red),
            'g' => Ok(Color::Green),
            'y' => Ok(Color::Yellow),
            _ => Err(anyhow!(
                "parse error in config: color '{value}' not recognized. Choose 'r', 'g' or 'y'"
            )),
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
        traffic_lights_colors: traffic_lights,
        duration,
    })
}

pub fn parse_config(config: &str) -> Result<Config> {
    let mut lines = config.lines().peekable();

    let first_line = lines.peek().ok_or(anyhow!("empty configuration"))?;

    let (offset, should_skip_first_line) = if let Ok(offset) = first_line.parse() {
        (offset, true)
    } else {
        (0, false)
    };

    let states = lines
        .skip(if should_skip_first_line { 1 } else { 0 })
        .skip_while(|l| l.is_empty())
        .map(parse_line)
        .collect::<Result<Vec<_>>>()?;

    let number_of_colors = states
        .first()
        .map(|s| s.traffic_lights_colors.len())
        .unwrap_or(0);

    if states
        .iter()
        .any(|s| s.traffic_lights_colors.len() != number_of_colors)
    {
        return Err(anyhow!(
            "mismatched number of traffic light colors in different lines"
        ));
    }

    if states
        .iter()
        .any(|s| s.traffic_lights_colors.len() > MAX_NUM_TLS)
    {
        return Err(anyhow!(
            "mismatched number of traffic light colors in different lines"
        ));
    }

    Ok(Config { states, offset })
}
