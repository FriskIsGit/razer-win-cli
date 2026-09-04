use std::cmp::{max, PartialEq};
use std::fmt::Write;
use hidapi::HidApi;
use razer_hid::{Device, DeviceDef, Registry};
use crate::cmd;
use crate::cmd::{
    apply_lighting, apply_profile, cmd_dpi, cmd_get_dpi,
    cmd_get_polling, cmd_polling, list_profiles, load_profile,
};
use crate::inputs::KeyCode;
use crate::{led_id_for, Effect, Profile};

fn test() {
    // After setting DPI do we get a report back stating what DPI is currently set?
    print_color_bar(128, 255, 128, 50);
    
    println!("\nThis text is back to the default terminal color.");

    println!("┌──────────────────────────────────────────────┐");
    println!("│  < BACK                      LIGHTING CONFIG │");
    println!("├──────────────────────────────────────────────┤");
    println!("│  Current Color:  [████████████]  RGB(255,0,0)│");
    println!("│                                              │");
    println!("│  > Set Color (RGB)                           │");
    println!("│    Brightness    :  85%                      │");
    println!("│    Effect        :  Static                   │");
    println!("│    Speed         :  Medium                   │");
    println!("│                                              │");
    println!("├──────────────────────────────────────────────┤");
    println!("│  [↑/↓] Navigate  [Enter] Select  [Esc] Back  │");
    println!("└──────────────────────────────────────────────┘");
}

const TOP_LEFT: char = '┌';
const TOP_RIGHT: char = '┐';
const DOWN_RIGHT: char = '┘';
const DOWN_LEFT: char = '└';
const LEFT_T: char = '├';
const RIGHT_T: char = '┤';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';
const WIDTH: usize = 48;
const DPI_STEP: usize = 50;

struct MenuItem {
    name: String,
    value: Option<usize>,
    unit: Option<String>,
    is_expandable: bool,
    action: Action,
}

impl MenuItem {
    fn new(name: &str, value: Option<usize>, unit: &str, action: Action) -> Self {
        Self { name: name.into(), value, unit: Some(unit.into()), is_expandable: false, action}
    }

    fn new_expandable(name: &str, action: Action) -> Self {
        Self { name: name.into(), value: None, unit: None, is_expandable: true, action }
    }
}

#[derive(PartialEq, Eq)]
enum Action {
    Noop,
    Dpi,
    Polling,
    Brightness,
    Lighting,
    Profiles,
}

const DPI_X_INDEX: usize = 0;
const DPI_Y_INDEX: usize = 1;
const POLLING_TABLE: [u16; 3] = [125, 500, 1000];

const LIGHTING_ROW_EFFECT: usize = 0;
const LIGHTING_ROW_COLOR_R: usize = 1;
const LIGHTING_ROW_COLOR_G: usize = 2;
const LIGHTING_ROW_COLOR_B: usize = 3;
const LIGHTING_ROW_BRIGHTNESS: usize = 4;
const LIGHTING_ROWS: usize = 5;
const LIGHTING_STEP: i8 = 16;

const EFFECTS: [Effect; 6] = [
    Effect::Static,
    Effect::Breathing,
    Effect::Spectrum,
    Effect::Wave,
    Effect::Reactive,
    Effect::Off,
];

fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Static => "Static",
        Effect::Breathing => "Breathing",
        Effect::Spectrum => "Spectrum",
        Effect::Wave => "Wave",
        Effect::Reactive => "Reactive",
        Effect::Off => "Off",
    }
}

pub fn start(api: HidApi, registry: Registry) -> Result<(), String> {
    let pid = cmd::auto_detect_pid(&api, &registry)?;

    let (device, definition) = cmd::open_device(&api, &registry, pid)?;

    let (x, y) = cmd_get_dpi(&device, definition)?;
    let polling = cmd_get_polling(&device, definition)?;

    let mut edit_mode = false;
    let mut index = 0;

    let mut polling_index = 0;
    if let Some(current_polling_index) = POLLING_TABLE.iter().position(|p| *p == polling) {
        polling_index = current_polling_index;
    }

    let dpi_x = MenuItem::new("DPI X", Some(x as usize), "", Action::Dpi);
    let dpi_y = MenuItem::new("DPI Y", Some(y as usize), "", Action::Dpi);
    let polling = MenuItem::new("POLLING (RATE)", Some(polling as usize), "Hz", Action::Polling);
    let lighting = MenuItem::new_expandable("LIGHTING", Action::Lighting);
    let profiles = MenuItem::new_expandable("PROFILES", Action::Profiles);
    let mut menu_items = [dpi_x, dpi_y, polling, lighting, profiles];


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
                if index > 0 {
                    index -= 1;
                }
            }
            KeyCode::ArrowLeft => {
                if menu_items[index].is_expandable {
                    continue
                }
                let action = &menu_items[index].action;
                match action {
                    Action::Dpi => {
                        let dpi_value = menu_items[index].value.expect("DPI must have value");
                        let mut x = menu_items[DPI_X_INDEX].value.expect("DPI X must have value");
                        let mut y = menu_items[DPI_Y_INDEX].value.expect("DPI Y must have value");
                        if dpi_value < DPI_STEP {
                            continue
                        }
                        let new_value = dpi_value - DPI_STEP;
                        if index == DPI_X_INDEX {
                            x = new_value;
                        } else {
                            y = new_value;
                        }
                        // Better handle error to show in UI
                        match cmd_dpi(&device, definition, x as u16, y as u16) {
                            Ok(()) => menu_items[index].value = Some(new_value),
                            Err(_) => continue,
                        }
                    }
                    Action::Polling => {
                        if polling_index == 0 {
                            continue
                        }
                        polling_index -= 1;
                        let new_polling = POLLING_TABLE[polling_index];
                        match cmd_polling(&device, definition, new_polling) {
                            Ok(()) => menu_items[index].value = Some(new_polling as usize),
                            Err(_) => continue,
                        }
                    }
                    _  => {}
                }

            }
            KeyCode::ArrowRight => {
                if menu_items[index].is_expandable { continue; }

                let action = &menu_items[index].action;
                match action {
                    Action::Dpi => {
                        let dpi_value = menu_items[index].value.expect("DPI must have value");
                        let mut x = menu_items[DPI_X_INDEX].value.expect("DPI X must have value");
                        let mut y = menu_items[DPI_Y_INDEX].value.expect("DPI Y must have value");

                        let mut max_dpi = 45000;
                        if let Some(dpi_max) = definition.dpi_max {
                            max_dpi = dpi_max;
                        };
                        let new_value = dpi_value + DPI_STEP;
                        if new_value > max_dpi as usize {
                            continue
                        }
                        if index == DPI_X_INDEX {
                            x = new_value;
                        } else {
                            y = new_value;
                        }

                        match cmd_dpi(&device, definition, x as u16, y as u16) {
                            Ok(()) => { menu_items[index].value = Some(new_value); }
                            Err(_) => continue,
                        }
                    }
                    Action::Polling => {
                        if polling_index >= POLLING_TABLE.len() - 1 {
                            continue
                        }
                        polling_index += 1;
                        let new_polling = POLLING_TABLE[polling_index];
                        match cmd_polling(&device, definition, new_polling) {
                            Ok(()) => menu_items[index].value = Some(new_polling as usize),
                            Err(_) => continue,
                        }
                    }
                    _ => {}
                }
            }

            KeyCode::ArrowDown | KeyCode::Char('s') | KeyCode::Char('S') => {
                if index < menu_items.len() - 1 {
                    index += 1;
                }
            }
            KeyCode::Enter => {
                let item = &menu_items[index];
                if item.is_expandable {
                    match item.action {
                        Action::Lighting => start_lighting_menu(&device, definition, &mut buffer),
                        Action::Profiles => start_profiles_menu(&device, definition, &mut buffer),
                        _ => {}
                    }
                } else {
                    if edit_mode {

                    }
                    edit_mode = !edit_mode;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn draw_ui(buffer: &mut String, definition: &DeviceDef, index: usize, menu_items: &[MenuItem; 5], edit_mode: bool) {
    draw_top(buffer);
    box_content(buffer, &definition.name);
    draw_separator(buffer);
    draw_options(buffer, index, menu_items, edit_mode);
    draw_separator(buffer);
    draw_main_menu_navigation(buffer);
    draw_bottom(buffer);
}

fn start_profiles_menu(device: &Device, definition: &DeviceDef, buffer: &mut String) {
    let pid = definition.usb_pid;
    let profiles = load_profile_entries(pid);
    let mut index = 0;
    let mut status: Option<String> = None;

    loop {
        crate::inputs::clear_console();

        draw_profiles_ui(buffer, &profiles, index, &status);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace => {
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
                match apply_profile(device, definition, pid, name) {
                    Ok(()) => status = Some(format!("applied {name:?}")),
                    Err(e) => status = Some(format!("failed to apply {name:?}: {e}")),
                }
            }
            _ => {}
        }
    }
}

fn start_lighting_menu(device: &Device, definition: &DeviceDef, buffer: &mut String) {
    let mut state = LightingState::read_initial(device, definition);
    let mut index = LIGHTING_ROW_EFFECT;
    let mut status: Option<String> = None;

    loop {
        crate::inputs::clear_console();

        draw_lighting_ui(buffer, index, &state, &status);
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace => {
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
            KeyCode::ArrowLeft => {
                adjust_lighting(device, definition, index, -LIGHTING_STEP, &mut state, &mut status);
            }
            KeyCode::ArrowRight => {
                adjust_lighting(device, definition, index, LIGHTING_STEP, &mut state, &mut status);
            }
            _ => {}
        }
    }
}

#[derive(PartialEq)]
struct LightingState {
    effect: Effect,
    color: [u8; 3],
    brightness: u8,
}

impl LightingState {
    /// Defaults to static white at full brightness.
    /// The device's current brightness is read if available (there is no get command for effect or color).
    fn read_initial(device: &Device, definition: &DeviceDef) -> Self {
        let mut state = Self {
            effect: Effect::Static,
            color: [255, 255, 255],
            brightness: 255,
        };
        if definition.capabilities.lighting {
            let led = led_id_for(definition);
            if let Ok(brightness) = cmd::cmd_get_brightness(device, definition, led) {
                state.brightness = brightness;
            }
        }
        state
    }
}

/// Step the selected lighting row by one step or return LightingState unchanged.
fn step_lighting_row(index: usize, delta: i8, state: &LightingState) -> LightingState {
    match index {
        LIGHTING_ROW_EFFECT => {
            let current = effect_index(state.effect) as isize;
            let mut next_index = current + delta.signum() as isize;
            if next_index < 0 || next_index >= EFFECTS.len() as isize {
                next_index = current;
            }
            LightingState {
                effect: EFFECTS[next_index as usize],
                color: state.color,
                brightness: state.brightness,
            }
        }
        LIGHTING_ROW_COLOR_R | LIGHTING_ROW_COLOR_G | LIGHTING_ROW_COLOR_B => {
            let channel = match index {
                LIGHTING_ROW_COLOR_R => 0,
                LIGHTING_ROW_COLOR_G => 1,
                LIGHTING_ROW_COLOR_B => 2,
                _ => unreachable!("matched earlier")
            };

            let mut color = state.color;
            color[channel] = step_u8(color[channel], delta);
            LightingState {
                effect: state.effect,
                color,
                brightness: state.brightness,
            }
        }
        LIGHTING_ROW_BRIGHTNESS => LightingState {
            effect: state.effect,
            color: state.color,
            brightness: step_u8(state.brightness, delta),
        },
        _ => unreachable!("index is always within lighting rows")
    }
}

fn effect_index(effect: Effect) -> usize {
    EFFECTS.iter().position(|e| *e == effect).unwrap_or(0)
}

fn step_u8(value: u8, delta: i8) -> u8 {
    (value as i16).saturating_add(delta as i16).clamp(0, 255) as u8
}

/// Step the selected lighting row and send the result to the device.
/// The local state is only committed on success;
/// on failure the old state is kept and the error is recorded for display.
fn adjust_lighting(
    device: &Device,
    definition: &DeviceDef,
    index: usize,
    delta: i8,
    state: &mut LightingState,
    status: &mut Option<String>,
) {
    let new_state = step_lighting_row(index, delta, state);
    if new_state == *state {
        return;
    }
    // TODO: Make the LED selectable from UI
    let led = led_id_for(definition);
    match apply_lighting(device, definition, new_state.effect, new_state.color, led, new_state.brightness) {
        Ok(()) => {
            *state = new_state;
            *status = None;
        }
        Err(e) => *status = Some(format!("set lighting failed: {e}")),
    }
}

fn draw_lighting_ui(buffer: &mut String, index: usize, state: &LightingState, status: &Option<String>) {
    draw_top(buffer);
    box_content(buffer, "< BACK");
    draw_separator(buffer);
    draw_lighting_color_bar(buffer, state.color);
    draw_separator(buffer);
    draw_lighting_options(buffer, index, state);
    draw_separator(buffer);
    if let Some(msg) = status {
        box_content(buffer, msg);
        draw_separator(buffer);
    }
    draw_sub_menu_navigation(buffer);
    draw_bottom(buffer);
}

struct LightingUiRow {
    pub name: &'static str,
    pub value: String,
}

impl LightingUiRow {
    fn new(name: &'static str, value: String) -> Self {
        Self { name, value }
    }
}

fn draw_lighting_options(s: &mut String, index: usize, state: &LightingState) {
    let rows: [LightingUiRow; LIGHTING_ROWS] = [
        LightingUiRow::new("EFFECT", effect_name(state.effect).to_string()),
        LightingUiRow::new("COLOR R", state.color[0].to_string()),
        LightingUiRow::new("COLOR G", state.color[1].to_string()),
        LightingUiRow::new("COLOR B", state.color[2].to_string()),
        LightingUiRow::new("BRIGHTNESS", state.brightness.to_string()),
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
    let bar_width = WIDTH - 1 - label.len() - rgb.len();
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

fn load_profile_entries(pid: u16) -> Vec<ProfileEntry> {
    let mut entries = Vec::new();
    for name in list_profiles() {
        match load_profile(&name) {
            Ok(profile) => entries.push(ProfileEntry {
                name,
                summary: profile_summary_for(&profile, pid),
            }),
            Err(e) => entries.push(ProfileEntry {
                name,
                summary: format!("unreadable: {e}"),
            }),
        }
    }
    entries
}

fn profile_summary_for(profile: &Profile, pid: u16) -> String {
    let Some(entry) = profile.devices.iter().find(|entry| entry.id == pid) else {
        return "(no settings for this device)".to_string();
    };
    let settings = &entry.settings;
    let mut parts: Vec<String> = Vec::new();
    if let Some(dpi) = settings.dpi {
        parts.push(format!("DPI {}x{}", dpi.x, dpi.y));
    }
    if let Some(lighting) = settings.lighting {
        parts.push(format!("{} {}/255", effect_name(lighting.effect), lighting.brightness));
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
    box_content(buffer, "< BACK");
    draw_separator(buffer);
    if profiles.is_empty() {
        box_content(buffer, "(no saved profiles)");
    } else {
        for (i, entry) in profiles.iter().enumerate() {
            let marker = if i == index { " > " } else { "   " };
            box_content(buffer, &format!("{marker}{}", entry.name));
            box_content(buffer, &format!("    {}", entry.summary));
        }
    }
    draw_separator(buffer);
    if let Some(msg) = status {
        box_content(buffer, msg);
        draw_separator(buffer);
    }
    draw_sub_menu_navigation(buffer);
    draw_bottom(buffer);
}

fn draw_options(s: &mut String, index: usize, items: &[MenuItem; 5], edit_mode: bool) {
    // Find widest item
    let mut max_length = 0;
    for item in items {
        max_length = max(item.name.len(), max_length);
    }
    let mut content = String::new();
    for (i, item) in items.iter().enumerate() {
        if i == index {
            content.push_str(" > ");
        } else {
            content.push_str("   ");
        }
        content.push_str(&format!("{:<max_length$}", &item.name));
        // add 4 separators for visibility
        content.push_str("    ");

        if item.is_expandable {
            content.push('>');
        } else {
            content.push_str(&item.value.unwrap().to_string());
            if let Some(unit) = item.unit.as_ref() {
                content.push(' ');
                content.push_str(unit);
            }

            if i == index && edit_mode {
                content.push(' ');
                content.push_str("[EDITING]");
            }
        }
        box_content(s, &content);
        content.clear();
    }
}

fn draw_main_menu_navigation(s: &mut String) {
    box_content(s, "[↑/↓] Navigate  [←/→] Change  [q] Quit");
}

fn draw_sub_menu_navigation(s: &mut String) {
    box_content(s, "[↑/↓] Navigate  [←/→] Change  [Bksp] Back");
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
    for _ in 0..WIDTH {
        s.push(HORIZONTAL);
    }
}


fn box_content(s: &mut String, content: &str) {
    // Truncate so long content (e.g. 64-char profile names) can't break the box.
    let truncated: String = content.chars().take(WIDTH - 1).collect();
    s.push(VERTICAL);
    s.push(' ');
    s.push_str(&truncated);
    let content_width = truncated.chars().count();
    let padding = WIDTH.saturating_sub(content_width + 1);
    for _ in 0..padding {
        s.push(' ');
    }
    s.push(VERTICAL);
    s.push('\n');
}

fn print_color_bar(r: u8, g: u8, b: u8, length: usize) {
    let reset = "\x1b[0m";
    let bg_code = format!("\x1b[48;2;{};{};{}m", r, g, b);
    let bar = " ".repeat(length);
    print!("{bg_code}{bar}{reset}\n");
}

fn pad_right(s: &mut String, text: &str, width: usize) {
    write!(s, "{:<width$}", text, width = width).unwrap();
}
