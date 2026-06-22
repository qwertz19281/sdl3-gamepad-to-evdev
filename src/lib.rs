use anyhow::Context;

use crate::config::Config;
use crate::event_loop::entry;
use crate::parsed_config::ParsedConfig;

pub mod config;
pub mod simulated;
pub mod parsed_config;
pub mod button_tracker;
pub mod event_loop;

pub fn init(cfg: &Config) -> anyhow::Result<()> {
    eprintln!("\nConfig: {cfg:#?}");

    let parsed_config = ParsedConfig::parse(cfg).context("parsing context")?;

    eprintln!("\nParsed Config: {parsed_config:#?}\n");

    entry(cfg, &parsed_config)
}
