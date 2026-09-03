#[derive(Debug)]
pub enum KeyCode {
    Char(char),
    Enter,
    Backspace,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowRight,
    ArrowLeft,
    Other(u64),
    Error,
}

#[cfg(target_os = "windows")]
pub(crate) mod windows {
    use super::KeyCode;

    #[allow(non_camel_case_types)]
    type void = std::ffi::c_void;

    const STD_INPUT_HANDLE:  u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;

    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct EventKey {
        key_down:          i32,
        repeat_count:      u16,
        virtual_keycode:   u16,
        virtual_scancode:  u16,
        character_data:    u16,
        // character_data:    u8,
        control_key_state: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct EventMouse {
        mouse_position:     Coord,
        button_state:       u32,
        control_key_state : u32,
        event_flags:        u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    union Event {
        focus: i32,
        key:   EventKey,
        menu:  u32,
        mouse: EventMouse,
        size:  Coord,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct InputRecord {
        event_type: u16,
        event: Event,
    }

    impl Default for InputRecord {
        fn default() -> Self {
            Self {
                event_type: 0,
                event: Event { focus: 0 }
            }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct CharInfo {
        // ascii_char:   u8,
        unicode_char: u16,
        attributes:   u16,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct SmallRect {
        left:   i16,
        top:    i16,
        right:  i16,
        bottom: i16,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct ConsoleBufferInfo {
        buffer_size:     Coord,
        cursor_position: Coord,
        attributes:      u16,
        window_coords:   SmallRect,
        maximum_size:    Coord,
    }

    extern "system" {
        fn GetStdHandle(std_handle_code: u32) -> *const void;
        fn FlushConsoleInputBuffer(handle: *const void) -> i32;
        fn ReadConsoleInputW(handle: *const void, buffer: *mut InputRecord, buffer_length: i32, entries_read: *mut u32) -> i32;
        fn GetConsoleMode(handle: *const void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *const void, mode: u32) -> i32;
        fn ReadConsoleA(handle: *const void, buffer: *mut void, buffer_size: u32, bytes_read: *mut u32, input_control: *const u32) -> i32;
        fn WriteConsoleW(handle: *const void, buffer: *const void, buffer_size: u32, bytes_written: *mut u32, reserved: *const void);

        fn GetConsoleScreenBufferInfo(handle: *const void, buffer_info: *mut ConsoleBufferInfo) -> i32;
        fn ScrollConsoleScreenBufferW(handle: *const void, scroll: *const SmallRect, clip: *const SmallRect, destination: Coord, fill: *const CharInfo) -> i32;
        fn SetConsoleCursorPosition(handle: *const void, cursor_position: Coord) -> i32;
    }

    unsafe fn fallback_read_key(handle: *const void) -> KeyCode {
        use std::io::Read;

        let mut buffer = [0u8; 3];
        let _ = std::io::stdin().read(&mut buffer);
        return KeyCode::Char(char::from_u32_unchecked(buffer[0] as u32));
    }

    pub fn read_key() -> KeyCode {
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle == std::ptr::null() {
                return KeyCode::Error
            }

            let _ = FlushConsoleInputBuffer(handle);

            let mut entries_read = 0u32;
            let mut input = InputRecord::default();
            loop {
                let result = ReadConsoleInputW(handle, &mut input as *mut InputRecord, 1, &mut entries_read as *mut u32);

                // Reading the console input failed.
                if result == 0 || entries_read == 0 {
                    return fallback_read_key(handle);
                }

                if input.event_type != 1 {
                    continue;
                }

                let key = input.event.key;
                if key.key_down == 0 {
                    continue;
                }

                let key = input.event.key;

                if key.character_data != 0 {
                    let data = key.character_data;
                    match key.character_data  {
                        65..=90 | 97..=122 => return KeyCode::Char(char::from_u32_unchecked(data as u32)),
                        8  => return KeyCode::Backspace,
                        13 => return KeyCode::Enter,
                        32 => return KeyCode::Space,
                        _  => return KeyCode::Other(data as u64),
                    }
                }

                match key.virtual_keycode {
                    0x25 => return KeyCode::ArrowLeft,
                    0x26 => return KeyCode::ArrowUp,
                    0x27 => return KeyCode::ArrowRight,
                    0x28 => return KeyCode::ArrowDown,
                    _    => continue,
                }
            }
        }
    }

    pub fn clear_console() {
        println!("\x1b[2J\x1b[3J\x1b[H");

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle == std::ptr::null() {
                return;
            }

            let mut buffer_info = ConsoleBufferInfo::default();
            GetConsoleScreenBufferInfo(handle, &mut buffer_info as *mut ConsoleBufferInfo);

            let rect = SmallRect {
                left: 0,
                top: 0,
                right: buffer_info.buffer_size.x,
                bottom: buffer_info.buffer_size.y,
            };

            let target = Coord {
                x: 0,
                y: 0 - buffer_info.buffer_size.y
            };

            let fill = CharInfo {
                unicode_char: 32,
                attributes: buffer_info.attributes,
            };

            ScrollConsoleScreenBufferW(handle, &rect as *const SmallRect, std::ptr::null(), target, &fill as *const CharInfo);

            buffer_info.cursor_position.x = 0;
            buffer_info.cursor_position.y = 0;
            SetConsoleCursorPosition(handle, buffer_info.cursor_position);
        }
    }
}


#[cfg(unix)]
pub(crate) mod unix {
    use super::KeyCode;

    const STDIN:   i32 = 0;

    #[allow(non_camel_case_types)]
    type void = std::ffi::c_void;

    const TCSANOW: i32 = 0;
    const ICANON:  i32 = 2;
    const ECHO:    i32 = 10;

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    struct Termios {
        input_mode:          i32,       // tcflag_t c_iflag
        output_mode:         i32,       // tcflag_t c_oflag
        control_mode:        i32,       // tcflag_t c_cflag
        local_mode:          i32,       // tcflag_t c_lflag
        line_discipline:     i32,       // cc_t     c_line
        control_characters: [i32; 32],  // cc_t     c_cc[32]
        input_speed:         i32,       // speed_t  c_ispeed
        output_speed:        i32,       // speed_t  c_ospeed
    }

    const POLLIN: i16 = 1;

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    struct PollFd {
        file_descriptor: i32,  // int   fd
        request_events:  i16,  // short events
        return_events:   i16,  // short revents
    }

    extern "C" {
        fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
        fn read(fd: i32, buffer: *mut void, buffer_size: i32) -> i32;
        fn poll(fds: *mut PollFd, fds_count: u64, timeout: i32) -> i32;
    }

    unsafe fn flush_stdin() {
        let mut pollfd = PollFd {
            file_descriptor: STDIN,
            request_events:  POLLIN,
            return_events:   0,
        };

        poll(&mut pollfd as *mut PollFd, 1, 0);
        while pollfd.return_events != 0 {
            let mut data: u8 = 0;
            let _ = read(STDIN, &mut data as *mut u8 as *mut void, 1);

            let poll_result = poll(&mut pollfd as *mut PollFd, 1, 0);
            if poll_result == -1 {
                break;
            }
        }
    }

    pub fn read_key() -> KeyCode {
        unsafe {
            let mut old_settings = Termios::default();
            tcgetattr(STDIN, &mut old_settings as *mut Termios);

            let mut new_settings = old_settings;
            new_settings.local_mode &= !(ICANON | ECHO);

            tcsetattr(STDIN, TCSANOW, &new_settings as *const Termios);
            flush_stdin();

            let mut data: u64 = 0;
            let _ = read(STDIN, &mut data as *mut u64 as *mut void, 8);

            tcsetattr(STDIN, TCSANOW, &old_settings as *const Termios);

            match data {
                65..=90 | 97..=122 => return KeyCode::Char(char::from_u32_unchecked(data as u32)),
                10  => return KeyCode::Enter,
                32  => return KeyCode::Space,
                127 => return KeyCode::Backspace,
                0x445b1b => return KeyCode::ArrowLeft,
                0x415b1b => return KeyCode::ArrowUp,
                0x435b1b => return KeyCode::ArrowRight,
                0x425b1b => return KeyCode::ArrowDown,
                _ => return KeyCode::Other(data),
            }
        }
    }

    pub fn clear_console() {
        println!("\x1b[2J\x1b[3J\x1b[H");
    }
}

#[cfg(target_os = "windows")]
pub use windows::{clear_console, read_key};

#[cfg(unix)]
pub use unix::{clear_console, read_key};