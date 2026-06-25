use std::fmt;

use anyhow::Context;

use crate::config::Config;
use crate::event_loop::entry;
use crate::parsed_config::ParsedConfig;

pub mod config;
pub mod simulated;
pub mod parsed_config;
pub mod button_tracker;
pub mod event_loop;
pub mod event_processing;
pub mod simulated_gyro;

pub fn init(cfg: &Config, verbose: bool) -> anyhow::Result<()> {
    if verbose {
        eprintln!("\nConfig: {cfg:#?}");
    }

    let parsed_config = ParsedConfig::parse(cfg).context("parsing context")?;

    if verbose {
        eprintln!("\nParsed Config: {parsed_config:#?}\n");
    }

    entry(cfg, &parsed_config)
}

struct FmtOpt<T>(T);

impl<T> fmt::Display for FmtOpt<&Option<T>> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => write!(f, "{v}"),
            None => write!(f, "??"),
        }
    }
}

impl<T,E> fmt::Display for FmtOpt<&Result<T,E>> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Ok(v) => write!(f, "{v}"),
            Err(_) => write!(f, "??"),
        }
    }
}

struct FmtOptHex<T>(T);

impl<T> fmt::Display for FmtOptHex<&Option<T>> where T: fmt::UpperHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => write!(f, "{v:#06X}"),
            None => write!(f, "??"),
        }
    }
}

fn none_vec<T>(len: usize) -> Vec<Option<T>> {
    std::iter::repeat_with(|| None).take(len).collect::<Vec<_>>()
}
