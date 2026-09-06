mod tui;
mod cmd;
mod inputs;

use std::env;
use std::fmt::{Debug};
use std::process::ExitCode;

use hidapi::HidApi;
use razer_hid::commands::lighting::{led_id, Rgb};
use razer_hid::{DeviceDef, Registry};

// =========================================================================
// Profile model (mirrors the app's profiles.rs domain model)
// =========================================================================

use serde::{Deserialize, Serialize};
use razer_hid::registry::Effect;
use crate::cmd::open_device;



#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct LightingSettings {
    effect: Effect,
    color: Rgb,
    brightness: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct DpiSettings {
    x: u16,
    y: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DeviceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lighting: Option<LightingSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dpi: Option<DpiSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    polling_hz: Option<u16>,
}

impl DeviceEntry {
    pub fn new(id: u16, settings: DeviceSettings) -> Self {
        Self { id, settings }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeviceEntry {
    id: u16,
    #[serde(default)]
    settings: DeviceSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Profile {
    name: String,
    #[serde(default)]
    devices: Vec<DeviceEntry>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::new(),
            devices: Default::default(),
        }
    }
}

// =========================================================================
// Registry & device helpers
// =========================================================================

// Device TOMLs are embedded at compile time so the binary is fully self-contained.
const DEVICE_SOURCES: &[&str] = &[
    include_str!("../devices/basilisk-v3.toml"),
    include_str!("../devices/blackwidow-v3.toml"),
    include_str!("../devices/deathadder-elite.toml"),
    include_str!("../devices/deathadder-v2-mini.toml"),
    include_str!("../devices/deathadder-v2.toml"),
];

fn load_registry() -> Registry {
    let mut registry = Registry::new();
    for src in DEVICE_SOURCES {
        match Registry::parse_def(src) {
            Ok(def) => registry.push(def),
            Err(e) => eprintln!("warning: embedded device definition failed to parse: {e}"),
        }
    }
    registry
}

/// Parses an optional LED value, defaulting to [`led_id::LOGO`] if absent.
fn parse_led(raw: Option<&String>) -> Result<u8, String> {
    match raw {
        Some(r) => parse_hex_to_u8(r),
        None => Ok(led_id::LOGO),
    }
}

fn strip_hex_prefix(raw: &str) -> String {
    let lowercase = raw.to_lowercase();
    lowercase.trim_start_matches("0x").to_owned()
}


fn parse_hex_to_u16(raw: &str) -> Result<u16, String> {
    let trimmed = strip_hex_prefix(raw);
    u16::from_str_radix(&trimmed, 16)
        .map_err(|e| format!("invalid hex id {raw:?}: {e}"))
}

fn parse_hex_to_u8(raw: &str) -> Result<u8, String> {
    let trimmed = strip_hex_prefix(raw);
    u8::from_str_radix(&trimmed, 16)
        .map_err(|e| format!("invalid hex id {raw:?}: {e}"))
}

// TODO Get rid of this method
fn led_id_for(def: &DeviceDef) -> u8 {
    def.led_regions.first().map(|r| r.id).unwrap_or(led_id::LOGO)
}


fn update_lighting_color(settings: &mut DeviceSettings, color: [u8; 3]) {
    match &mut settings.lighting {
        Some(lighting) => lighting.color = color,
        None =>
            settings.lighting = Some(LightingSettings {
            effect: Effect::Static,
            color,
            brightness: 255,
        })
    }
}

// =========================================================================
// Entry point
// =========================================================================

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let is_help = args.iter().any(|s| s == "help" || s == "-h" || s == "--help");
    let is_version = args.iter().any(|s| s == "version" || s == "-v" || s == "--version");
    if is_help {
        println!("{}", usage());
        return Ok(())
    }
    if is_version {
        println!("{}", version());
        return Ok(())
    }
    let api = HidApi::new().map_err(|e| format!("failed to initialize hidapi: {e}"))?;
    let registry = load_registry();

    let Some(cmd) = args.first() else {
        return tui::start(api, registry);
    };

    let rest = &args[1..]; // args after the command name
    return perform_command(api, registry, cmd, rest);
}

fn perform_command(api: HidApi, registry: Registry, cmd: &String, rest: &[String]) -> Result<(), String> {
    let (pid, vals) = cmd::resolve_pid(&api, &registry, rest)?;
    let (device, def) = open_device(&api, &registry, pid)?;
    match cmd.as_str() {
        "list" => {
            cmd::cmd_list(&api, &registry);
            Ok(())
        }
        "info" => {
            cmd::cmd_info(&device, def)
        }
        "battery" => {
            let battery = cmd::cmd_battery(&device, def)?;
            let (level, charging) = (battery.level, battery.charging);
            println!("{}: battery {level}%{}", def.name, if charging { " (charging)" } else { "" });
            Ok(())
        }
        "dpi" => {
            if vals.is_empty() {
                let (x, y) = cmd::cmd_get_dpi(&device, def)?;
                println!("{}: DPI {x}x{y}", def.name);
            } else {
                let (x, y) = parse_dpi(vals)?;
                cmd::cmd_dpi(&device, &def, x, y)?;
                println!("{}: set DPI {x}x{y}", def.name);
            }
            Ok(())
        }
        "dpi-stages" => {
            let Some(active_index) = vals.first() else {
                let (active_stage, stages) = cmd::cmd_get_dpi_stages(&device, def)?;
                println!("active_stage:{} stages:{:?}", active_stage, stages);
                return Ok(());
            };
            let active = active_index.parse::<u8>()
                .map_err(|e| format!("invalid active index: {e}"))?;

            let values = to_vec_u16(&vals, 1)?;
            cmd::cmd_dpi_stages(&device, def, active, &values)
        }
        "color" => {
            let rgb = parse_rgb(&vals)?;
            let led = parse_led(vals.get(3))?;
            cmd::cmd_color(&device, &def, rgb, led)?;
            println!(
                "{}: set static color #{:02x}{:02x}{:02x} on LED {led:#04x}",
                def.name, rgb[0], rgb[1], rgb[2]
            );
            Ok(())
        }
        "effect" => {
            let Some(effect_name) = vals.first() else {
                return Err("effect requires <static|breathing|spectrum|wave|reactive|none>".to_owned())
            };
            let effect = Effect::parse(effect_name)?;

            let led = parse_led(vals.get(1))?;
            let rgb: Rgb = match vals.get(2..5) {
                Some(values) => parse_rgb(values)?,
                None => [255, 255, 255]
            };
            cmd::cmd_effect(&device, def, led, effect, rgb)
        }
        "brightness" => {
            let (brightness, led) = parse_brightness_args(&vals)?;
            match brightness {
                Some(value) => {
                    cmd::cmd_brightness(&device, def, value, led)?;
                    let mouse_name = &def.name;
                    let percentage = value as u32 * 100 / 255;
                    println!(
                        "{mouse_name}: set brightness {value}/255 ({percentage}%) on LED {led:#04x}",
                    );
                },
                None => {
                    let value = cmd::cmd_get_brightness(&device, def, led)?;
                    println!(
                        "{}: brightness {}/255 ({}%) on LED {led:#04x}",
                        def.name, value, value as usize * 100 / 255
                    );
                }
            }
            Ok(())
        }
        "polling" => {
            if vals.is_empty() {
                let hz = cmd::cmd_get_polling(&device, &def)?;
                println!("{}: polling at {hz} Hz", def.name);
            } else {
                let hz: u16 = vals[0].parse()
                    .map_err(|e| format!("invalid hz: {e}"))?;
                cmd::cmd_polling(&device, &def, hz)?;
                println!("{}: set polling rate {hz} Hz", def.name);
            }
            Ok(())
        }
        "profile" => {
            let sub = rest.first().map(|s| s.as_str()).unwrap_or("");
            let sub_args = &rest[1..];
            match sub {
                "save" => cmd::cmd_profile_save(&api, &registry, sub_args),
                "apply" => {
                    let name = sub_args.first().ok_or("profile apply requires <name>")?;
                    cmd::cmd_profile_apply(&api, &registry, name)
                }
                "list" => {
                    cmd::cmd_profile_list();
                    Ok(())
                }
                "show" => {
                    let name = sub_args.first().ok_or("profile show requires <name>")?;
                    cmd::cmd_profile_show(name)
                }
                "delete" => {
                    let name = sub_args.first().ok_or("profile delete requires <name>")?;
                    cmd::cmd_profile_delete(name)
                }
                "" | "help" => {
                    println!("profile subcommands: save, apply, list, show, delete");
                    Ok(())
                }
                other => Err(format!("unknown profile subcommand {other:?}")),
            }
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn to_vec_u16(vec: &Vec<String>, from: usize) -> Result<Vec<u16>, String> {
    let mut output = Vec::with_capacity(vec.len()-from);
    for i in from..vec.len() {
        let Ok(val) = vec[i].parse::<u16>() else {
            return Err(format!("invalid u16: {}", vec[i]));
        };
        output.push(val);
    }
    Ok(output)
}

fn parse_brightness_args(args: &Vec<String>) -> Result<(Option<u8>, u8), String> {
    let mut brightness = None;
    let mut led_str = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--led" => {
                i += 1;
                led_str = args.get(i);
            }
            _ => {
                let Ok(value) = args[i].parse::<u8>() else {
                    return Err("invalid brightness value. Usage: --brightness <0-255>".to_owned());
                };
                brightness = Some(value);
            }
        }
        i += 1;
    }
    let Ok(led) = parse_led(led_str) else {
        return Err("invalid LED value. Usage: --led <id>".to_owned());
    };
    Ok((brightness, led))
}

fn parse_dpi(vals: Vec<String>) -> Result<(u16, u16), String> {
    if vals.is_empty() {
        return Err("dpi requires <x>".to_string())
    }
    let x: u16 = vals[0].parse()
        .map_err(|e| format!("invalid x: {e}"))?;
    if vals.len() == 1 {
        return Ok((x, x))
    }
    let y: u16 = vals[1].parse()
        .map_err(|e| format!("invalid y: {e}"))?;
    Ok((x, y))
}

fn parse_rgb(vals: &[String]) -> Result<Rgb, String> {
    if vals.len() < 3 {
        return Err("rgb requires 3 channels".to_string())
    }
    let r = vals[0].parse::<u8>().map_err(|e| format!("invalid r channel: {e}"))?;
    let g = vals[1].parse::<u8>().map_err(|e| format!("invalid g channel: {e}"))?;
    let b = vals[2].parse::<u8>().map_err(|e| format!("invalid b channel: {e}"))?;
    Ok([r, g, b])
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn version() -> &'static str {
    return "v-0.1-DEV"
}

// =========================================================================
// Usage
// =========================================================================

fn usage() -> &'static str {
    "\
razer-win-cli — CLI for Razer device settings over USB HID

USAGE:
  razer-win-cli <command> [args...] [--pid <pid>]

  --pid <pid>   Optional. Accepts 0x005c, 005C, or 5c.
                When omitted, auto-detects the single connected Razer device.
                Required if multiple devices are connected.

DEVICE:
  list                                      Enumerate attached Razer devices + registry
  info                                      Show device details (serial, firmware, capabilities)
  battery                                   Read battery level + charging status

PERFORMANCE:
  dpi                                       Read current DPI
  dpi <x> [y]                               Set DPI (y defaults to x)
  dpi-stages <active> <v1> [<v2> ...]       Set 2-5 DPI stages; <active> is the 0-based index
  polling                                   Read polling rate
  polling <hz>                              Set polling rate (125/500/1000)

LIGHTING / RGB:
  color <r> <g> <b> [led]                   Set a static colour
  effect <effect> [led] [r g b]             Set lighting effect (static|breathing|spectrum|wave|reactive|none)
  brightness <0-255> [--led <LED>]          Set LED brightness
  brightness [--led <LED>]                  Read LED brightness


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

EXAMPLES:
  razer-win-cli list
  razer-win-cli dpi 1600                    # auto-detect, set DPI 1600x1600
  razer-win-cli dpi 1600 800 --pid 0x005C   # explicit PID
  razer-win-cli color 255 0 128             # pink logo LED
  razer-win-cli color 0 255 0 0x01          # green scroll LED
  razer-win-cli effect spectrum             # spectrum cycle
  razer-win-cli polling 1000
  razer-win-cli profile save Gaming --dpi 1600 1600 --effect static --rgb 255 0 128

NOTES:
  <led> is a hex LED id (default: 0x04 logo). Common: 0x01 scroll, 0x04 logo, 0x05 backlight.
  Profiles are stored as JSON in ~/.razer-win-cli/profiles/ (override with RAZER_CLI_PROFILES_DIR)."
}