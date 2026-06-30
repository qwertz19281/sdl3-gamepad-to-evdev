# SDL3 gamepad to evdev bridge

This CLI program can bridge a SDL3-compatible gamepad to a virtual evdev gamepad.

This for examples allows to use the new Steam Controller with older games without SDL3 or native Steam Controller support, without Steam / Steam Input.

Current Features:
- configurable and mappable buttons/axes/hats
- stick deadzone
- gyro, accelerometer passthrough
- basic rumble support

Not (yet) implemented:
- Touchpad
- HD rumble (not supported by SDL3, would require gamepad-specific raw commands/effects)
- multiple gamepads (can be worked around by running multiple instances, and by filtering by gamepad serial)
- sdl3_joystick without sdl3_gamepad support

# Install

## Download release builds on [GitHub Releases](https://github.com/qwertz19281/sdl3-gamepad-to-evdev/releases), with SDL3 statically linked into a single binary

## Build & install with cargo

```sh
cargo install --locked --git https://github.com/qwertz19281/sdl3-gamepad-to-evdev --branch release --features "build_sdl3_static"
```

| feature flag | description |
| --- | --- |
| none | Headers of a recent SDL3 version like 3.4.10 should be provided. |
| `build_sdl3` | Build SDL3 from source for SDL3-to-evdev, link dynamically. |
| `build_sdl3_static` | Build SDL3 from source and statically link into sdl3-to-evdev, for a self-contained binary. |

# Run

`sdl3_to_evdev path-to-preset.toml`

It will automatically detect when the configured gamepad connects/disconnects and then open or close the virtual evdev device.

sdl3-to-evdev exits on receiving Ctrl+C / SIGINT.

The presets defines mappings, filter for input gamepad, id for output evdev, and more. See [sc2_to_ds4.toml](sc2_to_ds4.toml) for an example.

Note that the button and axis mapping is kinda fragile with evdev. You may have to adjust the mappings in the presets if the appear swapped in a game, and you can't or don't want to change mappings in-game. Especially the "+extra" presets which map more buttons than the to-emulate gamepads have are affected.

For the new Steam Controller, you may need to add udev rules so it can be fully accessed by the user, see [sc2.md](sc2.md).

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

sdl3-to-evdev is tested with a real Steam Controller, DS4, RetroArch, Dolphin, Firefox.
