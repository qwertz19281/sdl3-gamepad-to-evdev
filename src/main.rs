use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use sdl3_to_evdev::config::Config;
use sdl3_to_evdev::init;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let data = fs::read(&args.config).context("reading config file")?;

    let config = toml::from_slice::<Config>(&data).context("decoding config file")?;

    let app_args = sdl3_to_evdev::Args {
        dump_parse_config: args.dump_parsed_config,
        verbose: args.verbose,
    };

    init(&config, app_args)?;

    Ok(())
}

/// sd3-to-evdev
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// verbose
    #[arg(short='v')]
    pub verbose: bool,
    /// dump parsed config
    #[arg(long="dump-parsed-config")]
    pub dump_parsed_config: bool,
    /// Path to config toml file
    #[arg()]
    pub config: PathBuf,
}
