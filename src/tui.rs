use std::cmp::{max, PartialEq};
use hidapi::HidApi;
use razer_hid::{Device, DeviceDef, LedRegion, Registry};
use razer_hid::commands::lighting::Rgb;
use crate::{cmd, to_brightness_percentage};
use crate::cmd::{
    apply_profile, cmd_dpi, cmd_get_dpi, cmd_get_polling, cmd_polling, delete_profile,
    list_profiles, load_profile, save_profile, validate_profile_name,
};
use crate::inputs::KeyCode;
use crate::{DpiSettings, Effect, Profile, ProfileSettings, ZoneSettings};

const TOP_LEFT: char = '┌';
const TOP_RIGHT: char = '┐';
const DOWN_RIGHT: char = '┘';
const DOWN_LEFT: char = '└';
const LEFT_T: char = '├';
const RIGHT_T: char = '┤';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';
const TOTAL_WIDTH: usize = 48;
const DPI_STEP: u16 = 50;

const DPI_X_INDEX: usize = 0;
const DPI_Y_INDEX: usize = 1;
const POLLING_INDEX: usize = 2;
const LIGHTING_INDEX: usize = 3;
const PROFILES_INDEX: usize = 4;

const POLLING_TABLE: [u16; 3] = [125, 500, 1000];

const LIGHTING_ROW_LED: usize = 0;
const LIGHTING_ROW_EFFECT: usize = 1;
const LIGHTING_ROW_COLOR_R: usize = 2;
const LIGHTING_ROW_COLOR_G: usize = 3;
const LIGHTING_ROW_COLOR_B: usize = 4;
const LIGHTING_ROW_BRIGHTNESS: usize = 5;
const LIGHTING_ROW_SPEED: usize = 6;
const LIGHTING_ROW_LINK_ZONES: usize = 7;

const LIGHTING_ROWS: usize = 8;
const LIGHTING_STEP: i8 = 16;

const DEFAULT_COLOR: Rgb = [0, 255, 0];
const DEFAULT_SPEED: u8 = 2;

fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::None => "Off",
        Effect::Static => "Static",
        Effect::Breathing => "Breathing",
        Effect::Spectrum => "Spectrum",
        Effect::Wave => "Wave",
        Effect::Reactive => "Reactive",
        Effect::Starlight => "Starlight",
        Effect::Custom => "custom",
    }
}

pub fn start(api: HidApi, registry: Registry) -> Result<(), String> {
    let pid = cmd::auto_detect_pid(&api, &registry)?;

    let (device, definition) = cmd::open_device(&api, &registry, pid)?;

    let (mut x, mut y) = cmd_get_dpi(&device, definition).unwrap_or_default();
    let mut polling = cmd_get_polling(&device, definition).unwrap_or_default();
    let mut polling_index = POLLING_TABLE.iter().position(|p| *p == polling).unwrap_or_default();
    let mut lighting_state = LightingState::read_initial(&device, definition);

    let mut menu_items = build_menu_items(definition, x, y, polling);

    let mut edit_mode = false;
    let mut index = first_index(&menu_items);
    let mut buffer = String::with_capacity(1500);
    loop {
        crate::inputs::clear_console();

        draw_ui(&mut buffer, definition, index, &menu_items, edit_mode);

        // println!("BUFFER LENGTH: {}", buffer.len());
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                break;
            }
            KeyCode::ArrowUp | KeyCode::Char('w') | KeyCode::Char('W') => {
                index = next_row_index_up(&menu_items, index);
            }
            KeyCode::ArrowLeft | KeyCode::Char('a') | KeyCode::Char('A') => {
                match index {
                    DPI_X_INDEX | DPI_Y_INDEX => {
                        let dpi_value = match index {
                            DPI_X_INDEX => &mut x,
                            DPI_Y_INDEX => &mut y,
                            _ => unreachable!("matched earlier"),
                        };
                        let min_dpi = definition.dpi_min.unwrap_or(100);
                        if *dpi_value < DPI_STEP {
                            continue;
                        }
                        if *dpi_value - DPI_STEP < min_dpi {
                            *dpi_value = min_dpi;
                        } else {
                            *dpi_value -= DPI_STEP;
                        }
                        let new_value = *dpi_value;
                        let _ = cmd_dpi(&device, definition, x, y);
                        menu_items[index].value = new_value.to_string()
                    }
                    POLLING_INDEX => {
                        if polling_index == 0 {
                            continue
                        }
                        polling_index -= 1;
                        let new_polling = POLLING_TABLE[polling_index];
                        let _ = cmd_polling(&device, definition, new_polling);
                        menu_items[index].value = new_polling.to_string() + " Hz";
                    }
                    _  => {}
                }

            }
            KeyCode::ArrowRight | KeyCode::Char('d') | KeyCode::Char('D') => {
                match index {
                    DPI_X_INDEX | DPI_Y_INDEX => {
                        let dpi_value = match index {
                            DPI_X_INDEX => &mut x,
                            DPI_Y_INDEX => &mut y,
                            _ => unreachable!("matched earlier"),
                        };
                        let max_dpi = definition.dpi_max.unwrap_or(u16::MAX as u32) as u16;
                        if *dpi_value + DPI_STEP > max_dpi {
                            continue;
                        }
                        *dpi_value += DPI_STEP;
                        let new_value = *dpi_value;
                        let _ = cmd_dpi(&device, definition, x, y);
                        menu_items[index].value = new_value.to_string();
                    }
                    POLLING_INDEX => {
                        if polling_index >= POLLING_TABLE.len() - 1 {
                            continue
                        }
                        polling_index += 1;
                        let new_polling = POLLING_TABLE[polling_index];
                        let _ = cmd_polling(&device, definition, new_polling);
                        menu_items[index].value = new_polling.to_string() + " Hz";
                    }
                    _ => {}
                }
            }

            KeyCode::ArrowDown | KeyCode::Char('s') | KeyCode::Char('S') => {
                index = next_row_index_down(&menu_items, index);
            }
            KeyCode::Enter => {
                match index {
                    LIGHTING_INDEX => {
                        start_lighting_menu(&device, definition, &mut lighting_state, &mut buffer);
                    },
                    PROFILES_INDEX => {
                        start_profiles_menu(&device, definition, &mut buffer, &mut lighting_state);
                        (x, y) = cmd_get_dpi(&device, definition).unwrap_or_default();
                        polling = cmd_get_polling(&device, definition).unwrap_or_default();
                        menu_items[DPI_X_INDEX].value = x.to_string();
                        menu_items[DPI_Y_INDEX].value = y.to_string();
                        menu_items[POLLING_INDEX].value = polling.to_string();
                    },
                    _ => {
                        if edit_mode {

                        }
                        edit_mode = !edit_mode;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn build_menu_items(definition: &DeviceDef, x: u16, y: u16, polling: u16) -> [UiRow; 5] {
    let mut show = definition.capabilities.dpi;
    let dpi_x = UiRow::new_with_visibility("DPI X", x.to_string(), show);
    let dpi_y = UiRow::new_with_visibility("DPI Y", y.to_string(), show);

    show = definition.capabilities.polling_rate;
    let polling_row = UiRow::new_with_visibility("POLLING (RATE)", polling.to_string() + " Hz", show);

    show = definition.capabilities.lighting;
    let lighting = UiRow::new_with_visibility("LIGHTING", ">".into(), show);
    let profiles = UiRow::new("PROFILES", ">".into());
    return [dpi_x, dpi_y, polling_row, lighting, profiles];
}

fn first_index(rows: &[UiRow]) -> usize {
    for (i, item) in rows.iter().enumerate() {
        if item.visible {
            return i
        }
    }
    0
}

fn next_row_index_up(rows: &[UiRow], index: usize) -> usize {
    let len = rows.len();
    for offset in 1..=len {
        let candidate = (index + len - offset) % len;
        if rows[candidate].visible { return candidate; }
    }
    index
}

fn next_row_index_down(rows: &[UiRow], index: usize) -> usize {
    let len = rows.len();
    for offset in index+1..len {
        if rows[offset].visible { return offset; }
    }
    index
}

fn draw_ui(buffer: &mut String, definition: &DeviceDef, index: usize, menu_items: &[UiRow; 5], edit_mode: bool) {
    draw_top(buffer);
    box_content(buffer, &definition.name);
    draw_separator(buffer);
    draw_options(buffer, index, menu_items, edit_mode);
    draw_separator(buffer);
    draw_main_menu_navigation(buffer);
    draw_bottom(buffer);
}

fn start_profiles_menu(
    device: &Device,
    definition: &DeviceDef,
    buffer: &mut String,
    lighting_state: &mut LightingState,
) {
    let mut profiles = load_profile_entries();
    let mut index = 0;
    let mut status: Option<String> = None;

    loop {
        crate::inputs::clear_console();

        draw_profiles_ui(buffer, &profiles, index, &status);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace | KeyCode::Escape | KeyCode::Char('q') | KeyCode::Char('Q') => {
                break;
            }
            KeyCode::ArrowUp | KeyCode::Char('w') | KeyCode::Char('W') => {
                if index > 0 {
                    index -= 1;
                }
            }
            KeyCode::ArrowDown | KeyCode::Char('s') | KeyCode::Char('S') => {
                if index + 1 < profiles.len() {
                    index += 1;
                }
            }
            KeyCode::Enter => {
                if profiles.is_empty() {
                    // Status is already displayed to the user
                    continue;
                }
                let name = &profiles[index].name;
                match apply_profile(device, definition, name) {
                    Ok(profile) => {
                        status = Some(format!("applied {name}"));
                        sync_lighting_from_profile(definition, lighting_state, &profile.settings);
                    }
                    Err(e) => status = Some(format!("failed to apply {name}: {e}")),
                }
            }
            KeyCode::Delete => {
                if profiles.is_empty() {
                    continue;
                }
                let name = profiles[index].name.clone();
                if !confirm_profile_deletion(buffer, &name) {
                    continue;
                }
                match delete_profile(&name) {
                    Ok(()) => {
                        status = Some(format!("deleted {name}"));
                        profiles = load_profile_entries();
                        if index >= profiles.len() && index > 0 {
                            index -= 1;
                        }
                    }
                    Err(e) => status = Some(format!("failed to delete {name}: {e}")),
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let Some(name) = prompt_save_profile_name(buffer) else {
                    continue;
                };
                let (x, y) = cmd_get_dpi(device, definition).unwrap_or_default();
                let polling = cmd_get_polling(device, definition).unwrap_or_default();
                let settings = profile_settings_from(definition, x, y, polling, lighting_state);
                let profile = Profile { name, settings };
                match save_profile(&profile) {
                    Ok(()) => {
                        status = Some(format!("saved {}", profile.name));
                        profiles = load_profile_entries();
                        if let Some(pos) = profiles.iter().position(|e| e.name == profile.name) {
                            index = pos;
                        }
                    }
                    Err(e) => status = Some(format!("failed to save {}: {e}", profile.name)),
                }
            }
            _ => {}
        }
    }
}

/// Prompt for a profile name to save the current settings under.
/// Returns optional profile name
fn prompt_save_profile_name(buffer: &mut String) -> Option<String> {
    let mut input = String::new();
    let mut error: Option<String> = None;

    loop {
        crate::inputs::clear_console();

        draw_save_prompt_ui(buffer, &input, &error);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Char(c) => {
                if is_valid_name_char(c) && input.len() < 64 {
                    input.push(c);
                }
            }
            KeyCode::Space => {
                if input.len() < 64 {
                    input.push(' ');
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Escape => return None,
            KeyCode::Enter => {
                match validate_profile_name(&input) {
                    Ok(name) => return Some(name),
                    Err(e) => error = Some(e),
                }
            }
            _ => {}
        }
    }
}

fn is_valid_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn confirm_profile_deletion(buffer: &mut String, name: &str) -> bool {
    loop {
        crate::inputs::clear_console();

        draw_confirm_delete_ui(buffer, name);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Char('y') | KeyCode::Char('Y') => return true,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Escape => return false,
            _ => {}
        }
    }
}

fn zone_settings_from(zone: &ZoneState) -> ZoneSettings {
    ZoneSettings {
        led_id: zone.region.id,
        effect: zone.effect,
        color: zone.color,
        brightness: zone.brightness,
        speed: zone.speed,
    }
}

/// Build the profile settings from current device state.
fn profile_settings_from(
    definition: &DeviceDef,
    x: u16,
    y: u16,
    polling: u16,
    lighting_state: &LightingState,
) -> ProfileSettings {
    let mut settings = ProfileSettings::default();

    if definition.capabilities.dpi {
        settings.dpi = Some(DpiSettings { x, y });
    }
    if definition.capabilities.polling_rate {
        settings.polling_hz = Some(polling);
    }

    if lighting_state.zones.is_empty() {
        return settings;
    }
    if lighting_state.link_zones {
        settings.global_zone = Some(zone_settings_from(&lighting_state.zones[0]));
    } else {
        for zone in &lighting_state.zones {
            settings.lighting_zones.push(zone_settings_from(zone));
        }
    }
    settings
}

/// Update the TUI's tracked lighting state after a profile was applied.
/// Lighting settings cannot be read back from the device.
fn sync_lighting_from_profile(
    definition: &DeviceDef,
    lighting_state: &mut LightingState,
    settings: &ProfileSettings,
) {
    if !definition.capabilities.lighting {
        return;
    }

    match settings.global_zone {
        Some(global) => {
            for zone in &mut lighting_state.zones {
                zone.effect = global.effect;
                zone.color = global.color;
                zone.brightness = global.brightness;
                zone.speed = global.speed;
            }
        }
        None => {
            for zone_settings in &settings.lighting_zones {
                let Some(zone) = lighting_state.zones.iter_mut().find(|z| z.region.id == zone_settings.led_id) else {
                    continue;
                };
                zone.effect = zone_settings.effect;
                zone.color = zone_settings.color;
                zone.brightness = zone_settings.brightness;
                zone.speed = zone_settings.speed;
            }
        }
    }
}

fn start_lighting_menu(device: &Device, definition: &DeviceDef, state: &mut LightingState, buffer: &mut String) {
    let mut index = 0;
    let mut status: Option<String> = None;

    loop {
        crate::inputs::clear_console();

        draw_lighting_ui(buffer, index, state, &status);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace | KeyCode::Escape | KeyCode::Char('q') | KeyCode::Char('Q') => {
                break;
            }
            KeyCode::ArrowUp | KeyCode::Char('w') | KeyCode::Char('W') => {
                if index > 0 {
                    index -= 1;
                }
            }
            KeyCode::ArrowDown | KeyCode::Char('s') | KeyCode::Char('S') => {
                if index + 1 < LIGHTING_ROWS {
                    index += 1;
                }
            }
            KeyCode::ArrowLeft | KeyCode::Char('a') | KeyCode::Char('A') => {
                adjust_lighting(device, definition, index, -1, state, &mut status);
            }
            KeyCode::ArrowRight | KeyCode::Char('d') | KeyCode::Char('D') => {
                adjust_lighting(device, definition, index, 1, state, &mut status);
            }
            _ => {}
        }
    }
}

#[derive(PartialEq)]
struct ZoneState {
    effect: Effect,
    color: [u8; 3],
    brightness: u8,
    speed: u8,
    region: LedRegion,
}

#[derive(PartialEq)]
struct LightingState {
    zones: Vec<ZoneState>,
    selected_zone: usize,
    link_zones: bool,
}

impl LightingState {
    /// Defaults to static white at full brightness.
    /// The device's current brightness is read if available (there is no get command for effect or color).
    fn read_initial(device: &Device, definition: &DeviceDef) -> Self {
        if !definition.capabilities.lighting {
            return Self { zones: vec![], selected_zone: 0, link_zones: false }
        }

        let mut state = Self {
            zones: Vec::with_capacity(definition.led_regions.len()),
            selected_zone: 0,
            link_zones: true
        };
        for led_region in &definition.led_regions {
            let current_brightness = cmd::cmd_get_brightness(device, definition, led_region.id);
            let zone = ZoneState {
                effect: Effect::Static,
                color: DEFAULT_COLOR,
                brightness: current_brightness.unwrap_or(255),
                speed: DEFAULT_SPEED,
                region: led_region.clone()
            };
            state.zones.push(zone);
        }
        state
    }
}

/// Step the selected lighting row by one step. Return bool indicating if the state was altered.
fn step_lighting_row(index: usize, signum: i8, state: &mut LightingState, definition: &DeviceDef) -> bool {
    let effects = &definition.lighting_effects;

    let (zone_index, _) = select_zone_index_with_label(state);

    let modified = match index {
        LIGHTING_ROW_LED => {
            if let Some(next_index) = next_zone_index(signum, state) {
                state.selected_zone = next_index;
                true
            } else {
                false
            }
        }
        LIGHTING_ROW_EFFECT => {
            let zone = &mut state.zones[zone_index];
            let current = effect_index(effects, zone.effect) as isize;
            let mut next_index = current + signum as isize;
            if next_index < 0 || next_index >= effects.len() as isize {
                next_index = current;
            }
            zone.effect = effects[next_index as usize];
            true
        }
        LIGHTING_ROW_COLOR_R | LIGHTING_ROW_COLOR_G | LIGHTING_ROW_COLOR_B => {
            let channel = match index {
                LIGHTING_ROW_COLOR_R => 0,
                LIGHTING_ROW_COLOR_G => 1,
                LIGHTING_ROW_COLOR_B => 2,
                _ => unreachable!("matched earlier")
            };
            let zone = &mut state.zones[zone_index];
            zone.color[channel] = step_u8(zone.color[channel], signum * LIGHTING_STEP);
            true
        }
        LIGHTING_ROW_BRIGHTNESS => {
            let zone = &mut state.zones[zone_index];
            zone.brightness = step_u8(zone.brightness, signum * LIGHTING_STEP);
            true
        },
        LIGHTING_ROW_SPEED => {
            let zone = &mut state.zones[zone_index];
            zone.speed = step_u8_clamp(zone.speed, signum, 1, 4);
            true
        },
        LIGHTING_ROW_LINK_ZONES => {
            state.link_zones = !state.link_zones;
            true
        },
        _ => unreachable!("index is always within lighting rows")
    };
    let zone = &state.zones[zone_index];
    let (brightness, color, effect, speed) = (zone.brightness, zone.color, zone.effect, zone.speed);

    // Keep other zones in sync if they're linked
    if modified && state.link_zones {
        for a_zone in state.zones.iter_mut().skip(1) {
            a_zone.brightness = brightness;
            a_zone.color = color;
            a_zone.effect = effect;
            a_zone.speed = speed;
        }
    }
    return modified;
}

fn next_zone_index(signum: i8, state: &LightingState) -> Option<usize> {
    let zone_index = state.selected_zone;
    if state.link_zones ||
        signum == -1 && zone_index == 0 ||
        signum == 1 && zone_index + 1 == state.zones.len() {
        None
    } else {
        Some((zone_index as i8 + signum) as usize)
    }
}

fn effect_index(effects: &Vec<Effect>, effect: Effect) -> usize {
    effects.iter().position(|e| *e == effect).unwrap_or(0)
}

fn step_u8(value: u8, delta: i8) -> u8 {
    (value as i16).saturating_add(delta as i16).clamp(0, 255) as u8
}

fn step_u8_clamp(value: u8, delta: i8, min: i16, max: i16) -> u8 {
    (value as i16).saturating_add(delta as i16).clamp(min, max) as u8
}

/// Step the selected lighting row and send the result to the device.
/// The local state is only committed on success;
/// on failure the old state is kept and the error is recorded for display.
fn adjust_lighting(
    device: &Device,
    definition: &DeviceDef,
    index: usize,
    signum: i8,
    state: &mut LightingState,
    status: &mut Option<String>,
) {
    if !step_lighting_row(index, signum, state, &definition) {
        return;
    }
    let (zone_index, _) = select_zone_index_with_label(state);
    let led_ids;
    let zone = &state.zones[zone_index];
    if state.link_zones {
        led_ids = state.zones.iter().map(|z| z.region.id).collect();
    } else {
        led_ids = vec![zone.region.id];
    }
    for led in led_ids {
        if let Err(e) = cmd::set_effect(device, led , zone.effect, zone.color, zone.speed) {
            *status = Some(format!("set effect failed: {e}"));
            continue;
        }
        if let Err(e) = cmd::set_brightness(device, led, zone.brightness) {
            *status = Some(format!("set brightness failed: {e}"));
            continue;
        }
        *status = None;
    }
}

fn draw_lighting_ui(buffer: &mut String, index: usize, state: &LightingState, status: &Option<String>) {
    draw_top(buffer);
    box_and_side_pad(buffer, "< BACK", "LIGHTING");
    draw_separator(buffer);
    let (zone_index, _) = select_zone_index_with_label(state);
    draw_lighting_color_bar(buffer, state.zones[zone_index].color);
    draw_separator(buffer);
    draw_lighting_options(buffer, index, state);
    draw_separator(buffer);
    if let Some(msg) = status {
        box_content(buffer, msg);
        draw_separator(buffer);
    }
    box_content(buffer, "[↑/↓] Navigate  [←/→] Change  [Bksp] Back");
    draw_bottom(buffer);
}

struct UiRow {
    pub name: &'static str,
    pub value: String,
    pub visible: bool,
}

impl UiRow {
    fn new(name: &'static str, value: String) -> Self {
        Self { name, value, visible: true }
    }
    fn new_with_visibility(name: &'static str, value: String, visible: bool) -> Self {
        Self { name, value, visible }
    }
}

fn select_zone_index_with_label(state: &LightingState) -> (usize, String) {
    let index;
    let label;
    if state.link_zones {
        index = 0;
        label = "ALL ZONES".into();
    } else if state.selected_zone < state.zones.len() {
        index = state.selected_zone;
        label = state.zones[index].region.name.to_string()
    } else {
        panic!("Invalid state, selected_zone is out of bounds")
    };
    return (index, label);
}

fn draw_lighting_options(s: &mut String, index: usize, state: &LightingState) {
    if state.zones.len() == 0 {
        eprintln!("Lighting options shouldn't be drawn if there are no zones");
        return
    }

    let (zone_index, zone_label) = select_zone_index_with_label(state);
    let zone = &state.zones[zone_index];

    let rows: [UiRow; LIGHTING_ROWS] = [
        UiRow::new("LED", zone_label),
        UiRow::new("EFFECT", effect_name(zone.effect).to_string()),
        UiRow::new("COLOR R", zone.color[0].to_string()),
        UiRow::new("COLOR G", zone.color[1].to_string()),
        UiRow::new("COLOR B", zone.color[2].to_string()),
        UiRow::new("BRIGHTNESS", to_brightness_percentage(zone.brightness)),
        UiRow::new("SPEED", zone.speed.to_string()),
        UiRow::new("LINK ZONES", state.link_zones.to_string()),
    ];
    // Find widest item
    let mut max_length = 0;
    for ui_row in &rows {
        max_length = max(ui_row.name.len(), max_length);
    }
    let mut content = String::new();
    for (i, ui_row) in rows.iter().enumerate() {
        if i == index {
            content.push_str(" > ");
        } else {
            content.push_str("   ");
        }

        let (name, value) = (ui_row.name, &ui_row.value);
        content.push_str(&format!("{:<max_length$}", name));
        // add 4 separators for visibility
        content.push_str("    ");
        content.push_str(&value);
        box_content(s, &content);
        content.clear();
    }
}

fn draw_lighting_color_bar(s: &mut String, color: [u8; 3]) {
    let label = "Color: ";
    let rgb = format!(" RGB({}, {}, {})", color[0], color[1], color[2]);
    let bar_width = TOTAL_WIDTH - 3 - label.len() - rgb.len();
    let bar = " ".repeat(bar_width);
    let bg = format!("\x1b[48;2;{};{};{}m", color[0], color[1], color[2]);
    let reset = "\x1b[0m";
    s.push(VERTICAL);
    s.push(' ');
    s.push_str(label);
    s.push_str(&bg);
    s.push_str(&bar);
    s.push_str(reset);
    s.push_str(&rgb);
    s.push(VERTICAL);
    s.push('\n');
}

struct ProfileEntry {
    name: String,
    summary: String,
}

fn load_profile_entries() -> Vec<ProfileEntry> {
    let mut entries = Vec::new();
    for name in list_profiles() {
        match load_profile(&name) {
            Ok(profile) => entries.push(ProfileEntry {
                name,
                summary: profile_summary_for(&profile),
            }),
            Err(e) => entries.push(ProfileEntry {
                name,
                summary: format!("unreadable: {e}"),
            }),
        }
    }
    entries
}

fn profile_summary_for(profile: &Profile) -> String {
    let settings = &profile.settings;
    let mut parts: Vec<String> = Vec::new();
    if let Some(dpi) = settings.dpi {
        parts.push(format!("DPI {}x{}", dpi.x, dpi.y));
    }
    if let Some(zone) = settings.global_zone {
        let percentage = zone.brightness as usize * 100 / 255;
        parts.push(format!("{} {percentage}%", effect_name(zone.effect)));
    }
    if let Some(hz) = settings.polling_hz {
        parts.push(format!("{hz} Hz"));
    }
    if parts.is_empty() {
        "(no settings)".to_string()
    } else {
        parts.join("  ")
    }
}

fn draw_profiles_ui(buffer: &mut String, profiles: &[ProfileEntry], index: usize, status: &Option<String>) {
    draw_top(buffer);
    box_and_side_pad(buffer, "< BACK", "PROFILES");
    draw_separator(buffer);
    if profiles.is_empty() {
        box_content(buffer, "(no saved profiles)");
    } else {
        for (i, entry) in profiles.iter().enumerate() {
            let marker = if i == index { "> " } else { "  " };
            box_content(buffer, &format!("{marker}{}", entry.name));
            box_content(buffer, &format!("    {}", entry.summary));
        }
    }
    draw_separator(buffer);
    if let Some(msg) = status {
        box_content(buffer, msg);
        draw_separator(buffer);
    }
    box_content(buffer, "[↑/↓] Select  [Enter] Apply  [Bksp] Back");
    box_content(buffer, "[n] Save current  [Del] Delete");
    draw_bottom(buffer);
}

fn draw_save_prompt_ui(buffer: &mut String, input: &str, error: &Option<String>) {
    draw_top(buffer);
    box_and_side_pad(buffer, "< BACK", "SAVE PROFILE");
    draw_separator(buffer);
    box_content(buffer, "Save current settings as:");
    box_content(buffer, &format!("name: {input}_"));
    draw_separator(buffer);
    if let Some(err) = error {
        box_content(buffer, err);
        draw_separator(buffer);
    }
    box_content(buffer, "[Enter] Save  [Bksp] Edit  [Esc] Cancel");
    draw_bottom(buffer);
}

fn draw_confirm_delete_ui(buffer: &mut String, name: &str) {
    draw_top(buffer);
    box_and_side_pad(buffer, "< BACK", "DELETE PROFILE");
    draw_separator(buffer);
    box_content(buffer, &format!("Delete {name}?"));
    draw_separator(buffer);
    box_content(buffer, "[y] Yes  [n] No");
    draw_bottom(buffer);
}

fn draw_options(s: &mut String, index: usize, items: &[UiRow; 5], _edit_mode: bool) {
    // Find widest item
    let mut max_length = 0;
    for item in items {
        if item.visible {
            max_length = max(item.name.len(), max_length);
        }
    }
    let mut content = String::new();
    for (i, item) in items.iter().enumerate() {
        if !item.visible {
            continue
        }
        if i == index {
            content.push_str(" > ");
        } else {
            content.push_str("   ");
        }

        content.push_str(&format!("{:<max_length$}", &item.name));
        // add 4 separators for visibility
        content.push_str("    ");

        content.push_str(&item.value);
        box_content(s, &content);
        content.clear();
    }
}

fn draw_main_menu_navigation(s: &mut String) {
    box_content(s, "[↑/↓] Navigate  [←/→] Change  [q] Quit");
}

fn draw_separator(s: &mut String) {
    s.push(LEFT_T);
    draw_horizontal_line(s);
    s.push(RIGHT_T);
    s.push('\n');
}

fn draw_top(s: &mut String) {
    s.push(TOP_LEFT);
    draw_horizontal_line(s);
    s.push(TOP_RIGHT);
    s.push('\n');
}

fn draw_bottom(s: &mut String) {
    s.push(DOWN_LEFT);
    draw_horizontal_line(s);
    s.push(DOWN_RIGHT);
    s.push('\n');
}

fn draw_horizontal_line(s: &mut String) {
    for _ in 0..TOTAL_WIDTH-2 {
        s.push(HORIZONTAL);
    }
}


fn box_content(s: &mut String, content: &str) {
    // Includes both `|` characters and leading space.
    let target_width = TOTAL_WIDTH - 3;

    for line in content.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            continue;
        }
        for chunk in chars.chunks(target_width) {
            let chunk: String = chunk.iter().collect();
            push_box_line(s, &chunk);
        }
    }
}

fn box_and_side_pad(s: &mut String, left_content: &str, right_content: &str) {
    // Includes both `|` characters, leading & trailing space.
    let target_width = TOTAL_WIDTH - 4;

    let content_width = left_content.chars().count() + right_content.chars().count();
    let padding = target_width - content_width;
    s.push(VERTICAL);
    s.push(' ');
    s.push_str(left_content);
    for _ in 0..padding {
        s.push(' ');
    }
    s.push_str(right_content);
    s.push(' ');
    s.push(VERTICAL);
    s.push('\n');
}

fn push_box_line(s: &mut String, content: &str) {
    let target_width = TOTAL_WIDTH - 3;
    let padding = target_width - content.chars().count();
    s.push(VERTICAL);
    s.push(' ');
    s.push_str(content);
    for _ in 0..padding {
        s.push(' ');
    }
    s.push(VERTICAL);
    s.push('\n');
}

