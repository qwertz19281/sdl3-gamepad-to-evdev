use std::fs;

use anyhow::Context as _;
use sdl3_to_evdev::Args;
use sdl3_to_evdev::config::Config;
use sdl3_to_evdev::parsed_config::ParsedConfig;

pub fn main() -> anyhow::Result<()> {
    let args = Args {
        dump_parse_config: false,
        verbose: false,
    };

    for f in fs::read_dir(".")? {
        let f = f?;
        let path = f.path();
        let name = f.file_name();
        let name = name.to_string_lossy();

        if !(name.starts_with("sc2_") && name.ends_with(".toml")) {
            continue;
        }

        eprintln!("Trying to parse config: {}", f.file_name().to_string_lossy());

        let data = fs::read(path).context("reading config file")?;

        let cfg = toml::from_slice::<Config>(&data).context("decoding config file")?;

        if args.dump_parse_config {
            eprintln!("\nConfig: {cfg:#?}");
        }

        let parsed_config = ParsedConfig::parse(&cfg, &args).context("parsing config")?;

        if args.dump_parse_config {
            eprintln!("\nParsed Config: {parsed_config:#?}\n");
        }
    }

    Ok(())
}
