# SDL3 gamepad to evdev bridge

This program can bridge a SDL3-compatible gamepad to a virtual evdev gamepad.

This for examples allows to use the new Steam Controller with older games without SDL3 or native Steam Controller support, without Steam / Steam Input.

Current Features:
- configurable and mappable buttons/axes/hats
- stick deadzone
- gyro, accelerometer passthrough
- basic rumble support

Not (yet) implemented:
- Touchpad
- HD rumble (not supported by SDL3, would require gamepad-specific raw commands/effects)
- multiple gamepads (can be worked around by running multiple instances)
- sdl3_joystick without sdl3_gamepad support

# Install

## Release build on GitHub Releases, with SDL3 statically linked into a single binary

## Build & install with cargo

```sh
cargo install --locked --git https://github.com/qwertz19281/sdl3-to-evdev --branch release --features "build_sdl3_static"
```

| feature flag | description |
| --- | --- |
| none | build sdl3 from source for sdl3-to-evdev, look for sdl3 source/headers via pkg-config. A recent sdl3 version like 3.4.10 should be provided. |
`build_sdl3` | build sdl3 from source for sdl3-to-evdev.
`build_sdl3_static` | build sdl3 from source and statically link into sdl3-to-evdev, for self-contained binary.

# Run

`sdl3-to-evdev path-to-config.toml`

It will automatically detect when the configured gamepad connects/disconnects and then open or close the virtual evdev device.

sdl3-to-evdev exits on recieving Ctrl+C / SIGINT.

See [sc2_to_ds4.toml](sc2_to_ds4.toml) for a configuration example.

"+extra" profiles map additional buttons the to-emulate gamepad doesn't have, and as excess buttons may mess up evdev button order, so it may only work for apps/games that support button remapping.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

### AI Disclosure

The code is human-written and doesn't contain any significant AI-generated sections.

AI/LLM were used in research, results are plausibility and quality checked.

sdl3-to-evdev is tested with a real steam controller, DS4, RetroArch, dolphin, firefox.
