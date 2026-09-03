use std::cmp::{max, PartialEq};
use std::fmt::Write;
use hidapi::HidApi;
use razer_hid::{DeviceDef, Registry};
use crate::cmd;
use crate::cmd::{cmd_dpi, cmd_get_dpi, cmd_get_polling, cmd_polling};
use crate::inputs::KeyCode;

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

    let mut dpi_x = MenuItem::new("DPI X", Some(x as usize), "", Action::Dpi);
    let mut dpi_y = MenuItem::new("DPI Y", Some(y as usize), "", Action::Dpi);
    let mut polling = MenuItem::new("POLLING (RATE)", Some(polling as usize), "Hz", Action::Polling);
    let mut lighting = MenuItem::new_expandable("LIGHTING", Action::Lighting);
    let mut profiles = MenuItem::new_expandable("PROFILES", Action::Profiles);
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

                if menu_items[index].action == Action::Dpi {
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
                } else if menu_items[index].action == Action::Polling {
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
                        Action::Lighting => start_lighting_menu(&mut buffer),
                        Action::Profiles => start_profiles_menu(&mut buffer),
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
    draw_navigation(buffer);
    draw_bottom(buffer);
}

fn start_profiles_menu(buffer: &mut String) {
    let index = 0;
    let edit_mode = false;
    loop {
        crate::inputs::clear_console();

        draw_top(buffer);
        draw_separator(buffer);
        draw_separator(buffer);
        draw_navigation(buffer);
        draw_bottom(buffer);

        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace => {
                break;
            }
            _ => {}
        }
    }
}

fn start_lighting_menu(buffer: &mut String) {
    let index = 0;
    let edit_mode = false;
    loop {
        crate::inputs::clear_console();

        draw_top(buffer);
        draw_separator(buffer);
        draw_separator(buffer);
        draw_navigation(buffer);
        draw_bottom(buffer);

        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace => {
                break;
            }
            _ => {}
        }
    }
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

fn draw_navigation(s: &mut String) {
    box_content(s, "[↑/↓] Navigate  [Enter] Edit  [q] Quit");
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
    s.push(VERTICAL);
    s.push(' ');
    s.push_str(content);
    let content_width = content.chars().count();
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
