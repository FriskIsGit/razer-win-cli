use std::fmt::Write;
use hidapi::HidApi;
use razer_hid::Registry;
use crate::cmd;

fn test() {
    // After setting DPI do we get a report back stating what DPI is currently set?
    print_color_bar(128, 255, 128, 50);
    
    println!("\nThis text is back to the default terminal color.");
    println!("┌─────────────────────────────────────────────┐");
    println!("│  DEVICE: SuperMouse V2          [CONNECTED] |");
    println!("├─────────────────────────────────────────────┤");
    println!("│                                             │");
    println!("│  > DPI X             1600                   │");
    println!("│    DPI Y             1600                   │");
    println!("│    POLLING (RATE)    1000 Hz                │");
    println!("│    LIGHTING           ›                     │");
    println!("│    PROFILES           ›                     │");
    println!("│                                             │");
    println!("├─────────────────────────────────────────────┤");
    println!("│  [↑/↓] Navigate  [Enter] Edit  [q] Quit     │");
    println!("└─────────────────────────────────────────────┘");

  
    println!("┌──────────────────────────────────────────────┐");
    println!("│  < BACK                      LIGHTING CONFIG │");
    println!("├──────────────────────────────────────────────┤");
    println!("│                                              │");
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

pub fn start(api: HidApi, registry: Registry) -> Result<(), String> {
    let pid = cmd::auto_detect_pid(&api, &registry)?;

    let (device, definition) = cmd::open_device(&api, &registry, pid)?;

    let mut buffer = String::with_capacity(WIDTH * 15);
    draw_top(&mut buffer);
    box_content(&mut buffer, &definition.name);
    draw_separator(&mut buffer);
    draw_separator(&mut buffer);
    draw_bottom(&mut buffer);

    println!("┌─────────────────────────────────────────────┐");
    println!("│  DEVICE: SuperMouse V2          [CONNECTED] |");
    println!("├─────────────────────────────────────────────┤");
    println!("│                                             │");
    println!("│  > DPI X             1600                   │");
    println!("│    DPI Y             1600                   │");
    println!("│    POLLING (RATE)    1000 Hz                │");
    println!("│    LIGHTING           ›                     │");
    println!("│    PROFILES           ›                     │");
    println!("│                                             │");
    println!("├─────────────────────────────────────────────┤");
    println!("│  [↑/↓] Navigate  [Enter] Edit  [q] Quit     │");
    println!("└─────────────────────────────────────────────┘");

    println!("{buffer}");
    Ok(())
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
