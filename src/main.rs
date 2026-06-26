use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use sdl3toevdev::config::Config;
use sdl3toevdev::init;
use sdl3toevdev::parsed_config::match_sdl_button_string;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let data = fs::read(&args.config).context("Failed to read config file")?;

    let config = toml::from_slice::<Config>(&data).context("Failed to decode config file")?;

    let app_args = sdl3toevdev::Args {
        dump_parse_config: args.verbose,
        center_calib: args.calib_center.as_ref().map(|v| match_sdl_button_string(v).unwrap() ),
    };

    init(&config, app_args)?;

    Ok(())
}

/// sd3toevdev
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// verbose
    #[arg(short='v')]
    pub verbose: bool,
    #[arg(long="calib-center")]
    pub calib_center: Option<String>,
    /// Path to config toml file
    #[arg()]
    pub config: PathBuf,
}
