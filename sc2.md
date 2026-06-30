# Steam Controller 2026 udev rules

On Linux you need to have udev rules set for the Steam Controller so it can be fully accessed by normal users.

Installing Steam will automatically install these udev rules, else add these udev rules (sourced from steam installer):

```
# Valve USB devices
SUBSYSTEMS=="usb", ATTRS{idVendor}=="28de", MODE="0660", TAG+="uaccess"

# Steam Controller udev write access
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"

# Valve HID devices over USB hidraw
KERNEL=="hidraw*", ATTRS{idVendor}=="28de", MODE="0660", TAG+="uaccess"

# Valve HID devices over bluetooth hidraw
KERNEL=="hidraw*", KERNELS=="*28DE:*", MODE="0660", TAG+="uaccess"
```

e.g. into a file under /etc/udev.d/rules/

After adding the udev rules or installing Steam, either run `sudo udevadm control --reload-rules && sudo udevadm trigger` or reboot to apply the udev rules.

# Steam Controller 2026 exposed button/axes as of sdl3-to-evdev 0.1 and SDL 3.4.10

SDL 3.4.10 default mapping:

Raw index | SDL3 gamepad button code | Steam Controller button
--- | --- | ---
`0` | `South` | A
`1` | `East` | B
`2` | `West` | X
`3` | `North` | Y
`4` | `Back` | View / Select
`5` | `Guide` | Steam (middle button)
`6` | `Start` | Menu / Start
`7` | `Left Stick` | L3 (press left analog stick)
`8` | `Right Stick` | R3 (press right analog stick)
`9` | `Left Shoulder` | L1
`10` | `Right Shoulder` | R2
`11` | `Misc1` | Quick Access Menu (between touchpads)
`12` | `Right Paddle 1` | R4
`13` | `Left Paddle 1` | L4
`14` | `Right Paddle 2` | R5
`15` | `Left Paddle 2` | L5
`16` | `Misc2` | pressing right touchpad (this will change with newer SDL version)
`17` | `Touchpad` | pressing left touchpad (this will change with newer SDL version)
`18` | `Misc4` | Right analog stick capacitive sensor (this will change with newer SDL version)
`19` | `Misc3` | Left analog stick capacitive sensor (this will change with newer SDL version)
`20` | `Misc6` | Right capacitive grip (this will change with newer SDL version)
`21` | `Misc5` | Left capacitive grip (this will change with newer SDL version)

Raw index | SDL3 gamepad axis code | Steam Controller axis
--- | --- | ---
`0` | `LeftX` | Left analog stick X axis
`1` | `LeftY` | Left analog stick Y axis
`2` | `RightX` | Right analog stick X axis
`3` | `RightY` | Right analog stick Y axis
`4` | `Left Trigger` | L2
`5` | `Right Trigger` | R2

D-pad is exposed as hats from joypad API, but also as "dpad up/down/left/right" buttons from gamepad API.

For this default mapping: ` 03002854de2800000413000002006800,*,a:b0,b:b1,back:b4,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b5,leftshoulder:b9,leftstick:b7,lefttrigger:a4,leftx:a0,lefty:a1,rightshoulder:b10,rightstick:b8,righttrigger:a5,rightx:a2,righty:a3,start:b6,x:b2,y:b3,misc1:b11,paddle1:b12,paddle2:b13,paddle3:b14,paddle4:b15,touchpad:b17,misc2:b16,misc3:b19,misc4:b18,misc5:b21,misc6:b20,crc:5428,platform:Linux,`
