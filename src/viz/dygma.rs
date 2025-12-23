//! Dygma Raise split keyboard visualization
//!
//! Shows the split keyboard layout with:
//! - Real-time key press heat map via evdev
//! - Active layer detection via Focus serial protocol
//! - Per-layer key labels from keyboard firmware

use crate::terminal::Terminal;
use crate::viz::{scheme_color, VizState};
use crossterm::event::KeyCode;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Configuration for Dygma visualization
pub struct DygmaConfig {
    pub time_step: f32,
    pub port: Option<PathBuf>,
    pub debug: bool,
}

// ============================================================================
// Physical Layout Constants
// ============================================================================

/// Key position: (x, y, width) in key units
/// x=0 is leftmost, y=0 is top row
/// Standard key width is 1.0
#[derive(Clone, Copy)]
struct KeyPos {
    x: f32,
    y: f32,
    w: f32,
}

impl KeyPos {
    const fn new(x: f32, y: f32, w: f32) -> Self {
        Self { x, y, w }
    }
}

/// Left half main keys (32 keys, positions 0-31)
/// Columnar stagger: each column has slight vertical offset
const LEFT_MAIN: &[(usize, KeyPos)] = &[
    // Row 0 (top): ESC, 1-6
    (0,  KeyPos::new(0.0, 0.0, 1.0)),   // ESC
    (1,  KeyPos::new(1.0, 0.0, 1.0)),   // 1
    (2,  KeyPos::new(2.0, 0.0, 1.0)),   // 2
    (3,  KeyPos::new(3.0, 0.0, 1.0)),   // 3
    (4,  KeyPos::new(4.0, 0.0, 1.0)),   // 4
    (5,  KeyPos::new(5.0, 0.0, 1.0)),   // 5
    (6,  KeyPos::new(6.0, 0.0, 1.0)),   // 6
    // Row 1: Tab, Q-T (columnar stagger)
    (7,  KeyPos::new(0.0, 1.0, 1.5)),   // Tab
    (8,  KeyPos::new(1.5, 1.1, 1.0)),   // Q
    (9,  KeyPos::new(2.5, 1.0, 1.0)),   // W
    (10, KeyPos::new(3.5, 0.9, 1.0)),   // E
    (11, KeyPos::new(4.5, 1.0, 1.0)),   // R
    (12, KeyPos::new(5.5, 1.1, 1.0)),   // T
    // Row 2: Caps, A-G
    (13, KeyPos::new(0.0, 2.0, 1.75)),  // Caps
    (14, KeyPos::new(1.75, 2.1, 1.0)),  // A
    (15, KeyPos::new(2.75, 2.0, 1.0)),  // S
    (16, KeyPos::new(3.75, 1.9, 1.0)),  // D
    (17, KeyPos::new(4.75, 2.0, 1.0)),  // F
    (18, KeyPos::new(5.75, 2.1, 1.0)),  // G
    // Row 3: Shift, Z-B
    (19, KeyPos::new(0.0, 3.0, 1.25)),  // Shift
    (20, KeyPos::new(1.25, 3.1, 1.0)),  // \
    (21, KeyPos::new(2.25, 3.1, 1.0)),  // Z
    (22, KeyPos::new(3.25, 3.0, 1.0)),  // X
    (23, KeyPos::new(4.25, 2.9, 1.0)),  // C
    (24, KeyPos::new(5.25, 3.0, 1.0)),  // V
    (25, KeyPos::new(6.25, 3.1, 1.0)),  // B
    // Row 4: Ctrl, Win, Alt + small keys
    (26, KeyPos::new(0.0, 4.0, 1.25)),  // Ctrl
    (27, KeyPos::new(1.25, 4.0, 1.0)),  // Win
    (28, KeyPos::new(2.25, 4.0, 1.0)),  // Alt
    (29, KeyPos::new(3.25, 4.0, 1.0)),  // (space left)
    (30, KeyPos::new(4.5, 4.0, 1.0)),   // (small)
    (31, KeyPos::new(5.75, 4.0, 1.0)),  // (small)
];

/// Left thumb cluster (4 keys, positions 32-35)
const LEFT_THUMB: &[(usize, KeyPos)] = &[
    (32, KeyPos::new(4.0, 5.0, 1.5)),   // T1 (inner top)
    (33, KeyPos::new(5.5, 5.0, 1.5)),   // T2 (outer top)
    (34, KeyPos::new(4.0, 6.0, 1.5)),   // T3 (inner bottom)
    (35, KeyPos::new(5.5, 6.0, 1.5)),   // T4 (outer bottom)
];

/// Right half main keys (32 keys, positions 40-71)
/// Mirror of left with offset
const RIGHT_MAIN: &[(usize, KeyPos)] = &[
    // Row 0: 7-0, -, =, Backspace
    (40, KeyPos::new(0.0, 0.0, 1.0)),   // 7
    (41, KeyPos::new(1.0, 0.0, 1.0)),   // 8
    (42, KeyPos::new(2.0, 0.0, 1.0)),   // 9
    (43, KeyPos::new(3.0, 0.0, 1.0)),   // 0
    (44, KeyPos::new(4.0, 0.0, 1.0)),   // -
    (45, KeyPos::new(5.0, 0.0, 1.0)),   // =
    (46, KeyPos::new(6.0, 0.0, 1.5)),   // Backspace
    // Row 1: Y-P, [, ], \
    (47, KeyPos::new(0.0, 1.1, 1.0)),   // Y
    (48, KeyPos::new(1.0, 1.0, 1.0)),   // U
    (49, KeyPos::new(2.0, 0.9, 1.0)),   // I
    (50, KeyPos::new(3.0, 1.0, 1.0)),   // O
    (51, KeyPos::new(4.0, 1.1, 1.0)),   // P
    (52, KeyPos::new(5.0, 1.1, 1.0)),   // [
    (53, KeyPos::new(6.0, 1.0, 1.5)),   // ]
    // Row 2: H-L, ;, ', Enter
    (54, KeyPos::new(0.0, 2.1, 1.0)),   // H
    (55, KeyPos::new(1.0, 2.0, 1.0)),   // J
    (56, KeyPos::new(2.0, 1.9, 1.0)),   // K
    (57, KeyPos::new(3.0, 2.0, 1.0)),   // L
    (58, KeyPos::new(4.0, 2.1, 1.0)),   // ;
    (59, KeyPos::new(5.0, 2.1, 1.0)),   // '
    (60, KeyPos::new(6.0, 2.0, 1.5)),   // Enter
    // Row 3: N-/, Shift
    (61, KeyPos::new(0.0, 3.1, 1.0)),   // N
    (62, KeyPos::new(1.0, 3.0, 1.0)),   // M
    (63, KeyPos::new(2.0, 2.9, 1.0)),   // ,
    (64, KeyPos::new(3.0, 3.0, 1.0)),   // .
    (65, KeyPos::new(4.0, 3.1, 1.0)),   // /
    (66, KeyPos::new(5.0, 3.1, 1.0)),   // (extra)
    (67, KeyPos::new(6.0, 3.0, 1.5)),   // Shift
    // Row 4: small keys, Alt, Win, Menu, Ctrl
    (68, KeyPos::new(0.0, 4.0, 1.0)),   // (small)
    (69, KeyPos::new(1.25, 4.0, 1.0)),  // (small)
    (70, KeyPos::new(2.5, 4.0, 1.0)),   // AltGr
    (71, KeyPos::new(3.5, 4.0, 1.0)),   // Win
    (72, KeyPos::new(4.5, 4.0, 1.0)),   // Menu
    (73, KeyPos::new(5.5, 4.0, 1.25)),  // Ctrl
];

/// Right thumb cluster (4 keys, positions 76-79)
const RIGHT_THUMB: &[(usize, KeyPos)] = &[
    (76, KeyPos::new(0.0, 5.0, 1.5)),   // T5 (outer top)
    (77, KeyPos::new(1.5, 5.0, 1.5)),   // T6 (inner top)
    (78, KeyPos::new(0.0, 6.0, 1.5)),   // T7 (outer bottom)
    (79, KeyPos::new(1.5, 6.0, 1.5)),   // T8 (inner bottom)
];

/// Map from physical key index (our layout) to Dygma keymap index
/// Based on official Dygma RaiseANSIKeyMap.png
/// Array index = our physical position, value = Dygma keymap index
const PHYSICAL_TO_KEYMAP: &[usize] = &[
    // Physical 0-6: Left Row 0 (ESC, 1-6)
    0, 1, 2, 3, 4, 5, 6,
    // Physical 7-12: Left Row 1 (Tab, Q-T)
    16, 17, 18, 19, 20, 21,
    // Physical 13-18: Left Row 2 (Caps, A-G)
    32, 33, 34, 35, 36, 37,
    // Physical 19-25: Left Row 3 (Shift, \, Z-B) - Dygma has 48,50-54 (skips 49)
    48, 50, 51, 52, 53, 54, 255, // 7 physical keys, only 6 Dygma slots
    // Physical 26-31: Left Row 4 (Ctrl, Win, Alt, small×3) - Dygma has 64-68
    64, 65, 66, 67, 68, 255, // 6 physical keys, only 5 Dygma slots
    // Physical 32-35: Left Thumb (T1-T4) - Dygma has 70-71
    70, 71, 255, 255,
    // Physical 36-39: Gap
    255, 255, 255, 255,
    // Physical 40-46: Right Row 0 (7-=, BS)
    9, 10, 11, 12, 13, 14, 15,
    // Physical 47-53: Right Row 1 (Y-]) - Dygma has 24-30
    24, 25, 26, 27, 28, 29, 30,
    // Physical 54-60: Right Row 2 (H-Enter) - Dygma has 41-46, 31
    41, 42, 43, 44, 45, 46, 31,
    // Physical 61-67: Right Row 3 (N-Shift) - Dygma has 58-63
    58, 59, 60, 61, 62, 63, 255, // 7 physical, 6 Dygma
    // Physical 68-73: Right Row 4 - Dygma has 74-79
    74, 75, 76, 77, 78, 79,
    // Physical 74-75: Gap
    255, 255,
    // Physical 76-79: Right Thumb (T5-T8) - Dygma has 72-73
    72, 73, 255, 255,
];

/// Default labels for base layer (QWERTY) - used when no keymap available
const DEFAULT_LABELS: &[&str] = &[
    // Left main (0-31)
    "ESC", "1", "2", "3", "4", "5", "6",
    "TAB", "Q", "W", "E", "R", "T",
    "CAP", "A", "S", "D", "F", "G",
    "SHF", "\\", "Z", "X", "C", "V", "B",
    "CTL", "WIN", "ALT", "", "", "",
    // Left thumb (32-35)
    "T1", "T2", "T3", "T4",
    // Gap (36-39)
    "", "", "", "",
    // Right main (40-73)
    "7", "8", "9", "0", "-", "=", "BSP",
    "Y", "U", "I", "O", "P", "[", "]",
    "H", "J", "K", "L", ";", "'", "ENT",
    "N", "M", ",", ".", "/", "", "SHF",
    "", "", "ALT", "WIN", "MNU", "CTL",
    // Gap (74-75)
    "", "",
    // Right thumb (76-79)
    "T5", "T6", "T7", "T8",
];

/// Gap between keyboard halves (in key units)
const SPLIT_GAP: f32 = 2.0;

// ============================================================================
// Kaleidoscope Keycode Conversion
// ============================================================================

/// Convert Kaleidoscope keycode to display label
fn keycode_to_label(code: u16, shifted: bool) -> String {
    // Handle shifted versions of keys
    if shifted {
        if let Some(s) = shifted_label(code) {
            return s;
        }
    }

    match code {
        // Transparent/blank
        0 | 65535 => String::new(),

        // HID keycodes (standard USB)
        0x04 => "A".into(), 0x05 => "B".into(), 0x06 => "C".into(), 0x07 => "D".into(),
        0x08 => "E".into(), 0x09 => "F".into(), 0x0A => "G".into(), 0x0B => "H".into(),
        0x0C => "I".into(), 0x0D => "J".into(), 0x0E => "K".into(), 0x0F => "L".into(),
        0x10 => "M".into(), 0x11 => "N".into(), 0x12 => "O".into(), 0x13 => "P".into(),
        0x14 => "Q".into(), 0x15 => "R".into(), 0x16 => "S".into(), 0x17 => "T".into(),
        0x18 => "U".into(), 0x19 => "V".into(), 0x1A => "W".into(), 0x1B => "X".into(),
        0x1C => "Y".into(), 0x1D => "Z".into(),
        0x1E => "1".into(), 0x1F => "2".into(), 0x20 => "3".into(), 0x21 => "4".into(),
        0x22 => "5".into(), 0x23 => "6".into(), 0x24 => "7".into(), 0x25 => "8".into(),
        0x26 => "9".into(), 0x27 => "0".into(),

        0x28 => "ENT".into(), 0x29 => "ESC".into(), 0x2A => "BSP".into(),
        0x2B => "TAB".into(), 0x2C => "SPC".into(),
        0x2D => "-".into(), 0x2E => "=".into(), 0x2F => "[".into(), 0x30 => "]".into(),
        0x31 => "\\".into(), 0x32 => "#".into(), 0x33 => ";".into(), 0x34 => "'".into(),
        0x35 => "`".into(), 0x36 => ",".into(), 0x37 => ".".into(), 0x38 => "/".into(),
        0x39 => "CAP".into(),

        // Function keys
        0x3A => "F1".into(), 0x3B => "F2".into(), 0x3C => "F3".into(), 0x3D => "F4".into(),
        0x3E => "F5".into(), 0x3F => "F6".into(), 0x40 => "F7".into(), 0x41 => "F8".into(),
        0x42 => "F9".into(), 0x43 => "F10".into(), 0x44 => "F11".into(), 0x45 => "F12".into(),

        0x46 => "PRT".into(), 0x47 => "SCR".into(), 0x48 => "PAU".into(),
        0x49 => "INS".into(), 0x4A => "HOM".into(), 0x4B => "PGU".into(),
        0x4C => "DEL".into(), 0x4D => "END".into(), 0x4E => "PGD".into(),

        // Arrow keys
        0x4F => "→".into(), 0x50 => "←".into(), 0x51 => "↓".into(), 0x52 => "↑".into(),

        // Numpad
        0x53 => "NUM".into(), 0x54 => "N/".into(), 0x55 => "N*".into(), 0x56 => "N-".into(),
        0x57 => "N+".into(), 0x58 => "NEN".into(),
        0x59 => "N1".into(), 0x5A => "N2".into(), 0x5B => "N3".into(), 0x5C => "N4".into(),
        0x5D => "N5".into(), 0x5E => "N6".into(), 0x5F => "N7".into(), 0x60 => "N8".into(),
        0x61 => "N9".into(), 0x62 => "N0".into(), 0x63 => "N.".into(),

        // Modifiers (HID codes 0xE0-0xE7)
        0xE0 => "CTL".into(), 0xE1 => "SHF".into(), 0xE2 => "ALT".into(), 0xE3 => "GUI".into(),
        0xE4 => "CTL".into(), 0xE5 => "SHF".into(), 0xE6 => "ALT".into(), 0xE7 => "GUI".into(),

        // Kaleidoscope modifier keys (high byte encodes modifier)
        // Left modifiers: 0x01xx
        c if (0x0100..0x0200).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "CTL".into() } else { format!("C-{}", keycode_to_label(base, false)) }
        }
        // 0x02xx = Left Shift + key
        c if (0x0200..0x0300).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "SHF".into() } else { format!("S-{}", keycode_to_label(base, false)) }
        }
        // 0x04xx = Left Alt + key
        c if (0x0400..0x0500).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "ALT".into() } else { format!("A-{}", keycode_to_label(base, false)) }
        }
        // 0x08xx = Left GUI + key
        c if (0x0800..0x0900).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "GUI".into() } else { format!("G-{}", keycode_to_label(base, false)) }
        }

        // Kaleidoscope layer keys
        // ShiftToLayer: 17408 + layer (0x4400)
        c if (0x4400..0x4420).contains(&c) => format!("→L{}", c - 0x4400),
        // LockLayer: 17664 + layer (0x4500)
        c if (0x4500..0x4520).contains(&c) => format!("L{}", c - 0x4500),
        // MoveToLayer: 17152 + layer (0x4300)
        c if (0x4300..0x4320).contains(&c) => format!("⇒L{}", c - 0x4300),

        // OneShot layers: 49153+ (0xC001)
        c if (0xC001..0xC010).contains(&c) => format!("¹L{}", c - 0xC001),

        // OneShot modifiers
        0xC011 => "¹S".into(),  // OneShot Shift
        0xC012 => "¹C".into(),  // OneShot Ctrl
        0xC014 => "¹A".into(),  // OneShot Alt
        0xC018 => "¹G".into(),  // OneShot GUI

        // Shifted keys (0xC1xx = Shift + HID key)
        c if (0xC100..0xC200).contains(&c) => {
            let base = (c & 0xFF) as u16;
            format!("S-{}", keycode_to_label(base, false))
        }
        // Ctrl keys (0xC2xx = Ctrl + HID key)
        c if (0xC200..0xC300).contains(&c) => {
            let base = (c & 0xFF) as u16;
            format!("C-{}", keycode_to_label(base, false))
        }
        // Alt keys (0xC4xx = Alt + HID key)
        c if (0xC400..0xC500).contains(&c) => {
            let base = (c & 0xFF) as u16;
            format!("A-{}", keycode_to_label(base, false))
        }
        // GUI keys (0xC8xx = GUI + HID key)
        c if (0xC800..0xC900).contains(&c) => {
            let base = (c & 0xFF) as u16;
            format!("G-{}", keycode_to_label(base, false))
        }

        // Media/Consumer keys (0x00E8 range or 0x4Exx in Kaleidoscope)
        0x00E8 => "MUT".into(),
        0x00E9 => "V+".into(),
        0x00EA => "V-".into(),
        0x4E00..=0x4EFF => "MED".into(),

        // Macro keys
        c if (0x5000..0x5100).contains(&c) => format!("M{}", c - 0x5000),

        // DualUse keys (tap/hold)
        c if (0x5100..0x5200).contains(&c) => {
            let layer = (c >> 8) & 0xF;
            let key = c & 0xFF;
            if key == 0 {
                format!("DL{}", layer)
            } else {
                keycode_to_label(key as u16, false)
            }
        }

        // Unknown - show abbreviated hex
        _ => {
            if code < 0x100 {
                format!("x{:02X}", code)
            } else {
                format!("{:04X}", code)
            }
        }
    }
}

/// Get shifted version of a key label
fn shifted_label(code: u16) -> Option<String> {
    Some(match code {
        0x1E => "!".into(),  // 1 -> !
        0x1F => "@".into(),  // 2 -> @
        0x20 => "#".into(),  // 3 -> #
        0x21 => "$".into(),  // 4 -> $
        0x22 => "%".into(),  // 5 -> %
        0x23 => "^".into(),  // 6 -> ^
        0x24 => "&".into(),  // 7 -> &
        0x25 => "*".into(),  // 8 -> *
        0x26 => "(".into(),  // 9 -> (
        0x27 => ")".into(),  // 0 -> )
        0x2D => "_".into(),  // - -> _
        0x2E => "+".into(),  // = -> +
        0x2F => "{".into(),  // [ -> {
        0x30 => "}".into(),  // ] -> }
        0x31 => "|".into(),  // \ -> |
        0x33 => ":".into(),  // ; -> :
        0x34 => "\"".into(), // ' -> "
        0x35 => "~".into(),  // ` -> ~
        0x36 => "<".into(),  // , -> <
        0x37 => ">".into(),  // . -> >
        0x38 => "?".into(),  // / -> ?
        _ => return None,
    })
}

/// Width of each half
const HALF_WIDTH: f32 = 8.0;

/// Total height
const TOTAL_HEIGHT: f32 = 7.0;

// ============================================================================
// evdev Key Detection
// ============================================================================

/// Find keyboard input devices via evdev
fn find_keyboard_devices() -> Vec<evdev::Device> {
    let mut keyboards = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("event") {
                    if let Ok(device) = evdev::Device::open(&path) {
                        if device.supported_keys().is_some_and(|keys| {
                            keys.contains(evdev::Key::KEY_A) && keys.contains(evdev::Key::KEY_SPACE)
                        }) {
                            keyboards.push(device);
                        }
                    }
                }
            }
        }
    }
    keyboards
}

/// Map evdev key code to label string
fn evdev_key_to_label(key: evdev::Key) -> Option<&'static str> {
    use evdev::Key;
    Some(match key {
        Key::KEY_ESC => "ESC",
        Key::KEY_1 => "1", Key::KEY_2 => "2", Key::KEY_3 => "3", Key::KEY_4 => "4", Key::KEY_5 => "5",
        Key::KEY_6 => "6", Key::KEY_7 => "7", Key::KEY_8 => "8", Key::KEY_9 => "9", Key::KEY_0 => "0",
        Key::KEY_MINUS => "-", Key::KEY_EQUAL => "=", Key::KEY_BACKSPACE => "BSP",
        Key::KEY_TAB => "TAB",
        Key::KEY_Q => "Q", Key::KEY_W => "W", Key::KEY_E => "E", Key::KEY_R => "R", Key::KEY_T => "T",
        Key::KEY_Y => "Y", Key::KEY_U => "U", Key::KEY_I => "I", Key::KEY_O => "O", Key::KEY_P => "P",
        Key::KEY_LEFTBRACE => "[", Key::KEY_RIGHTBRACE => "]", Key::KEY_BACKSLASH => "\\",
        Key::KEY_CAPSLOCK => "CAP",
        Key::KEY_A => "A", Key::KEY_S => "S", Key::KEY_D => "D", Key::KEY_F => "F", Key::KEY_G => "G",
        Key::KEY_H => "H", Key::KEY_J => "J", Key::KEY_K => "K", Key::KEY_L => "L",
        Key::KEY_SEMICOLON => ";", Key::KEY_APOSTROPHE => "'", Key::KEY_ENTER => "ENT",
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => "SHF",
        Key::KEY_Z => "Z", Key::KEY_X => "X", Key::KEY_C => "C", Key::KEY_V => "V", Key::KEY_B => "B",
        Key::KEY_N => "N", Key::KEY_M => "M",
        Key::KEY_COMMA => ",", Key::KEY_DOT => ".", Key::KEY_SLASH => "/",
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => "CTL",
        Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => "WIN",
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => "ALT",
        Key::KEY_SPACE => "SPC",
        Key::KEY_GRAVE => "`",
        Key::KEY_F1 => "F1", Key::KEY_F2 => "F2", Key::KEY_F3 => "F3", Key::KEY_F4 => "F4",
        Key::KEY_F5 => "F5", Key::KEY_F6 => "F6", Key::KEY_F7 => "F7", Key::KEY_F8 => "F8",
        Key::KEY_F9 => "F9", Key::KEY_F10 => "F10", Key::KEY_F11 => "F11", Key::KEY_F12 => "F12",
        Key::KEY_COMPOSE => "MNU",
        // Additional keys that thumb clusters might use
        Key::KEY_DELETE => "DEL",
        Key::KEY_HOME => "HOM",
        Key::KEY_END => "END",
        Key::KEY_PAGEUP => "PGU",
        Key::KEY_PAGEDOWN => "PGD",
        Key::KEY_INSERT => "INS",
        Key::KEY_UP => "UP",
        Key::KEY_DOWN => "DN",
        Key::KEY_LEFT => "LT",
        Key::KEY_RIGHT => "RT",
        Key::KEY_PRINT => "PRT",
        Key::KEY_SCROLLLOCK => "SCR",
        Key::KEY_PAUSE => "PAU",
        // Media keys
        Key::KEY_MUTE => "MUT",
        Key::KEY_VOLUMEDOWN => "V-",
        Key::KEY_VOLUMEUP => "V+",
        Key::KEY_PLAYPAUSE => "PLA",
        Key::KEY_STOPCD => "STP",
        Key::KEY_PREVIOUSSONG => "PRV",
        Key::KEY_NEXTSONG => "NXT",
        // Application keys
        Key::KEY_HOMEPAGE => "WWW",
        Key::KEY_MAIL => "MAI",
        Key::KEY_CALC => "CAL",
        Key::KEY_FILE => "FIL",
        _ => return None,
    })
}

/// Get raw key code as string for debugging unknown keys
fn evdev_key_raw(key: evdev::Key) -> String {
    format!("K{}", key.0)
}

// ============================================================================
// Focus Protocol (Serial Communication)
// ============================================================================

/// Connection to Dygma keyboard via Focus protocol
struct FocusConnection {
    port: Box<dyn serialport::SerialPort>,
}

impl FocusConnection {
    /// Try to connect to a Dygma Raise keyboard
    fn connect(port_path: Option<&PathBuf>) -> Option<Self> {
        // If port specified, use it directly
        if let Some(path) = port_path {
            if let Ok(port) = serialport::new(path.to_string_lossy(), 115200)
                .timeout(std::time::Duration::from_millis(500))
                .open()
            {
                return Some(Self { port });
            }
        }

        // Auto-detect: find Dygma by USB VID/PID
        let ports = serialport::available_ports().ok()?;
        for port_info in &ports {
            if let serialport::SerialPortType::UsbPort(usb) = &port_info.port_type {
                // Dygma VID: 0x1209, Raise PID: 0x2201, Defy PID: 0x2200
                if usb.vid == 0x1209 && (usb.pid == 0x2201 || usb.pid == 0x2200) {
                    if let Ok(port) = serialport::new(&port_info.port_name, 115200)
                        .timeout(std::time::Duration::from_millis(500))
                        .open()
                    {
                        return Some(Self { port });
                    }
                }
            }
        }
        None
    }

    /// Query available commands from keyboard
    #[allow(dead_code)]
    fn help(&mut self) -> Option<String> {
        self.command("help")
    }

    /// Query the full keymap (all layers, all keys)
    /// Returns 10 layers x 80 keys = 800 keycodes
    fn keymap(&mut self) -> Option<Vec<Vec<u16>>> {
        use std::io::{Read, Write};

        // Clear any pending data
        let _ = self.port.clear(serialport::ClearBuffer::Input);

        // Send command (Dygma uses keymap.custom, not keymap)
        writeln!(self.port, "keymap.custom").ok()?;
        self.port.flush().ok()?;

        // Keymap is large - give it more time
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Read raw bytes (keymap response is ~4KB)
        let mut buffer = vec![0u8; 8192];
        let mut response = String::new();

        // Read in chunks until no more data
        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buffer[..n]) {
                        response.push_str(s);
                        // Check for Focus terminator
                        if response.contains("\r\n.\r\n") {
                            break;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(_) => break,
            }
        }

        // Parse space-separated keycodes (ignore the trailing ".")
        let keycodes: Vec<u16> = response
            .split_whitespace()
            .filter(|s| *s != ".")
            .filter_map(|s| s.parse().ok())
            .collect();

        // Split into 10 layers of 80 keys each
        if keycodes.len() >= 800 {
            let mut layers = Vec::with_capacity(10);
            for layer in 0..10 {
                let start = layer * 80;
                let end = start + 80;
                layers.push(keycodes[start..end].to_vec());
            }
            Some(layers)
        } else if keycodes.len() >= 80 {
            // At least one layer - use what we got
            let num_layers = keycodes.len() / 80;
            let mut layers = Vec::with_capacity(num_layers);
            for layer in 0..num_layers {
                let start = layer * 80;
                let end = start + 80;
                layers.push(keycodes[start..end].to_vec());
            }
            Some(layers)
        } else {
            None
        }
    }

    /// Send command and read response
    fn command(&mut self, cmd: &str) -> Option<String> {
        use std::io::{BufRead, BufReader, Write};

        // Clear any pending data
        let _ = self.port.clear(serialport::ClearBuffer::Input);

        // Send command
        writeln!(self.port, "{}", cmd).ok()?;
        self.port.flush().ok()?;

        // Small delay for keyboard to respond
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Read response
        let mut reader = BufReader::new(&mut *self.port);
        let mut response = String::new();

        // Read lines until we get "." (Focus protocol terminator)
        for _ in 0..10 {  // Max 10 lines to prevent infinite loop
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,  // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed == "." {
                        break;
                    }
                    if !response.is_empty() {
                        response.push(' ');
                    }
                    response.push_str(trimmed);
                }
                Err(_) => break,
            }
        }

        if response.is_empty() {
            None
        } else {
            Some(response)
        }
    }

    /// Query active layer - returns highest active layer number
    /// layer.state returns 32 space-separated values (0=inactive, 1=active)
    fn active_layer(&mut self) -> Option<u8> {
        let response = self.command("layer.state")?;

        // Parse "0 0 1 0 0 ..." format - find highest active layer
        let states: Vec<bool> = response
            .split_whitespace()
            .filter_map(|s| s.parse::<u8>().ok())
            .map(|v| v != 0)
            .collect();

        // Return the highest active layer (layer stacking means multiple can be active)
        states.iter()
            .enumerate()
            .filter(|(_, &active)| active)
            .map(|(i, _)| i as u8)
            .last()
    }
}

// ============================================================================
// Main Visualization
// ============================================================================

pub fn run(config: DygmaConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Electric default

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Key heat tracking
    let key_heat: Arc<Mutex<HashMap<String, f32>>> = Arc::new(Mutex::new(HashMap::new()));
    let running = Arc::new(AtomicBool::new(true));
    let current_layer = Arc::new(AtomicU8::new(0));
    let shift_held = Arc::new(AtomicBool::new(false));

    // Try to connect to keyboard via Focus protocol
    let mut focus = FocusConnection::connect(config.port.as_ref());
    let has_focus = focus.is_some();
    let mut debug_info = String::new();

    // Query keymap from keyboard (10 layers x 80 keys)
    let keymap: Option<Vec<Vec<u16>>> = if let Some(ref mut f) = focus {
        f.keymap()
    } else {
        None
    };

    // Store keymap status for debug display
    let keymap_status = match &keymap {
        Some(km) => {
            let total_keys: usize = km.iter().map(|l| l.len()).sum();
            format!("KM:{}/{}", km.len(), total_keys)
        }
        None => "KM:FAIL".to_string(),
    };

    // Spawn evdev listener threads
    let keyboards = find_keyboard_devices();
    let has_evdev = !keyboards.is_empty();
    let mut handles = Vec::new();

    // Shared debug key info
    let last_key = Arc::new(Mutex::new(String::new()));

    for mut device in keyboards {
        let heat_clone = Arc::clone(&key_heat);
        let running_clone = Arc::clone(&running);
        let last_key_clone = Arc::clone(&last_key);
        let shift_clone = Arc::clone(&shift_held);
        let debug_mode = config.debug;

        let handle = std::thread::spawn(move || {
            use std::os::unix::io::AsRawFd;
            let fd = device.as_raw_fd();
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }

            while running_clone.load(Ordering::Relaxed) {
                if let Ok(events) = device.fetch_events() {
                    for ev in events {
                        if let evdev::InputEventKind::Key(key) = ev.kind() {
                            // Track shift state
                            if key == evdev::Key::KEY_LEFTSHIFT || key == evdev::Key::KEY_RIGHTSHIFT {
                                shift_clone.store(ev.value() != 0, Ordering::Relaxed);
                            }

                            if ev.value() == 1 || ev.value() == 2 {
                                // Get label or raw code for unknown keys
                                let label = evdev_key_to_label(key)
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| evdev_key_raw(key));

                                if let Ok(mut heat) = heat_clone.lock() {
                                    heat.insert(label.clone(), 1.0);
                                }

                                // Store for debug display
                                if debug_mode {
                                    if let Ok(mut lk) = last_key_clone.lock() {
                                        *lk = format!("{:?} -> {}", key, label);
                                    }
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    // Layer query interval
    let mut layer_query_timer = 0.0f32;

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        // Handle input
        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
            // Manual layer selection with number keys (when no Focus)
            if !has_focus {
                if let KeyCode::Char(c) = code {
                    if c.is_ascii_digit() {
                        // Don't override speed - use Ctrl+number for layer?
                    }
                }
            }
        }

        // Query layer from keyboard periodically
        layer_query_timer += state.speed;
        if layer_query_timer > 0.5 {
            layer_query_timer = 0.0;
            if let Some(ref mut f) = focus {
                // Get raw response for debug
                if config.debug {
                    if let Some(resp) = f.command("layer.state") {
                        debug_info = format!("layer.state: {}", resp);
                        // Parse to find active layer
                        let states: Vec<bool> = resp
                            .split_whitespace()
                            .filter_map(|s| s.parse::<u8>().ok())
                            .map(|v| v != 0)
                            .collect();
                        if let Some(layer) = states.iter()
                            .enumerate()
                            .filter(|(_, &active)| active)
                            .map(|(i, _)| i as u8)
                            .last() {
                            current_layer.store(layer, Ordering::Relaxed);
                        }
                    } else {
                        debug_info = "layer.state: no response".to_string();
                    }
                } else if let Some(layer) = f.active_layer() {
                    current_layer.store(layer, Ordering::Relaxed);
                }
            }
        }

        // Decay heat values
        if let Ok(mut heat) = key_heat.lock() {
            for v in heat.values_mut() {
                *v = (*v - state.speed * 3.0).max(0.0);
            }
            heat.retain(|_, v| *v > 0.0);
        }

        let heat_snapshot = key_heat.lock().map(|h| h.clone()).unwrap_or_default();
        let layer = current_layer.load(Ordering::Relaxed);
        let shifted = shift_held.load(Ordering::Relaxed);

        term.clear();

        let w = width as f32;
        let h = height as f32;

        // Calculate scaling
        let total_width = HALF_WIDTH * 2.0 + SPLIT_GAP;
        let scale_x = (w * 0.9) / total_width;
        let scale_y = (h * 0.8) / TOTAL_HEIGHT;
        let scale = scale_x.min(scale_y).min(4.0); // Cap scale

        let key_char_width = (scale * 3.0) as usize;

        // Center the layout
        let layout_width = (total_width * scale * 3.0) as usize;
        let layout_height = (TOTAL_HEIGHT * scale) as usize;
        let start_x = (width as usize).saturating_sub(layout_width) / 2;
        let start_y = (height as usize).saturating_sub(layout_height) / 2;

        // Draw layer indicator
        let layer_text = format!("[ Layer {} ]", layer);
        let layer_x = (width as usize - layer_text.len()) / 2;
        let (layer_color, _) = scheme_color(state.color_scheme, 2, true);
        term.set_str(layer_x as i32, 0, &layer_text, Some(layer_color), true);

        // Status line
        let connection_status = if has_focus { "Focus" } else if has_evdev { "evdev" } else { "none" };
        let (status_color, _) = scheme_color(state.color_scheme, 0, false);

        if config.debug {
            // Show last key pressed + keymap status
            let last_key_info = last_key.lock().map(|k| k.clone()).unwrap_or_default();
            let debug_status = format!("[{}] {} | Key: {} | {}", connection_status, keymap_status, last_key_info, debug_info);
            term.set_str(1, height as i32 - 1, &debug_status, Some(status_color), false);
        } else {
            term.set_str(1, height as i32 - 1, connection_status, Some(status_color), false);
        }

        // Draw left half
        draw_half(
            &mut term,
            LEFT_MAIN,
            LEFT_THUMB,
            start_x,
            start_y + 1,
            scale,
            key_char_width,
            &heat_snapshot,
            state.color_scheme,
            keymap.as_ref(),
            layer,
            shifted,
        );

        // Draw right half (offset by left width + gap)
        let right_offset = ((HALF_WIDTH + SPLIT_GAP) * scale * 3.0) as usize;
        draw_half(
            &mut term,
            RIGHT_MAIN,
            RIGHT_THUMB,
            start_x + right_offset,
            start_y + 1,
            scale,
            key_char_width,
            &heat_snapshot,
            state.color_scheme,
            keymap.as_ref(),
            layer,
            shifted,
        );

        term.present()?;
        term.sleep(state.speed);
    }

    // Cleanup
    running.store(false, Ordering::Relaxed);
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

fn draw_half(
    term: &mut Terminal,
    main_keys: &[(usize, KeyPos)],
    thumb_keys: &[(usize, KeyPos)],
    base_x: usize,
    base_y: usize,
    scale: f32,
    _key_char_width: usize,
    heat: &HashMap<String, f32>,
    scheme: u8,
    keymap: Option<&Vec<Vec<u16>>>,
    layer: u8,
    shifted: bool,
) {
    for (idx, pos) in main_keys.iter().chain(thumb_keys.iter()) {
        let x = base_x + (pos.x * scale * 3.0) as usize;
        let y = base_y + (pos.y * scale) as usize;
        let w = ((pos.w * scale * 3.0) as usize).max(1);

        // Get label from keymap (current layer) or fall back to defaults
        let label: String = if let Some(km) = keymap {
            // Map physical index to Dygma keymap index
            let keymap_idx = PHYSICAL_TO_KEYMAP.get(*idx).copied().unwrap_or(255);
            if keymap_idx == 255 {
                // Unmapped position
                DEFAULT_LABELS.get(*idx).copied().unwrap_or("").to_string()
            } else {
                // Try current layer first, fall back to lower layers for transparent keys
                let mut found_label = String::new();
                for check_layer in (0..=layer as usize).rev() {
                    if let Some(layer_map) = km.get(check_layer) {
                        if let Some(&keycode) = layer_map.get(keymap_idx) {
                            let lbl = keycode_to_label(keycode, shifted);
                            if !lbl.is_empty() {
                                found_label = lbl;
                                break;
                            }
                        }
                    }
                }
                found_label
            }
        } else {
            DEFAULT_LABELS.get(*idx).copied().unwrap_or("").to_string()
        };

        if label.is_empty() {
            continue;
        }

        // Get heat for this key (match by label)
        let key_heat = heat.get(&label).copied().unwrap_or(0.0);

        draw_key(term, x, y, w, &label, key_heat, scheme);
    }
}

fn draw_key(
    term: &mut Terminal,
    x: usize,
    y: usize,
    width: usize,
    label: &str,
    heat: f32,
    scheme: u8,
) {
    let intensity = if heat > 0.7 { 3 }
        else if heat > 0.3 { 2 }
        else if heat > 0.0 { 1 }
        else { 0 };

    let (color, bold) = scheme_color(scheme, intensity, heat > 0.7);

    // Truncate and center label
    let display: String = label.chars().take(width).collect();
    let padding = width.saturating_sub(display.len()) / 2;

    term.set_str(
        (x + padding) as i32,
        y as i32,
        &display,
        Some(color),
        bold,
    );
}
