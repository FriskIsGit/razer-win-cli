use std::cmp::max;
use std::fmt::Write;
use hidapi::HidApi;
use razer_hid::Registry;
use crate::cmd;
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
const MAIN_MENU_ITEMS: usize = 5;

struct MenuItem {
    name: String,
    value: Option<usize>,
    unit: Option<String>,
    is_expandable: bool,
}

impl MenuItem {
    fn new(name: &str, value: Option<usize>, unit: &str, is_expandable: bool) -> Self {
        Self { name: name.into(), value, unit: Some(unit.into()), is_expandable }
    }

    fn new_expandable(name: &str) -> Self {
        Self { name: name.into(), value: None, unit: None, is_expandable: true }
    }
}

pub fn start(api: HidApi, registry: Registry) -> Result<(), String> {
    let pid = cmd::auto_detect_pid(&api, &registry)?;

    let (device, definition) = cmd::open_device(&api, &registry, pid)?;

    let mut in_main_menu = true;
    let mut index = 0;

    let dpi_x = MenuItem::new("DPI X", Some(0), "", false);
    let dpi_y = MenuItem::new("DPI Y", Some(0), "", false);
    let polling = MenuItem::new("POLLING (RATE)", Some(0), "Hz", false);
    let lighting = MenuItem::new_expandable("LIGHTING");
    let profiles = MenuItem::new_expandable("PROFILES");
    let menu_items = [dpi_x, dpi_y, polling, lighting, profiles];

    let mut buffer = String::with_capacity(1000);
    while in_main_menu {
        crate::inputs::clear_console();

        draw_top(&mut buffer);
        box_content(&mut buffer, &definition.name);
        draw_separator(&mut buffer);
        draw_options(&mut buffer, index, &menu_items);
        draw_separator(&mut buffer);
        draw_navigation(&mut buffer);
        draw_bottom(&mut buffer);

        println!("BUFFER LENGTH: {}", buffer.len());
        println!("{buffer}");
        buffer.clear();

        match crate::inputs::read_key() {
            KeyCode::Backspace => {}
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                in_main_menu = false;
            }
            KeyCode::ArrowUp | KeyCode::Char('w') | KeyCode::Char('W') => {
                if index > 0 {
                    index -= 1;
                }
            }
            KeyCode::ArrowDown | KeyCode::Char('s') | KeyCode::Char('S') => {
                if index < menu_items.len() - 1 {
                    index += 1;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn draw_options(s: &mut String, index: usize, items: &[MenuItem; 5]) {
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
            content.push(' ');
            content.push_str(item.unit.as_ref().unwrap());
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
