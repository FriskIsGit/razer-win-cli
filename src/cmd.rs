use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs::{create_dir_all, read_dir, read_to_string, remove_file, write};
use std::path::PathBuf;
use std::time::Duration;
use hidapi::HidApi;
use razer_hid::{Device, DeviceDef, Registry, TransportError, RAZER_VID};
use razer_hid::commands::dpi::DpiStage;
use razer_hid::commands::lighting::Rgb;
use razer_hid::commands::{NOSTORE, VARSTORE};
use razer_hid::commands::info::mode;
use crate::{ led_id_for, parse_hex_to_u16, DeviceEntry, DeviceSettings, DpiSettings, Effect, LightingSettings, Profile};

/// Auto-detect the single connected Razer device. Returns an error if zero or
/// more than one registry-known device is attached.
pub fn auto_detect_pid(api: &HidApi, registry: &Registry) -> Result<u16, String> {
    let (pids, razer_pid) = unique_razer_pids(api, registry);
    match pids.len() {
        0 => {
            if let Some(pid) = razer_pid {
                Err(format!("Razer device connected but PID {pid:#06x} is not supported."))
            } else {
                Err("no Razer device connected. See if any HID-compliant mice are visible in device manager".into())
            }
        },
        1 => Ok(pids[0]),
        _ => {
            let list = format_pid_list(&pids);
            Err(format!("multiple Razer devices connected ({list}). Use --pid to select one."))
        }
    }
}

/// Returns unique supported product IDs and the PID of the last Razer device found.
/// This PID is included for debugging purposes and may be unsupported.
fn unique_razer_pids(api: &HidApi, registry: &Registry) -> (Vec<u16>, Option<u16>) {
    let mut razer_pid = None;
    let mut unique_pids = HashSet::new();
    for info in api.device_list() {
        let product_id = info.product_id();
        if info.vendor_id() != RAZER_VID {
            continue
        }
        razer_pid = Some(product_id);
        if registry.find_by_pid(product_id).is_some() {
            unique_pids.insert(product_id);
        }
    }
    let pids: Vec<u16> = unique_pids.into_iter().collect();
    (pids, razer_pid)
}

fn format_pid_list(pids: &Vec<u16>) -> String {
    let mut list = String::new();
    let mut first = true;
    for p in pids {
        if !first {
            list.push_str(", ");
        }
        first = false;
        list.push_str(&format!("{p:#06x}"));
    }
    list
}

// =========================================================================
// Commands
// =========================================================================

pub fn cmd_list(api: &HidApi, registry: &Registry) {
    let (pids, razer_pid) = unique_razer_pids(api, registry);

    if pids.is_empty() {
        if let Some(pid) = razer_pid {
            println!("Razer device connected but PID {pid:#06x} is not supported.")
        } else {
            println!("no Razer device connected. See if any HID-compliant mice are visible in device manager")
        }
        println!("\nSupported registry devices:");
        for def in registry.devices() {
            println!("  {:#06x}  {}", def.usb_pid, def.name);
        }
        return;
    }

    println!("{:10}  {:30}  Status", "PID", "Name");
    println!("{:-<60}", "");
    for pid in pids {
        match registry.find_by_pid(pid) {
            Some(definition) => {
                let dev_type = match definition.device_type {
                    razer_hid::DeviceType::Mouse => "mouse",
                    razer_hid::DeviceType::Keyboard => "keyboard",
                };

                let capabilities = get_capabilities(definition);
                println!(
                    "{:<10}  {:<30}  {} [{}]",
                    format!("{pid:#06x}"),
                    definition.name,
                    dev_type,
                    capabilities.join(", "),
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

fn get_capabilities(def: &DeviceDef) -> Vec<&str> {
    let mut caps = Vec::new();
    if def.capabilities.lighting { caps.push("lighting"); }
    if def.capabilities.dpi { caps.push("dpi"); }
    if def.capabilities.polling_rate { caps.push("polling"); }
    if def.capabilities.battery { caps.push("battery"); }
    if def.capabilities.onboard { caps.push("onboard"); }
    caps
}

pub fn cmd_info(device: &Device, def: &DeviceDef) -> Result<(), String> {
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

pub fn cmd_dpi(device: &Device, def: &DeviceDef, x: u16, y: u16) -> Result<(), String> {
    if !def.capabilities.dpi {
        return Err(format!("{} does not support DPI", def.name));
    }
    with_retry(3, "set DPI", || {
        device.set_dpi(VARSTORE, x, y).map_err(|e| e.to_string())
    })?;
    Ok(())
}

pub fn cmd_get_dpi(device: &Device, definition: &DeviceDef) -> Result<(u16, u16), String> {
    if !definition.capabilities.dpi {
        return Err(format!("{} does not support DPI", definition.name));
    }
    let (x, y) = with_retry(3, "get DPI", || {
        device.get_dpi(VARSTORE).map_err(|e| e.to_string())
    })?;
    Ok((x, y))
}

pub fn cmd_get_dpi_stages(device: &Device, def: &DeviceDef) -> Result<(u8, Vec<DpiStage>), String> {
    if !def.capabilities.dpi {
        return Err(format!("{} does not support DPI", def.name));
    }
    with_retry(3, "get DPI stages", || {
        device.get_dpi_stages(VARSTORE).map_err(|e| e.to_string())
    }).map_err(|e| e.to_string())
}

pub fn cmd_dpi_stages(device: &Device, def: &DeviceDef, active: u8, values: &[u16]) -> Result<(), String> {
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
    println!("{:?}", stages);
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

pub fn cmd_color(device: &Device, def: &DeviceDef, rgb: Rgb, led: u8) -> Result<(), String> {
    if !def.capabilities.lighting {
        return Err(format!("{} does not support lighting", def.name));
    }
    with_retry(3, "set color", || {
        device.set_static_color(NOSTORE, led, rgb).map_err(|e| e.to_string())
    })
}

pub fn cmd_effect(
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

pub fn cmd_brightness(device: &Device, definition: &DeviceDef, value: u8, led: u8) -> Result<(), String> {
    if !definition.capabilities.lighting {
        return Err(format!("{} does not support lighting", definition.name));
    }
    with_retry(3, "set brightness", || {
        device.set_brightness(NOSTORE, led, value).map_err(|e| e.to_string())
    })
}

pub fn cmd_get_brightness(device: &Device, def: &DeviceDef, led: u8) -> Result<u8, String> {
    if !def.capabilities.lighting {
        return Err(format!("{} does not support lighting", def.name));
    }
    with_retry(3, "get brightness", || {
        device.get_brightness(NOSTORE, led).map_err(|e| e.to_string())
    })
}

pub fn cmd_polling(device: &Device, def: &DeviceDef, hz: u16) -> Result<(), String> {
    if !def.capabilities.polling_rate {
        return Err(format!("{} does not support polling rate", def.name));
    }
    with_retry(3, "set polling", || {
        device.set_polling_rate(hz).map_err(|e| e.to_string())
    })
}

pub fn cmd_get_polling(device: &Device, def: &DeviceDef) -> Result<u16, String> {
    if !def.capabilities.polling_rate {
        return Err(format!("{} does not support polling rate", def.name));
    }
    match with_retry(3, "get polling", || {
        device.get_polling_rate().map_err(|e| e.to_string())
    })? {
        Some(hz) => Ok(hz),
        None => Err(format!("{}: polling rate unknown (unrecognized wire code)", def.name)),
    }
}

pub struct Battery {
    pub level: u8,
    pub charging: bool,
}
pub fn cmd_battery(device: &Device, def: &DeviceDef) -> Result<Battery, String> {
    if !def.capabilities.battery {
        return Err(format!("{} does not support battery (wired device)", def.name));
    }
    let level = device.get_battery_level().map_err(|e| e.to_string())?;
    let charging = device.get_charging_status().map_err(|e| e.to_string())?;
    Ok(Battery { level, charging })
}

// =========================================================================
// Profile commands
// =========================================================================

pub fn cmd_profile_save(api: &HidApi, registry: &Registry, args: &[String]) -> Result<(), String> {
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
                pid = Some(parse_hex_to_u16(&args[i + 1])?);
                i += 2;
            }
            "--dpi" => {
                if i + 2 >= args.len() {
                    return Err("--dpi requires <x> <y>".into());
                }
                let x = args[i + 1].parse::<u16>().map_err(|e| e.to_string())?;
                let y = args[i + 2].parse::<u16>().map_err(|e| e.to_string())?;
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
                let r = args[i + 1].parse::<u8>().map_err(|e| e.to_string())?;
                let g = args[i + 2].parse::<u8>().map_err(|e| e.to_string())?;
                let b = args[i + 3].parse::<u8>().map_err(|e| e.to_string())?;
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
                let v = args[i + 1].parse::<u8>().map_err(|e| e.to_string())?;
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
                let hz = args[i + 1].parse::<u16>().map_err(|e| e.to_string())?;
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

    let profile_name = validate_profile_name(name)?;
    let mut profile = Profile { name: profile_name, ..Default::default() };
    let device_entry = DeviceEntry::new(pid, settings);
    profile.devices.push(device_entry);
    save_profile(&profile)?;
    println!("Saved profile {:?} for {} ({pid:#06x})", profile.name, definition.name);
    Ok(())
}

pub fn cmd_profile_apply(api: &HidApi, registry: &Registry, name: &str) -> Result<(), String> {
    let profile = load_profile(name)?;
    for device_entry in &profile.devices {
        let pid = device_entry.id;
        match open_device(api, registry, pid) {
            Ok((device, def)) => match apply_settings(&device, def, &device_entry.settings) {
                Ok(()) => println!("Applied {:?} to {} ({pid:#06x})", profile.name, def.name),
                Err(e) => eprintln!("Failed to apply to {pid:#06x}: {e}"),
            },
            Err(e) => eprintln!("Cannot open {pid:#06x}: {e}"),
        }
    }
    Ok(())
}

pub fn cmd_profile_list() {
    let names = list_profiles();
    let dir = profiles_dir();
    if names.is_empty() {
        println!("No saved profiles (stored in {})", dir.display());
        return;
    }
    println!("Profiles ({}):", dir.display());
    for name in names {
        println!("  {name}");
    }
}

pub fn cmd_profile_show(name: &str) -> Result<(), String> {
    let profile = load_profile(name)?;
    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

pub fn cmd_profile_delete(name: &str) -> Result<(), String> {
    delete_profile(name)?;
    println!("Deleted profile {name:?}");
    Ok(())
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

fn profile_path(name: &str) -> Result<PathBuf, String> {
    let safe_name = validate_profile_name(name)?;
    let json_name = format!("{safe_name}.json");
    Ok(profiles_dir().join(json_name))
}

fn save_profile(profile: &Profile) -> Result<(), String> {
    create_dir_all(profiles_dir())
        .map_err(|e| e.to_string())?;

    let path = profile_path(&profile.name)?;
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| e.to_string())?;
    write(&path, json)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn load_profile(name: &str) -> Result<Profile, String> {
    let path = profile_path(name)?;

    let src = read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&src)
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

fn list_profiles() -> Vec<String> {
    let dir = profiles_dir();
    let Ok(entries) = read_dir(&dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension() else {
            continue
        };
        if ext != "json" {
            continue
        }
        // Convert to valid UTF-8, can fail if a filename has invalid encoding
        let Some(stem_os) = path.file_stem() else {
            continue
        };
        if let Some(stem) = stem_os.to_str() {
            names.push(stem.to_string())
        };
    }
    names.sort();
    names
}

fn delete_profile(name: &str) -> Result<(), String> {
    let path = profile_path(name)?;
    match remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn validate_profile_name(name: &str) -> Result<String, String> {
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

/// Extract `--pid <value>` (or `-p <value>`) from the argument list.
/// Returns (resolved PID, remaining args with --pid removed).
/// If --pid is absent, auto-detects the connected device.
pub fn resolve_pid(
    api: &HidApi,
    registry: &Registry,
    args: &[String],
) -> Result<(u16, Vec<String>), String> {
    let mut pid: Option<u16> = None;
    let mut remaining: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--pid" || args[i] == "-p") && i + 1 < args.len() {
            pid = Some(parse_hex_to_u16(&args[i + 1])?);
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
pub fn open_device<'a>(api: &HidApi, registry: &'a Registry, pid: u16) -> Result<(Device, &'a DeviceDef), String> {
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