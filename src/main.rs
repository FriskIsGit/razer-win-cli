use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use hidapi::HidApi;
use razer_hid::commands::dpi::DpiStage;
use razer_hid::commands::info::mode;
use razer_hid::commands::lighting::{led_id, Rgb};
use razer_hid::commands::{NOSTORE, VARSTORE};
use razer_hid::{Device, DeviceDef, Registry, TransportError, RAZER_VID};

// =========================================================================
// Profile model (mirrors the app's profiles.rs domain model)
// =========================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Effect {
    Static,
    Breathing,
    Spectrum,
    Wave,
    Reactive,
    #[serde(rename = "none")]
    Off,
}

impl Effect {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "static" => Ok(Effect::Static),
            "breathing" => Ok(Effect::Breathing),
            "spectrum" => Ok(Effect::Spectrum),
            "wave" => Ok(Effect::Wave),
            "reactive" => Ok(Effect::Reactive),
            "none" | "off" => Ok(Effect::Off),
            other => Err(format!(
                "unknown effect {other:?} (expected static, breathing, spectrum, wave, reactive, none)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LightingSettings {
    effect: Effect,
    color: Rgb,
    brightness: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DpiSettings {
    x: u16,
    y: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lighting: Option<LightingSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dpi: Option<DpiSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    polling_hz: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Profile {
    name: String,
    #[serde(default)]
    devices: std::collections::BTreeMap<u16, DeviceSettings>,
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
// Profile store
// =========================================================================

const PROFILES_PATH_ENV_VAR: &str = "RAZER_CLI_PROFILES_DIR";

fn profiles_dir() -> PathBuf {
    if let Some(dir) = env::var_os(PROFILES_PATH_ENV_VAR) {
        return PathBuf::from(dir);
    }
    if let Some(home) = get_home_directory() {
        return PathBuf::from(home).join(".razer-win-cli").join("profiles");
    }
    return PathBuf::from("profiles")
}

#[cfg(windows)]
fn get_home_directory() -> Option<OsString> {
    env::var_os("USERPROFILE")
}

#[cfg(not(windows))]
fn get_home_directory() -> Option<OsString> {
    env::var_os("HOME")
}

fn sanitize_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err("profile name must be 1-64 chars".to_string());
    }
    let ok = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_');
    if ok {
        Ok(trimmed.to_string())
    } else {
        Err("profile name contains invalid characters (allowed: alnum, space, -, _)".to_string())
    }
}

fn profile_path(name: &str) -> Result<PathBuf, String> {
    let safe = sanitize_name(name)?;
    Ok(profiles_dir().join(format!("{safe}.json")))
}

fn save_profile(profile: &Profile) -> Result<(), String> {
    let path = profile_path(&profile.name)?;
    std::fs::create_dir_all(path.parent().unwrap_or(&profiles_dir())).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write {}: {e}", path.display()))
}

fn load_profile(name: &str) -> Result<Profile, String> {
    let path = profile_path(name)?;
    let src = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&src).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn list_profiles() -> Vec<String> {
    let dir = profiles_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }
    }
    names.sort();
    names
}

fn delete_profile(name: &str) -> Result<(), String> {
    let path = profile_path(name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
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

fn parse_pid(raw: &str) -> Result<u16, String> {
    let trimmed = raw.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(trimmed, 16).map_err(|e| format!("invalid PID {raw:?}: {e}"))
}

fn parse_led(raw: Option<&str>) -> Result<u8, String> {
    match raw {
        Some(r) => {
            let trimmed = r.trim_start_matches("0x").trim_start_matches("0X");
            u8::from_str_radix(trimmed, 16).map_err(|e| format!("invalid LED id {r:?}: {e}"))
        }
        None => Ok(led_id::LOGO),
    }
}

fn led_id_for(def: &DeviceDef) -> u8 {
    def.led_regions.first().map(|r| r.id).unwrap_or(led_id::LOGO)
}

/// Auto-detect the single connected Razer device. Returns an error if zero or
/// more than one registry-known device is attached.
fn auto_detect_pid(api: &HidApi, registry: &Registry) -> Result<u16, String> {
    let mut pids: Vec<u16> = Vec::new();
    for info in api.device_list() {
        let product_id = info.product_id();
        if info.vendor_id() == RAZER_VID && registry.find_by_pid(product_id).is_some() {
            pids.push(product_id);
        }
    }
    pids.sort();
    pids.dedup();
    match pids.len() {
        0 => Err("no Razer device connected. Use --pid to specify one.".into()),
        1 => Ok(pids[0]),
        _ => {
            let list = pids.iter().map(|p| format!("{p:#06x}")).collect::<Vec<_>>().join(", ");
            Err(format!("multiple Razer devices connected ({list}). Use --pid to select one."))
        }
    }
}

/// Extract `--pid <value>` (or `-p <value>`) from the argument list.
/// Returns (resolved PID, remaining args with --pid removed).
/// If --pid is absent, auto-detects the connected device.
fn resolve_pid(
    api: &HidApi,
    registry: &Registry,
    args: &[String],
) -> Result<(u16, Vec<String>), String> {
    let mut pid: Option<u16> = None;
    let mut remaining: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--pid" || args[i] == "-p") && i + 1 < args.len() {
            pid = Some(parse_pid(&args[i + 1])?);
            i += 2;
        } else {
            remaining.push(args[i].clone());
            i += 1;
        }
    }
    let pid = match pid {
        Some(p) => p,
        None => auto_detect_pid(api, registry)?,
    };
    Ok((pid, remaining))
}

/// Open a device by PID, then put it in driver mode so it accepts config
/// commands. Best-effort on the mode switch — some devices don't need it.
fn open_device<'a>(api: &HidApi, registry: &'a Registry, pid: u16) -> Result<(Device, &'a DeviceDef), String> {
    let Some(definition) = registry.find_by_pid(pid) else {
        return Err(format!("PID {pid:#06x} is not in the device registry"));
    };

    let device = match Device::open(api, pid, definition.transaction_id, definition.wait_us) {
        Ok(d) => d,
        Err(e) => return Err(match e {
            TransportError::DeviceNotFound { .. } | TransportError::NoInterface { .. } => {
                format!("{} not found. Is it plugged in?", definition.name)
            }
            other => other.to_string(),
        })
    };

    // Put the device in driver mode (OpenRazer does this on daemon startup).
    // Without it some devices intermittently reject commands with 0x05.
    let _ = device.set_device_mode(mode::DRIVER);

    Ok((device, definition))
}

/// Retry a device operation up to `max_attempts` times with a delay between
/// attempts. The razer-hid transport already retries on BUSY internally (5×10ms),
/// but some devices need more recovery time for intermittent NOT_SUPPORTED or
/// BusyExhausted errors.
fn with_retry<T>(max_attempts: u32, label: &str, mut f: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut last_err = String::new();
    for attempt in 0..max_attempts {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = e;
                if attempt + 1 < max_attempts {
                    eprintln!("retrying {label} (attempt {}/{max_attempts}): {last_err}", attempt + 1);
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    Err(last_err)
}

/// Apply a DeviceSettings bundle to an open device. Lighting uses NOSTORE
/// (volatile); DPI uses VARSTORE (persistent — devices reject NOSTORE with
/// status 0x05 NOT_SUPPORTED).
fn apply_settings(device: &Device, def: &DeviceDef, settings: &DeviceSettings) -> Result<(), String> {
    let led = led_id_for(def);

    if def.capabilities.lighting {
        if let Some(l) = settings.lighting {
            match l.effect {
                Effect::Static => device.set_static_color(NOSTORE, led, l.color),
                Effect::Breathing => device.set_effect_breathing_single(NOSTORE, led, l.color),
                Effect::Spectrum => device.set_effect_spectrum(NOSTORE, led),
                Effect::Wave => device.set_effect_wave(NOSTORE, led, 0x01),
                Effect::Reactive => device.set_effect_reactive(NOSTORE, led, 0x02, l.color),
                Effect::Off => device.set_effect_none(NOSTORE, led),
            }
            .map_err(|e| format!("set lighting effect: {e}"))?;

            device.set_brightness(NOSTORE, led, l.brightness)
                .map_err(|e| format!("set brightness: {e}"))?;
        }
    }
    if def.capabilities.dpi {
        if let Some(d) = settings.dpi {
            device.set_dpi(VARSTORE, d.x, d.y)
                .map_err(|e| format!("set dpi: {e}"))?;
        }
    }
    if def.capabilities.polling_rate {
        if let Some(hz) = settings.polling_hz {
            device.set_polling_rate(hz).map_err(|e| format!("set polling rate: {e}"))?;
        }
    }
    Ok(())
}

// =========================================================================
// Command handlers
// =========================================================================

fn cmd_list(api: &HidApi, registry: &Registry) {
    let mut pids: BTreeSet<u16> = BTreeSet::new();
    for info in api.device_list() {
        if info.vendor_id() == RAZER_VID {
            pids.insert(info.product_id());
        }
    }

    if pids.is_empty() {
        println!("No Razer devices found (VID {RAZER_VID:#06x}).");
        println!();
        println!("Registry devices (not necessarily connected):");
        for def in registry.devices() {
            println!("  {:#06x}  {}", def.usb_pid, def.name);
        }
        return;
    }

    println!("{:10}  {:30}  Status", "PID", "Name");
    println!("{:-<60}", "");
    for pid in pids {
        match registry.find_by_pid(pid) {
            Some(def) => {
                let dev_type = match def.device_type {
                    razer_hid::DeviceType::Mouse => "mouse",
                    razer_hid::DeviceType::Keyboard => "keyboard",
                };
                let caps: Vec<&str> = [
                    def.capabilities.lighting.then_some("lighting"),
                    def.capabilities.dpi.then_some("dpi"),
                    def.capabilities.polling_rate.then_some("polling"),
                    def.capabilities.battery.then_some("battery"),
                ]
                .into_iter()
                .flatten()
                .collect();
                println!(
                    "{:<10}  {:<30}  {} [{}]",
                    format!("{pid:#06x}"),
                    def.name,
                    dev_type,
                    caps.join(", "),
                );
            }
            None => println!(
                "{:<10}  {:<30}  (unknown — not in registry)",
                format!("{pid:#06x}"),
                "?"
            ),
        }
    }
}

fn cmd_info(api: &HidApi, registry: &Registry, pid: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;

    println!("Device: {}", def.name);
    println!("  PID:            {:#06x}", def.usb_pid);
    println!("  Type:           {:?}", def.device_type);
    println!("  Transaction ID: {:#04x}", def.transaction_id);
    println!("  Wait:           {} us", def.wait_us);
    println!(
        "  DPI range:      {}-{}",
        def.dpi_min.map(|v| v.to_string()).unwrap_or("?".into()),
        def.dpi_max.map(|v| v.to_string()).unwrap_or("?".into()),
    );
    println!(
        "  Capabilities:   lighting={}, dpi={}, polling={}, battery={}",
        def.capabilities.lighting,
        def.capabilities.dpi,
        def.capabilities.polling_rate,
        def.capabilities.battery
    );
    println!(
        "  LED regions:    {}",
        if def.led_regions.is_empty() {
            "(none)".to_string()
        } else {
            def.led_regions
                .iter()
                .map(|r| format!("{} ({:#04x})", r.name, r.id))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    match device.get_serial() {
        Ok(s) => println!("  Serial:         {s}"),
        Err(e) => println!("  Serial:         (read failed: {e})"),
    }
    match device.get_firmware_version() {
        Ok((major, minor)) => println!("  Firmware:       v{major}.{minor}"),
        Err(e) => println!("  Firmware:       (read failed: {e})"),
    }
    Ok(())
}

fn cmd_dpi(api: &HidApi, registry: &Registry, pid: u16, x: u16, y: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.dpi {
        return Err(format!("{} does not support DPI", def.name));
    }
    with_retry(3, "set DPI", || {
        device.set_dpi(VARSTORE, x, y).map_err(|e| e.to_string())
    })?;
    println!("{}: set DPI {x}x{y}", def.name);
    Ok(())
}

fn cmd_get_dpi(api: &HidApi, registry: &Registry, pid: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.dpi {
        return Err(format!("{} does not support DPI", def.name));
    }
    let (x, y) = with_retry(3, "get DPI", || {
        device.get_dpi(VARSTORE).map_err(|e| e.to_string())
    })?;
    println!("{}: DPI {x}x{y}", def.name);
    Ok(())
}

fn cmd_dpi_stages(
    api: &HidApi,
    registry: &Registry,
    pid: u16,
    active: u8,
    values: &[u16],
) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.dpi {
        return Err(format!("{} does not support DPI", def.name));
    }
    if values.len() < 2 || values.len() > 5 {
        return Err("DPI stages: provide 2-5 values".to_string());
    }
    let stages: Vec<DpiStage> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| DpiStage { index: i as u8, dpi_x: v, dpi_y: v })
        .collect();
    with_retry(3, "set DPI stages", || {
        device.set_dpi_stages(VARSTORE, active, &stages).map_err(|e| e.to_string())
    })?;
    println!("{}: set {} DPI stage(s), active stage {active}", def.name, stages.len());
    for (i, s) in stages.iter().enumerate() {
        let marker = if i as u8 == active { " <== active" } else { "" };
        println!("  stage {}: {}x{}{}", s.index, s.dpi_x, s.dpi_y, marker);
    }
    Ok(())
}

fn cmd_color(api: &HidApi, registry: &Registry, pid: u16, rgb: Rgb, led: u8) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.lighting {
        return Err(format!("{} does not support lighting", def.name));
    }
    with_retry(3, "set color", || {
        device.set_static_color(NOSTORE, led, rgb).map_err(|e| e.to_string())
    })?;
    println!(
        "{}: set static color #{:02x}{:02x}{:02x} on LED {led:#04x}",
        def.name, rgb[0], rgb[1], rgb[2]
    );
    Ok(())
}

fn cmd_effect(
    api: &HidApi,
    registry: &Registry,
    pid: u16,
    effect: Effect,
    led: u8,
    rgb: Rgb,
) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.lighting {
        return Err(format!("{} does not support lighting", def.name));
    }
    with_retry(3, "set effect", || {
        match effect {
            Effect::Static => device.set_static_color(NOSTORE, led, rgb),
            Effect::Breathing => device.set_effect_breathing_single(NOSTORE, led, rgb),
            Effect::Spectrum => device.set_effect_spectrum(NOSTORE, led),
            Effect::Wave => device.set_effect_wave(NOSTORE, led, 0x01),
            Effect::Reactive => device.set_effect_reactive(NOSTORE, led, 0x02, rgb),
            Effect::Off => device.set_effect_none(NOSTORE, led),
        }
        .map_err(|e| e.to_string())
    })?;
    println!("{}: set effect {:?} on LED {led:#04x}", def.name, effect);
    Ok(())
}

fn cmd_brightness(api: &HidApi, registry: &Registry, pid: u16, value: u8, led: u8) -> Result<(), String> {
    let (device, definition) = open_device(api, registry, pid)?;
    if !definition.capabilities.lighting {
        return Err(format!("{} does not support lighting", definition.name));
    }
    with_retry(3, "set brightness", || {
        device.set_brightness(NOSTORE, led, value).map_err(|e| e.to_string())
    })?;
    let mouse_name = &definition.name;
    let percentage = value as u32 * 100 / 255;
    println!(
        "{mouse_name}: set brightness {value}/255 ({percentage}%) on LED {led:#04x}",
    );
    Ok(())
}

fn cmd_get_brightness(api: &HidApi, registry: &Registry, pid: u16, led: u8) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.lighting {
        return Err(format!("{} does not support lighting", def.name));
    }
    let value = with_retry(3, "get brightness", || {
        device.get_brightness(NOSTORE, led).map_err(|e| e.to_string())
    })?;
    println!(
        "{}: brightness {}/255 ({}%) on LED {led:#04x}",
        def.name, value, value * 100 / 255
    );
    Ok(())
}

fn cmd_polling(api: &HidApi, registry: &Registry, pid: u16, hz: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.polling_rate {
        return Err(format!("{} does not support polling rate", def.name));
    }
    with_retry(3, "set polling", || {
        device.set_polling_rate(hz).map_err(|e| e.to_string())
    })?;
    println!("{}: set polling rate {hz} Hz", def.name);
    Ok(())
}

fn cmd_get_polling(api: &HidApi, registry: &Registry, pid: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.polling_rate {
        return Err(format!("{} does not support polling rate", def.name));
    }
    match with_retry(3, "get polling", || {
        device.get_polling_rate().map_err(|e| e.to_string())
    })? {
        Some(hz) => println!("{}: polling rate {hz} Hz", def.name),
        None => println!("{}: polling rate unknown (unrecognized wire code)", def.name),
    }
    Ok(())
}

fn cmd_battery(api: &HidApi, registry: &Registry, pid: u16) -> Result<(), String> {
    let (device, def) = open_device(api, registry, pid)?;
    if !def.capabilities.battery {
        return Err(format!("{} does not support battery (wired device)", def.name));
    }
    let level = device.get_battery_level().map_err(|e| e.to_string())?;
    let charging = device.get_charging_status().map_err(|e| e.to_string())?;
    println!(
        "{}: battery {level}%{}",
        def.name,
        if charging { " (charging)" } else { "" }
    );
    Ok(())
}

// =========================================================================
// Profile commands
// =========================================================================

fn cmd_profile_save(api: &HidApi, registry: &Registry, args: &[String]) -> Result<(), String> {
    // profile save <name> [--pid <pid>] [--dpi x y] [--effect <e>] [--rgb r g b] [--brightness n] [--polling hz]
    if args.is_empty() {
        return Err("usage: profile save <name> [--pid <pid>] [--dpi x y] [--effect <e>] [--rgb r g b] [--brightness n] [--polling hz]".into());
    }
    let name = &args[0];
    let mut pid: Option<u16> = None;
    let mut settings = DeviceSettings::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pid" | "-p" => {
                if i + 1 >= args.len() {
                    return Err("--pid requires a value".into());
                }
                pid = Some(parse_pid(&args[i + 1])?);
                i += 2;
            }
            "--dpi" => {
                if i + 2 >= args.len() {
                    return Err("--dpi requires <x> <y>".into());
                }
                let x: u16 = args[i + 1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                let y: u16 = args[i + 2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                settings.dpi = Some(DpiSettings { x, y });
                i += 3;
            }
            "--effect" => {
                if i + 1 >= args.len() {
                    return Err("--effect requires <static|breathing|spectrum|wave|reactive|none>".into());
                }
                let effect = Effect::parse(&args[i + 1])?;
                if settings.lighting.is_none() {
                    settings.lighting = Some(LightingSettings { effect, color: [255, 255, 255], brightness: 255 });
                } else if let Some(ref mut l) = settings.lighting {
                    l.effect = effect;
                }
                i += 2;
            }
            "--rgb" => {
                if i + 3 >= args.len() {
                    return Err("--rgb requires <r> <g> <b>".into());
                }
                let r: u8 = args[i + 1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                let g: u8 = args[i + 2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                let b: u8 = args[i + 3].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                // update_lighting_color(&mut settings, [r,g,b]);
                if settings.lighting.is_none() {
                    settings.lighting = Some(LightingSettings { effect: Effect::Static, color: [r, g, b], brightness: 255 });
                } else if let Some(ref mut l) = settings.lighting {
                    l.color = [r, g, b];
                }
                i += 4;
            }
            "--brightness" => {
                if i + 1 >= args.len() {
                    return Err("--brightness requires <0-255>".into());
                }
                let v: u8 = args[i + 1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                if settings.lighting.is_none() {
                    settings.lighting = Some(LightingSettings { effect: Effect::Static, color: [255, 255, 255], brightness: v });
                } else if let Some(ref mut l) = settings.lighting {
                    l.brightness = v;
                }
                i += 2;
            }
            "--polling" => {
                if i + 1 >= args.len() {
                    return Err("--polling requires <hz>".into());
                }
                let hz: u16 = args[i + 1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                settings.polling_hz = Some(hz);
                i += 2;
            }
            other => return Err(format!("unknown flag {other:?}")),
        }
    }

    let pid = match pid {
        Some(p) => p,
        None => auto_detect_pid(api, registry)?,
    };

    let Some(definition) = registry.find_by_pid(pid) else {
        return Err(format!("PID {pid:#06x} not in registry"));
    };

    // Validate capabilities
    if settings.dpi.is_some() && !definition.capabilities.dpi {
        eprintln!("warning: {} does not support DPI; ignoring --dpi", definition.name);
        settings.dpi = None;
    }
    if settings.lighting.is_some() && !definition.capabilities.lighting {
        eprintln!("warning: {} does not support lighting; ignoring lighting flags", definition.name);
        settings.lighting = None;
    }
    if settings.polling_hz.is_some() && !definition.capabilities.polling_rate {
        eprintln!("warning: {} does not support polling rate; ignoring --polling", definition.name);
        settings.polling_hz = None;
    }

    let mut profile = Profile { name: sanitize_name(name)?, ..Default::default() };
    profile.devices.insert(pid, settings);
    save_profile(&profile)?;
    println!("Saved profile {:?} for {} ({pid:#06x})", profile.name, definition.name);
    Ok(())
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

fn cmd_profile_apply(api: &HidApi, registry: &Registry, name: &str) -> Result<(), String> {
    let profile = load_profile(name)?;
    for (pid, settings) in &profile.devices {
        match open_device(api, registry, *pid) {
            Ok((device, def)) => match apply_settings(&device, def, settings) {
                Ok(()) => println!("Applied {:?} to {} ({pid:#06x})", profile.name, def.name),
                Err(e) => eprintln!("Failed to apply to {pid:#06x}: {e}"),
            },
            Err(e) => eprintln!("Cannot open {pid:#06x}: {e}"),
        }
    }
    Ok(())
}

fn cmd_profile_list() {
    let names = list_profiles();
    if names.is_empty() {
        println!("No saved profiles (stored in {})", profiles_dir().display());
        return;
    }
    println!("Profiles ({}):", profiles_dir().display());
    for name in names {
        println!("  {name}");
    }
}

fn cmd_profile_show(name: &str) -> Result<(), String> {
    let profile = load_profile(name)?;
    let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

fn cmd_profile_delete(name: &str) -> Result<(), String> {
    delete_profile(name)?;
    println!("Deleted profile {name:?}");
    Ok(())
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

// =========================================================================
// Entry point
// =========================================================================

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        // TODO: Launch CLI interface if no argument is provided?
        println!("{}", usage());
        return Ok(());
    };

    let api = HidApi::new().map_err(|e| format!("failed to initialize hidapi: {e}"))?;
    let registry = load_registry();
    let rest = &args[1..]; // args after the command name
    return perform_command(api, registry, cmd, rest);
}

fn perform_command(api: HidApi, registry: Registry, cmd: &String, rest: &[String]) -> Result<(), String> {
    match cmd.as_str() {
        "list" => {
            cmd_list(&api, &registry);
            Ok(())
        }
        "info" => {
            let (pid, _) = resolve_pid(&api, &registry, rest)?;
            cmd_info(&api, &registry, pid)
        }
        "battery" => {
            let (pid, _) = resolve_pid(&api, &registry, rest)?;
            cmd_battery(&api, &registry, pid)
        }
        "dpi" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let x: u16 = vals
                .first()
                .ok_or("dpi requires <x>")?
                .parse()
                .map_err(|e: std::num::ParseIntError| format!("invalid x: {e}"))?;
            let y: u16 = match vals.get(1) {
                Some(raw) => raw
                    .parse()
                    .map_err(|e: std::num::ParseIntError| format!("invalid y: {e}"))?,
                None => x,
            };
            cmd_dpi(&api, &registry, pid, x, y)
        }
        "getdpi" => {
            let (pid, _) = resolve_pid(&api, &registry, rest)?;
            cmd_get_dpi(&api, &registry, pid)
        }
        "dpi-stages" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let active: u8 = vals
                .first()
                .ok_or("dpi-stages requires <active index>")?
                .parse()
                .map_err(|e: std::num::ParseIntError| format!("invalid active: {e}"))?;
            let values: Result<Vec<u16>, String> = vals[1..]
                .iter()
                .map(|v| v.parse::<u16>().map_err(|e| format!("invalid DPI value {v:?}: {e}")))
                .collect();
            cmd_dpi_stages(&api, &registry, pid, active, &values?)
        }
        "color" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let r = parse_rgb_component(&vals, 0, "r")?;
            let g = parse_rgb_component(&vals, 1, "g")?;
            let b = parse_rgb_component(&vals, 2, "b")?;
            let led = parse_led(vals.get(3).map(|x| x.as_str()))?;
            cmd_color(&api, &registry, pid, [r, g, b], led)
        }
        "effect" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let effect = Effect::parse(
                vals.first()
                    .ok_or("effect requires <static|breathing|spectrum|wave|reactive|none>")?,
            )?;
            let led = parse_led(vals.get(1).map(|x| x.as_str()))?;
            let rgb: Rgb = match (vals.get(2), vals.get(3), vals.get(4)) {
                (Some(r), Some(g), Some(b)) => [
                    r.parse().map_err(|e: std::num::ParseIntError| format!("invalid r: {e}"))?,
                    g.parse().map_err(|e: std::num::ParseIntError| format!("invalid g: {e}"))?,
                    b.parse().map_err(|e: std::num::ParseIntError| format!("invalid b: {e}"))?,
                ],
                _ => [255, 255, 255],
            };
            cmd_effect(&api, &registry, pid, effect, led, rgb)
        }
        "brightness" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let value: u8 = vals
                .first()
                .ok_or("brightness requires <0-255>")?
                .parse()
                .map_err(|e: std::num::ParseIntError| format!("invalid brightness: {e}"))?;
            let led = parse_led(vals.get(1).map(|x| x.as_str()))?;
            cmd_brightness(&api, &registry, pid, value, led)
        }
        "getbrightness" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let led = parse_led(vals.first().map(|x| x.as_str()))?;
            cmd_get_brightness(&api, &registry, pid, led)
        }
        "polling" => {
            let (pid, vals) = resolve_pid(&api, &registry, rest)?;
            let hz: u16 = vals
                .first()
                .ok_or("polling requires <hz>")?
                .parse()
                .map_err(|e: std::num::ParseIntError| format!("invalid hz: {e}"))?;
            cmd_polling(&api, &registry, pid, hz)
        }
        "getpolling" => {
            let (pid, _) = resolve_pid(&api, &registry, rest)?;
            cmd_get_polling(&api, &registry, pid)
        }
        "profile" => {
            let sub = rest.first().map(|s| s.as_str()).unwrap_or("");
            let sub_args = &rest[1..];
            match sub {
                "save" => cmd_profile_save(&api, &registry, sub_args),
                "apply" => {
                    let name = sub_args.first().ok_or("profile apply requires <name>")?;
                    cmd_profile_apply(&api, &registry, name)
                }
                "list" => {
                    cmd_profile_list();
                    Ok(())
                }
                "show" => {
                    let name = sub_args.first().ok_or("profile show requires <name>")?;
                    cmd_profile_show(name)
                }
                "delete" => {
                    let name = sub_args.first().ok_or("profile delete requires <name>")?;
                    cmd_profile_delete(name)
                }
                "" | "help" => {
                    println!("profile subcommands: save, apply, list, show, delete");
                    Ok(())
                }
                other => Err(format!("unknown profile subcommand {other:?}")),
            }
        }
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn parse_rgb_component(vals: &[String], index: usize, name: &str) -> Result<u8, String> {
    let str = match vals.get(index) {
        Some(s) => s,
        None => return Err(format!("color requires <{name}>")),
    };
    str.parse::<u8>().map_err(|e| format!("invalid {name}: {e}"))
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
