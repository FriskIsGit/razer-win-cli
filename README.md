# razer-win-cli

A self-contained CLI for controlling Razer device settings 
(DPI, RGB lighting, polling rate, profiles) directly over USB HID on Windows. 
The `razer-hid` protocol library is vendored **unmodified** under `crates/razer-hid/`.
The `hidapi-rs` bindings library is vendored **modified** under `crates/hidapi-rs/`.

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


## The --pid flag

`--pid` (or `-p`) is **optional**. When omitted, the tool auto-detects the
single connected Razer device. If multiple devices are connected, `--pid` is
required.

```
razer-win-cli dpi 1600                     # auto-detect device, set 1600x1600
razer-win-cli dpi 1600 800 --pid 0x005C    # explicit PID
razer-win-cli color 255 0 128 --pid 5c     # PID without 0x prefix
razer-win-cli info                         # auto-detect, show device info
```

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
  razer-win-cli <command> [args...] [--pid <pid>]

  --pid <pid>   Optional. Accepts 0x005c, 005C, or 5c.
                When omitted, auto-detects the single connected Razer device.

DEVICE:
  list                                      Enumerate attached Razer devices + registry
  info                                      Show device details (serial, firmware, capabilities)
  battery                                   Read battery level + charging status

DPI:
  dpi <x> [y]                               Set DPI (y defaults to x)
  getdpi                                    Read current DPI
  dpi-stages <active> <v1> [<v2> ...]       Set 2-5 DPI stages; <active> is the 0-based index

LIGHTING / RGB:
  color <r> <g> <b> [led]                   Set a static colour
  effect <effect> [led] [r g b]             Set lighting effect (static|breathing|spectrum|wave|reactive|none)
  brightness <0-255> [led]                  Set LED brightness
  getbrightness [led]                       Read LED brightness

POLLING:
  polling <hz>                              Set polling rate (125/500/1000)
  getpolling                                Read polling rate

PROFILES:
  profile save <name> [--pid <pid>] [flags] Save settings as a named profile
    --dpi <x> <y>          DPI to save
    --effect <e>           Lighting effect
    --rgb <r> <g> <b>      RGB colour
    --brightness <0-255>   Brightness
    --polling <hz>         Polling rate
  profile apply <name>                      Apply a saved profile to connected devices
  profile list                              List saved profiles
  profile show <name>                       Print a saved profile as JSON
  profile delete <name>                     Delete a saved profile
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
|   ├── hidapi-rs/         # vendored bindings library (MODIFIED)
│   └── razer-hid/         # vendored protocol library (UNMODIFIED)
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

1. **Driver mode** — `open_device()` calls `set_device_mode(DRIVER)` after
   opening. Without this, some devices intermittently reject config commands
   with status `0x05` (NOT_SUPPORTED). OpenRazer does this on daemon startup.

2. **Retry wrapper** — `with_retry()` wraps each device command with up to 3
   attempts (100ms apart). The transport layer's internal retry (5×10ms on
   BUSY) usually isn't enough; the outer retry catches intermittent
   `BusyExhausted` and `NOT_SUPPORTED` errors.

3. **VARSTORE for DPI** — DPI commands use `VARSTORE` (0x01), not `NOSTORE`
   (0x00). Devices reject `NOSTORE` for DPI with `0x05 NOT_SUPPORTED`. Lighting
   commands use `NOSTORE` (volatile). This matches the `ctl.rs` example.

## License

GPL-2.0-only — protocol facts ported from [OpenRazer](https://github.com/openrazer/openrazer).
