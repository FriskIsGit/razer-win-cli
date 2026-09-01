use std::fmt::Write;
use hidapi::HidApi;
use razer_hid::Registry;

fn test() {
    // 1. Strip get prefix
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

pub fn start(api: HidApi, registry: Registry) {
    let mut buffer = String::with_capacity(WIDTH * 15);
    draw_top(&mut buffer);
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

    println!("{buffer}")
}

fn draw_separator(s: &mut String) {
    s.push(LEFT_T);
    draw_horizontal(s);
    s.push(RIGHT_T);
    s.push('\n');
}

fn draw_top(s: &mut String) {
    s.push(TOP_LEFT);
    draw_horizontal(s);
    s.push(TOP_RIGHT);
    s.push('\n');
}

fn draw_bottom(s: &mut String) {
    s.push(DOWN_LEFT);
    draw_horizontal(s);
    s.push(DOWN_RIGHT);
    s.push('\n');
}

fn draw_horizontal(s: &mut String) {
    for _ in 0..WIDTH {
        s.push(HORIZONTAL);
    }
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
