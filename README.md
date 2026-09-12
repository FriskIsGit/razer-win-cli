# Razer CLI for Windows

A self-contained CLI for controlling Razer device settings—DPI, RGB lighting, polling rate, and profiles—directly over USB HID on Windows.

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


## Supported devices

| Device                   | USB PID  | Type     | DPI Range | Lighting | Polling |
|--------------------------|----------|----------|-----------|----------|---------|
| Razer DeathAdder Elite   | `0x005C` | Mouse    | 100–16000 | ✓        | ✓       |
| Razer DeathAdder V2      | `0x0084` | Mouse    | 100–20000 | ✓        | ✓       |
| Razer DeathAdder V2 Mini | `0x008C` | Mouse    | 100–8500  | ✓        | ✓       |
| Razer Basilisk V3        | `0x0099` | Mouse    | 100–26000 | ✓        | ✓       |
| Razer BlackWidow V3      | `0x024E` | Keyboard | —         | ✓        | ✓       |

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
├── devices/                # per-device TOML definitions
│   ├── deathadder-elite.toml   # PR #5 — target device
│   ├── deathadder-v2.toml
│   ├── deathadder-v2-mini.toml
│   ├── basilisk-v3.toml
│   └── blackwidow-v3.toml
└── src/
    └── main.rs             # CLI entry point (all prototype logic lives here)
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
