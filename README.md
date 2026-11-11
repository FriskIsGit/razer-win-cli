# Razer CLI for Windows

A self-contained CLI for controlling Razer device settings - DPI, RGB lighting, polling rate, and profiles;
directly over USB HID on Windows.

The **razer-hid** protocol library is vendored under `crates/razer-hid` with minimal modifications 
to its configuration structure to support additional configuration values. 

The **hidapi-rs** bindings library is vendored under `crates/hidapi-rs` with a minimal compatibility modification 
replacing `GetOverlappedResultEx` with `GetOverlappedResult` for broader Windows compatibility.

## Preview
<img width="657" height="307" alt="main" src="https://github.com/user-attachments/assets/a8e4c2f7-c90f-496b-acce-d10081e4adb5" />
<img width="665" height="428" alt="lighting" src="https://github.com/user-attachments/assets/81c8ed64-2db3-4403-9ef7-c44604cf4862" />

## Building
Prerequisite: [Rust](https://www.rust-lang.org/tools/install)

On Windows, check your active toolchain with `rustup toolchain list`.

### MSVC targets
If you're using the MSVC toolchain, install [Microsoft Build Tools](https://aka.ms/vs/stable/vs_BuildTools.exe) or, 
for a lighter install, use: https://github.com/Data-Oriented-House/PortableBuildTools

PortableBuildTools includes its own setup scripts and a number of environment paths.
To make this easier, the repository includes helper scripts in the root. 
Just edit the `BUILD_TOOLS` variable in the script to point to your installation directory, and you’re done.

Run it whenever you open a new shell.
```shell
./stage_env.ps1
```

```shell
./stage_env.bat
```

### GNU targets
If you're using a `*-pc-windows-gnu` target:
1. Go to https://winlibs.com
2. Scroll down to the **MSVCRT** builds.
3. Download the package for your architecture, preferably the one with **POSIX threads**.
4. Extract the archive.
5. Add `mingw64\bin` to your `PATH`.


## Commands

```
USAGE:
  cli <command> [args...] [--pid <pid>]

  --pid <pid>   Optional. Accepts 0x005c, 005C, or 5c.
                When omitted, auto-detects the single connected Razer device.
                Required if multiple devices are connected.

DEVICE:
  list                                       Enumerate attached Razer devices + registry
  info                                       Show device details (serial, firmware, capabilities)
  battery                                    Read battery level + charging status

PERFORMANCE:
  dpi                                        Read current DPI
  dpi <x> [y]                                Set DPI (y defaults to x)
  dpi-stages <active> <v1> [<v2> ...]        Set 2-5 DPI stages; <active> is the 0-based index
  polling                                    Read polling rate
  polling <hz>                               Set polling rate (125/500/1000)

LIGHTING / RGB:
  color <r> <g> <b> [led]                    Set a static color
  effect <effect> <r> <g> <b> [--led <id>]   Set a lighting effect
  brightness <0-255> [--led <id>]            Set LED brightness
  brightness [--led <id>]                    Read LED brightness

  EFFECTS: static | breathing | spectrum | wave | reactive | none
    Some effects, such as spectrum and wave, don’t require an RGB value.

PROFILES:
  profile save <name> [flags]  Save settings as a named profile
    --dpi <x> <y>              DPI on X,Y axis
    --polling <hz>             Polling rate
    --led <id>                 Select an LED zone
    --effect <e>               Led's lighting effect
    --speed <1-4>              Effect speed
    --rgb <r> <g> <b>          Led's RGB color
    --brightness <0-255>       Led's brightness

  profile list                 List saved profiles
  profile apply <name>         Apply a saved profile to connected devices
  profile show <name>          Print a saved profile as JSON
  profile delete <name>        Delete a saved profile

  LED SELECTION:
    LED settings apply to the currently selected zone.
    Repeat --led <id> to select and configure another zone.
```

`<led>` is a hex LED id (default: `0x04` logo). Common: `0x01` scroll, `0x04`
logo, `0x05` backlight.

Profiles are stored as JSON files in `~/.razer-win-cli/profiles/` on Unix 
and `%USERPROFILE%\.razer-win-cli\profiles\` on Windows.
You can override the storage location with `RAZER_CLI_PROFILES_DIR` environment variable.

## Supported devices

| Device                                                     | USB PID  | Type     | DPI Range | Lighting | Polling |
|------------------------------------------------------------|----------|----------|-----------|----------|---------|
| Razer Orochi 2011                                          | `0x0013` | Mouse    | 100-4000  | yes      | yes     |
| Razer Naga                                                 | `0x0015` | Mouse    | 100-5600  | yes      | yes     |
| Razer DeathAdder 3.5G                                      | `0x0016` | Mouse    | 100-3500  | yes      | yes     |
| Razer Naga Epic                                            | `0x001F` | Mouse    | 100-5600  | yes      | yes     |
| Razer Abyssus 1800                                         | `0x0020` | Mouse    | 100-1800  | yes      | yes     |
| Razer Mamba 2012 (Wired)                                   | `0x0024` | Mouse    | 100-6400  | yes      | yes     |
| Razer Mamba 2012 (Wireless)                                | `0x0025` | Mouse    | 100-6400  | yes      | yes     |
| Razer DeathAdder 3.5G Black                                | `0x0029` | Mouse    | 100-3500  | yes      | yes     |
| Razer Naga 2012                                            | `0x002E` | Mouse    | 100-5600  | yes      | yes     |
| Razer Imperator 2012                                       | `0x002F` | Mouse    | 100-6400  | yes      | yes     |
| Razer Ouroboros 2012                                       | `0x0032` | Mouse    | 100-8200  | yes      | yes     |
| Razer Taipan                                               | `0x0034` | Mouse    | 100-8200  | yes      | yes     |
| Razer Naga Hex (Red)                                       | `0x0036` | Mouse    | 100-5600  | yes      | yes     |
| Razer DeathAdder 2013                                      | `0x0037` | Mouse    | 100-6400  | yes      | yes     |
| Razer DeathAdder 1800                                      | `0x0038` | Mouse    | 100-1800  | yes      | yes     |
| Razer Orochi 2013                                          | `0x0039` | Mouse    | 100-6400  | yes      | yes     |
| Razer Naga Epic Chroma (Wired)                             | `0x003E` | Mouse    | 100-8200  | yes      | yes     |
| Razer Naga Epic Chroma (Wireless)                          | `0x003F` | Mouse    | 100-8200  | yes      | yes     |
| Razer Naga 2014                                            | `0x0040` | Mouse    | 100-8200  | yes      | yes     |
| Razer Naga Hex                                             | `0x0041` | Mouse    | 100-5600  | yes      | yes     |
| Razer Abyssus 2014                                         | `0x0042` | Mouse    | -         | yes      | yes     |
| Razer DeathAdder Chroma                                    | `0x0043` | Mouse    | 100-10000 | yes      | yes     |
| Razer Mamba (Wired)                                        | `0x0044` | Mouse    | 100-16000 | yes      | yes     |
| Razer Mamba (Wireless)                                     | `0x0045` | Mouse    | 100-16000 | yes      | yes     |
| Razer Mamba Tournament Edition                             | `0x0046` | Mouse    | 100-16000 | yes      | -       |
| Razer Orochi (Wired)                                       | `0x0048` | Mouse    | 100-8200  | yes      | yes     |
| Razer Diamondback Chroma                                   | `0x004C` | Mouse    | 100-16000 | yes      | -       |
| Razer DeathAdder 2000                                      | `0x004F` | Mouse    | 100-2000  | yes      | yes     |
| Razer Naga Hex V2                                          | `0x0050` | Mouse    | 100-16000 | yes      | yes     |
| Razer Naga Chroma                                          | `0x0053` | Mouse    | 100-16000 | yes      | yes     |
| Razer DeathAdder 3500                                      | `0x0054` | Mouse    | 100-3500  | yes      | yes     |
| Razer Lancehead (Wired)                                    | `0x0059` | Mouse    | 100-16000 | yes      | yes     |
| Razer Lancehead (Wireless)                                 | `0x005A` | Mouse    | 100-16000 | yes      | yes     |
| Razer Abyssus V2                                           | `0x005B` | Mouse    | 100-5000  | yes      | yes     |
| Razer DeathAdder Elite                                     | `0x005C` | Mouse    | 100-16000 | yes      | yes     |
| Razer Abyssus 2000                                         | `0x005E` | Mouse    | 100-2000  | yes      | yes     |
| Razer Lancehead Tournament Edition                         | `0x0060` | Mouse    | 100-16000 | yes      | yes     |
| Razer Atheris (Receiver)                                   | `0x0062` | Mouse    | 100-7200  | yes      | yes     |
| Razer Basilisk                                             | `0x0064` | Mouse    | 100-16000 | yes      | yes     |
| Razer Basilisk Essential                                   | `0x0065` | Mouse    | 100-6400  | yes      | yes     |
| Razer Naga Trinity                                         | `0x0067` | Mouse    | 100-16000 | yes      | yes     |
| Razer Abyssus Elite (D.Va Edition)                         | `0x006A` | Mouse    | 100-7200  | yes      | yes     |
| Razer Abyssus Essential                                    | `0x006B` | Mouse    | 100-7200  | yes      | yes     |
| Razer Mamba Elite (Wired)                                  | `0x006C` | Mouse    | 100-16000 | yes      | yes     |
| Razer DeathAdder Essential                                 | `0x006E` | Mouse    | 100-6400  | yes      | yes     |
| Razer Lancehead Wireless (Receiver)                        | `0x006F` | Mouse    | 100-16000 | yes      | yes     |
| Razer Lancehead Wireless (Wired)                           | `0x0070` | Mouse    | 100-16000 | yes      | yes     |
| Razer DeathAdder Essential (White Edition)                 | `0x0071` | Mouse    | 100-6400  | yes      | yes     |
| Razer Mamba Wireless (Receiver)                            | `0x0072` | Mouse    | 100-16000 | yes      | yes     |
| Razer Mamba Wireless (Wired)                               | `0x0073` | Mouse    | 100-16000 | yes      | yes     |
| Razer Pro Click (Receiver)                                 | `0x0077` | Mouse    | 100-16000 | yes      | yes     |
| Razer Viper                                                | `0x0078` | Mouse    | 100-16000 | yes      | yes     |
| Razer Viper Ultimate (Wired)                               | `0x007A` | Mouse    | 100-20000 | yes      | yes     |
| Razer Viper Ultimate (Wireless)                            | `0x007B` | Mouse    | 100-20000 | yes      | yes     |
| Razer DeathAdder V2 Pro (Wired)                            | `0x007C` | Mouse    | 100-20000 | yes      | yes     |
| Razer DeathAdder V2 Pro (Wireless)                         | `0x007D` | Mouse    | 100-20000 | yes      | yes     |
| Razer Pro Click (Wired)                                    | `0x0080` | Mouse    | 100-16000 | yes      | yes     |
| Razer Basilisk X HyperSpeed                                | `0x0083` | Mouse    | 100-16000 | yes      | yes     |
| Razer DeathAdder V2                                        | `0x0084` | Mouse    | 100-20000 | yes      | yes     |
| Razer Basilisk V2                                          | `0x0085` | Mouse    | 100-20000 | yes      | yes     |
| Razer Basilisk Ultimate (Wired)                            | `0x0086` | Mouse    | 100-20000 | yes      | yes     |
| Razer Basilisk Ultimate (Receiver)                         | `0x0088` | Mouse    | 100-20000 | yes      | yes     |
| Razer Viper Mini                                           | `0x008A` | Mouse    | 100-8500  | yes      | yes     |
| Razer DeathAdder V2 Mini                                   | `0x008C` | Mouse    | 100-8500  | yes      | yes     |
| Razer Naga Left-Handed Edition                             | `0x008D` | Mouse    | 100-20000 | yes      | yes     |
| Razer Naga Pro (Wired)                                     | `0x008F` | Mouse    | 100-20000 | yes      | yes     |
| Razer Naga Pro (Wireless)                                  | `0x0090` | Mouse    | 100-20000 | yes      | yes     |
| Razer Viper 8KHz                                           | `0x0091` | Mouse    | 100-20000 | yes      | yes     |
| Razer Orochi V2 (Receiver)                                 | `0x0094` | Mouse    | 100-18000 | yes      | yes     |
| Razer Orochi V2 (Bluetooth)                                | `0x0095` | Mouse    | 100-18000 | yes      | yes     |
| Razer Naga X                                               | `0x0096` | Mouse    | 100-18000 | yes      | yes     |
| Razer DeathAdder Essential (2021)                          | `0x0098` | Mouse    | 100-6400  | yes      | yes     |
| Razer Basilisk V3                                          | `0x0099` | Mouse    | 100-26000 | yes      | yes     |
| Razer Pro Click Mini (Receiver)                            | `0x009A` | Mouse    | 100-12000 | yes      | yes     |
| Razer DeathAdder V2 X HyperSpeed                           | `0x009C` | Mouse    | 100-14000 | yes      | yes     |
| Razer Viper Mini Signature Edition (Wired)                 | `0x009E` | Mouse    | 100-30000 | yes      | yes     |
| Razer Viper Mini Signature Edition (Wireless)              | `0x009F` | Mouse    | 100-30000 | yes      | yes     |
| Razer DeathAdder V2 Lite                                   | `0x00A1` | Mouse    | 100-8500  | yes      | yes     |
| Razer Cobra                                                | `0x00A3` | Mouse    | 100-8500  | yes      | yes     |
| Razer Viper V2 Pro (Wired)                                 | `0x00A5` | Mouse    | 100-30000 | yes      | yes     |
| Razer Viper V2 Pro (Wireless)                              | `0x00A6` | Mouse    | 100-30000 | yes      | yes     |
| Razer Naga V2 Pro (Wired)                                  | `0x00A7` | Mouse    | 100-30000 | yes      | yes     |
| Razer Naga V2 Pro (Wireless)                               | `0x00A8` | Mouse    | 100-30000 | yes      | yes     |
| Razer Basilisk V3 Pro (Wired)                              | `0x00AA` | Mouse    | 100-30000 | yes      | yes     |
| Razer Basilisk V3 Pro (Wireless)                           | `0x00AB` | Mouse    | 100-30000 | yes      | yes     |
| Razer Cobra Pro (Wired)                                    | `0x00AF` | Mouse    | 100-30000 | yes      | yes     |
| Razer Cobra Pro (Wireless)                                 | `0x00B0` | Mouse    | 100-30000 | yes      | yes     |
| Razer DeathAdder V3                                        | `0x00B2` | Mouse    | 100-30000 | yes      | yes     |
| Razer HyperPolling Wireless Dongle                         | `0x00B3` | Mouse    | 100-30000 | yes      | yes     |
| Razer Naga V2 HyperSpeed (Receiver)                        | `0x00B4` | Mouse    | 100-30000 | yes      | yes     |
| Razer DeathAdder V3 Pro (Wired)                            | `0x00B6` | Mouse    | 100-35000 | yes      | yes     |
| Razer DeathAdder V3 Pro (Wireless)                         | `0x00B7` | Mouse    | 100-35000 | yes      | yes     |
| Razer Viper V3 HyperSpeed                                  | `0x00B8` | Mouse    | 100-30000 | yes      | yes     |
| Razer Basilisk V3 X HyperSpeed                             | `0x00B9` | Mouse    | 100-18000 | yes      | yes     |
| Razer DeathAdder V4 Pro (Wired)                            | `0x00BE` | Mouse    | 100-45000 | yes      | yes     |
| Razer DeathAdder V4 Pro (Wireless)                         | `0x00BF` | Mouse    | 100-45000 | yes      | yes     |
| Razer Viper V3 Pro (Wired)                                 | `0x00C0` | Mouse    | 100-35000 | yes      | yes     |
| Razer Viper V3 Pro (Wireless)                              | `0x00C1` | Mouse    | 100-35000 | yes      | yes     |
| Razer DeathAdder V3 Pro (Wired)                            | `0x00C2` | Mouse    | 100-35000 | yes      | yes     |
| Razer DeathAdder V3 Pro (Wireless)                         | `0x00C3` | Mouse    | 100-35000 | yes      | yes     |
| Razer DeathAdder V3 HyperSpeed (Wired)                     | `0x00C4` | Mouse    | 100-26000 | yes      | yes     |
| Razer DeathAdder V3 HyperSpeed (Wireless)                  | `0x00C5` | Mouse    | 100-26000 | yes      | yes     |
| Razer Pro Click V2 Vertical Edition (Wired)                | `0x00C7` | Mouse    | 100-30000 | yes      | yes     |
| Razer Pro Click V2 Vertical Edition (Wireless)             | `0x00C8` | Mouse    | 100-30000 | yes      | yes     |
| Razer Basilisk V3 35K                                      | `0x00CB` | Mouse    | 100-35000 | yes      | yes     |
| Razer Basilisk V3 Pro 35K (Wired)                          | `0x00CC` | Mouse    | 100-35000 | yes      | yes     |
| Razer Basilisk V3 Pro 35K (Wireless)                       | `0x00CD` | Mouse    | 100-35000 | yes      | yes     |
| Razer Pro Click V2 (Wired)                                 | `0x00D0` | Mouse    | 100-30000 | yes      | yes     |
| Razer Pro Click V2 (Wireless)                              | `0x00D1` | Mouse    | 100-30000 | yes      | yes     |
| Razer Basilisk Mobile (Wired)                              | `0x00D3` | Mouse    | 100-18000 | yes      | yes     |
| Razer Basilisk Mobile (Receiver)                           | `0x00D4` | Mouse    | 100-18000 | yes      | yes     |
| Razer Basilisk V3 Pro 35K Phantom Green Edition (Wired)    | `0x00D6` | Mouse    | 100-35000 | yes      | yes     |
| Razer Basilisk V3 Pro 35K Phantom Green Edition (Wireless) | `0x00D7` | Mouse    | 100-35000 | yes      | yes     |
| Razer Cobra HyperSpeed (Wired)                             | `0x00DA` | Mouse    | 100-26000 | yes      | yes     |
| Razer Cobra HyperSpeed (Wireless)                          | `0x00DB` | Mouse    | 100-26000 | yes      | yes     |
| Razer Viper V3 Pro SE (Wired)                              | `0x00DE` | Mouse    | 100-35000 | yes      | yes     |
| Razer Viper V3 Pro SE (Wireless)                           | `0x00DF` | Mouse    | 100-35000 | yes      | yes     |
| Razer BlackWidow V3                                        | `0x024E` | Keyboard | -         | yes      | yes     |

Some mice have no OpenRazer lighting data; their TOMLs mark effects as optimistically assumed and require hardware testing.

## Project structure

```
Proto/
├── Cargo.toml           # workspace root + binary package
├── FINDINGS.md          # codebase analysis findings
├── README.md            # this file
├── crates/
|   ├── hidapi-rs/         # vendored bindings library
│   └── razer-hid/         # vendored protocol library
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs       # crate root + re-exports
│           ├── report.rs    # 90-byte Razer report codec (pack/unpack + CRC)
│           ├── transport.rs # Windows/Linux HID feature-report transport
│           ├── registry.rs  # TOML device registry (PID → capabilities)
│           └── commands/
│               ├── mod.rs    # VARSTORE/NOSTORE constants
│               ├── lighting.rs # RGB effects (extended 0x0F + classic 0x03)
│               ├── dpi.rs    # DPI set/get + DPI stages
│               ├── polling.rs # Polling rate (classic + HyperPolling v2)
│               ├── power.rs  # Battery/charging/idle/threshold
│               └── info.rs   # Serial, firmware, device mode
├── devices/                # per-device TOML definitions (117 mice + 1 keyboard)
└── src/
    ├── main.rs             # CLI entry point (all prototype logic lives here)
    └── device_list.rs      # generated include_str! list for devices/*.toml
```

## Notes on device flakiness

The prototype handles two intermittent issues observed on the DeathAdder Elite,
both **in `src/main.rs`** (the vendored crate is untouched):

1. **Driver mode** — `open_device()` calls `set_device_mode(DRIVER)` after opening. 
   Without this, some devices may intermittently reject config commands
   with status `0x05` (NOT_SUPPORTED). OpenRazer does this on daemon startup.

2. **Persistence** — Testing has revealed that both `VARSTORE` (0x01) and `NOSTORE` (0x00)
   are supported for DPI, polling, and lighting commands. 
   The ctl.rs examples use `VARSTORE` for DPI and `NOSTORE` for lighting. 
   This convention was followed because DPI settings are generally expected to persist, 
   while lighting settings are less important to store and can be treated as volatile.

## License

GPL-2.0-only — protocol facts ported from [OpenRazer](https://github.com/openrazer/openrazer).
