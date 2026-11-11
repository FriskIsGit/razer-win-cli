//! Windows global mouse-to-key rebinding for testing.

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering};

#[cfg(target_os = "windows")]
static FROM_BUTTON: AtomicU8 = AtomicU8::new(0);
#[cfg(target_os = "windows")]
static TO_VK: AtomicU16 = AtomicU16::new(0);
#[cfg(target_os = "windows")]
static TO_SCAN: AtomicU16 = AtomicU16::new(0);
#[cfg(target_os = "windows")]
static PRESSED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

pub fn cmd_rebind(args: &[String]) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows::run(args)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = args;
        Err("rebind is only supported on Windows for now".into())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;
    use std::time::Duration;

    const WH_MOUSE_LL: i32 = 14;
    const WM_QUIT: u32 = 0x0012;
    const WM_LBUTTONDOWN: usize = 0x0201;
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_RBUTTONDOWN: usize = 0x0204;
    const WM_RBUTTONUP: usize = 0x0205;
    const WM_MBUTTONDOWN: usize = 0x0207;
    const WM_MBUTTONUP: usize = 0x0208;
    const WM_XBUTTONDOWN: usize = 0x020B;
    const WM_XBUTTONUP: usize = 0x020C;
    const LLMHF_INJECTED: u32 = 0x00000001;
    const INPUT_KEYBOARD: u32 = 1;
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const KEYEVENTF_SCANCODE: u32 = 0x0008;
    const MAPVK_VK_TO_VSC: u32 = 0;
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MsllHookStruct {
        pt: Point,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct KeybdInput {
        w_vk: u16,
        w_scan: u16,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    union InputUnion {
        ki: KeybdInput,
        _pad: [u8; 32],
    }

    #[repr(C)]
    struct Input {
        type_: u32,
        u: InputUnion,
    }

    type HookProc = Option<unsafe extern "system" fn(i32, usize, isize) -> isize>;
    type CtrlHandler = Option<unsafe extern "system" fn(u32) -> i32>;

    extern "system" {
        fn SetWindowsHookExW(
            id_hook: i32,
            lpfn: HookProc,
            hmod: *mut c_void,
            thread_id: u32,
        ) -> *mut c_void;
        fn UnhookWindowsHookEx(hhk: *mut c_void) -> i32;
        fn CallNextHookEx(hhk: *mut c_void, code: i32, w_param: usize, l_param: isize) -> isize;
        fn GetMessageW(
            msg: *mut Msg,
            hwnd: *mut c_void,
            filter_min: u32,
            filter_max: u32,
        ) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostThreadMessageW(
            thread_id: u32,
            msg: u32,
            w_param: usize,
            l_param: isize,
        ) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
        fn SetConsoleCtrlHandler(handler: CtrlHandler, add: i32) -> i32;
    }

    unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
        if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT {
            let thread_id = THREAD_ID.load(Ordering::SeqCst);
            if thread_id != 0 {
                PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
            }
            return 1;
        }
        0
    }

    fn is_extended_key(vk: u16) -> bool {
        matches!(
            vk,
            0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C | 0x5D | 0x6F | 0x90 | 0x91
        )
    }

    unsafe fn send_key(vk: u16, scan: u16, up: bool) {
        let mut flags = KEYEVENTF_SCANCODE;
        if is_extended_key(vk) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        let mut input: Input = zeroed();
        input.type_ = INPUT_KEYBOARD;
        input.u.ki = KeybdInput {
            w_vk: 0,
            w_scan: scan,
            dw_flags: flags,
            time: 0,
            dw_extra_info: 0,
        };
        SendInput(1, &input, size_of::<Input>() as i32);
    }

    fn match_event(from: u8, w_param: usize, info: &MsllHookStruct) -> Option<bool> {
        match from {
            1 => match w_param {
                WM_LBUTTONDOWN => Some(true),
                WM_LBUTTONUP => Some(false),
                _ => None,
            },
            2 => match w_param {
                WM_RBUTTONDOWN => Some(true),
                WM_RBUTTONUP => Some(false),
                _ => None,
            },
            3 => match w_param {
                WM_MBUTTONDOWN => Some(true),
                WM_MBUTTONUP => Some(false),
                _ => None,
            },
            4 | 5 => {
                if w_param != WM_XBUTTONDOWN && w_param != WM_XBUTTONUP {
                    return None;
                }
                let xbutton = (info.mouse_data >> 16) as u16;
                let expected = if from == 4 { 1 } else { 2 };
                if xbutton != expected {
                    return None;
                }
                match w_param {
                    WM_XBUTTONDOWN => Some(true),
                    WM_XBUTTONUP => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    unsafe extern "system" fn mouse_proc(code: i32, w_param: usize, l_param: isize) -> isize {
        if code < 0 {
            return CallNextHookEx(null_mut(), code, w_param, l_param);
        }
        let info = &*(l_param as *const MsllHookStruct);
        if info.flags & LLMHF_INJECTED != 0 {
            return CallNextHookEx(null_mut(), code, w_param, l_param);
        }
        let Some(is_down) = match_event(FROM_BUTTON.load(Ordering::SeqCst), w_param, info) else {
            return CallNextHookEx(null_mut(), code, w_param, l_param);
        };
        let was_pressed = PRESSED.swap(is_down, Ordering::SeqCst);
        let vk = TO_VK.load(Ordering::SeqCst);
        let scan = TO_SCAN.load(Ordering::SeqCst);
        if is_down && !was_pressed {
            send_key(vk, scan, false);
        } else if !is_down && was_pressed {
            send_key(vk, scan, true);
        }
        1
    }

    fn parse_from(raw: &str) -> Result<u8, String> {
        match raw.to_ascii_lowercase().as_str() {
            "left" | "lmb" => Ok(1),
            "right" | "rmb" => Ok(2),
            "middle" | "mmb" => Ok(3),
            "back" | "x1" | "mouse4" | "m4" => Ok(4),
            "forward" | "x2" | "mouse5" | "m5" => Ok(5),
            other => Err(format!(
                "unknown mouse button {other:?}; use left, right, middle, back, forward, x1, x2"
            )),
        }
    }

    fn parse_to(raw: &str) -> Result<u16, String> {
        let lower = raw.to_ascii_lowercase();
        match lower.as_str() {
            "enter" => Ok(0x0D),
            "space" => Ok(0x20),
            "tab" => Ok(0x09),
            "esc" | "escape" => Ok(0x1B),
            "backspace" => Ok(0x08),
            "delete" | "del" => Ok(0x2E),
            "up" => Ok(0x26),
            "down" => Ok(0x28),
            "left" => Ok(0x25),
            "right" => Ok(0x27),
            _ => {
                if lower.len() == 1 {
                    let ch = lower.chars().next().unwrap();
                    if ch.is_ascii_alphanumeric() {
                        return Ok(ch.to_ascii_uppercase() as u16);
                    }
                }
                if let Some(number) = lower.strip_prefix('f') {
                    if let Ok(index) = number.parse::<u8>() {
                        if (1..=24).contains(&index) {
                            return Ok(0x70 + u16::from(index - 1));
                        }
                    }
                }
                Err(format!(
                    "unknown key {raw:?}; use a-z, 0-9, f1-f12, enter, space, tab, esc, arrows"
                ))
            }
        }
    }

    fn parse_seconds(args: &[String]) -> Result<Option<u64>, String> {
        if args.is_empty() {
            return Ok(None);
        }
        if args[0] != "--seconds" {
            return Err(format!("unknown rebind flag {:?}", args[0]));
        }
        let Some(value) = args.get(1) else {
            return Err("--seconds requires <n>".into());
        };
        let seconds = value
            .parse::<u64>()
            .map_err(|error| format!("invalid seconds: {error}"))?;
        if seconds == 0 {
            return Err("--seconds must be at least 1".into());
        }
        Ok(Some(seconds))
    }

    fn usage() -> String {
        "usage: rebind <from> <to> [--seconds <n>]\n  from: left, right, middle, back, forward, x1, x2\n  to: a-z, 0-9, f1-f24, enter, space, tab, esc, arrows".into()
    }

    pub fn run(args: &[String]) -> Result<(), String> {
        let Some(from_raw) = args.first() else {
            return Err(usage());
        };
        let Some(to_raw) = args.get(1) else {
            return Err(usage());
        };
        let from = parse_from(from_raw)?;
        let vk = parse_to(to_raw)?;
        let seconds = parse_seconds(&args[2..])?;
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 };
        if scan == 0 {
            return Err(format!("no scan code for key {to_raw:?}"));
        }

        FROM_BUTTON.store(from, Ordering::SeqCst);
        TO_VK.store(vk, Ordering::SeqCst);
        TO_SCAN.store(scan, Ordering::SeqCst);
        PRESSED.store(false, Ordering::SeqCst);

        THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        unsafe {
            SetConsoleCtrlHandler(Some(ctrl_handler), 1);
        }
        let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), null_mut(), 0) };
        if hook.is_null() {
            return Err(format!(
                "SetWindowsHookExW failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        println!(
            "rebinding {} -> {} (scan {:#04x})",
            from_raw, to_raw, scan
        );
        if let Some(value) = seconds {
            println!("test window: {value} seconds");
        } else {
            println!("press Ctrl+C to stop");
        }
        if from <= 3 {
            println!("warning: remapping a primary mouse button suppresses normal clicking");
        }

        if let Some(value) = seconds {
            let target_thread = unsafe { GetCurrentThreadId() };
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(value));
                unsafe {
                    PostThreadMessageW(target_thread, WM_QUIT, 0, 0);
                }
            });
        }

        let mut msg: Msg = unsafe { zeroed() };
        loop {
            let result = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
            if result == 0 || result == -1 {
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if PRESSED.swap(false, Ordering::SeqCst) {
            let vk = TO_VK.load(Ordering::SeqCst);
            let scan = TO_SCAN.load(Ordering::SeqCst);
            unsafe { send_key(vk, scan, true) };
        }
        unsafe {
            UnhookWindowsHookEx(hook);
            SetConsoleCtrlHandler(Some(ctrl_handler), 0);
        }
        THREAD_ID.store(0, Ordering::SeqCst);
        println!("rebind stopped");
        Ok(())
    }
}
