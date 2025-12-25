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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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

/// Left half main keys (Dygma Raise 2)
/// Columnar stagger layout matching physical keyboard
/// Outer edge (left) aligned at x=0
const LEFT_MAIN: &[(usize, KeyPos)] = &[
    // Row 0 (number row): ESC, 1-6 (7 keys)
    (0,  KeyPos::new(0.0, 0.0, 1.0)),   // ESC
    (1,  KeyPos::new(1.0, 0.0, 1.0)),   // 1
    (2,  KeyPos::new(2.0, 0.0, 1.0)),   // 2
    (3,  KeyPos::new(3.0, 0.0, 1.0)),   // 3
    (4,  KeyPos::new(4.0, 0.0, 1.0)),   // 4
    (5,  KeyPos::new(5.0, 0.0, 1.0)),   // 5
    (6,  KeyPos::new(6.0, 0.0, 1.0)),   // 6
    // Row 1 (top letter): Tab, Q-T (6 keys) - Tab wider
    (7,  KeyPos::new(0.0, 1.0, 1.5)),   // Tab (1.5 wide)
    (8,  KeyPos::new(1.5, 1.0, 1.0)),   // Q
    (9,  KeyPos::new(2.5, 1.0, 1.0)),   // W
    (10, KeyPos::new(3.5, 1.0, 1.0)),   // E
    (11, KeyPos::new(4.5, 1.0, 1.0)),   // R
    (12, KeyPos::new(5.5, 1.0, 1.0)),   // T
    // Row 2 (home): Caps, A-G (6 keys) - Caps wider
    (13, KeyPos::new(0.0, 2.0, 1.5)),   // Caps (1.5 wide)
    (14, KeyPos::new(1.5, 2.0, 1.0)),   // A
    (15, KeyPos::new(2.5, 2.0, 1.0)),   // S
    (16, KeyPos::new(3.5, 2.0, 1.0)),   // D
    (17, KeyPos::new(4.5, 2.0, 1.0)),   // F
    (18, KeyPos::new(5.5, 2.0, 1.0)),   // G
    // Row 3 (bottom): Shift, Z-B (6 keys) - Shift wider
    (19, KeyPos::new(0.0, 3.0, 1.5)),   // Shift (1.5 wide)
    (20, KeyPos::new(1.5, 3.0, 1.0)),   // Z
    (21, KeyPos::new(2.5, 3.0, 1.0)),   // X
    (22, KeyPos::new(3.5, 3.0, 1.0)),   // C
    (23, KeyPos::new(4.5, 3.0, 1.0)),   // V
    (24, KeyPos::new(5.5, 3.0, 1.0)),   // B
    // Row 4 (modifiers): Ctrl, Cmd, Alt (3 keys)
    (25, KeyPos::new(0.0, 4.0, 1.0)),   // Ctrl
    (26, KeyPos::new(1.0, 4.0, 1.0)),   // Cmd/Meta
    (27, KeyPos::new(2.0, 4.0, 1.0)),   // Alt
];

/// Left thumb cluster - 4 keys in 2x2 arrangement
const LEFT_THUMB: &[(usize, KeyPos)] = &[
    (28, KeyPos::new(4.0, 4.0, 1.0)),   // T1 (upper left)
    (29, KeyPos::new(5.0, 4.0, 1.0)),   // T2 (upper right)
    (30, KeyPos::new(4.0, 5.0, 1.0)),   // T3 (lower left)
    (31, KeyPos::new(5.0, 5.0, 1.0)),   // T4 (lower right)
];

/// Right half main keys
/// Outer edge (right) aligned at x=7
const RIGHT_MAIN: &[(usize, KeyPos)] = &[
    // Row 0 (number row): 7-=, Backspace (7 keys) - right-aligned with row 1
    (32, KeyPos::new(1.0, 0.0, 1.0)),   // 7
    (33, KeyPos::new(2.0, 0.0, 1.0)),   // 8
    (34, KeyPos::new(3.0, 0.0, 1.0)),   // 9
    (35, KeyPos::new(4.0, 0.0, 1.0)),   // 0
    (36, KeyPos::new(5.0, 0.0, 1.0)),   // -
    (37, KeyPos::new(6.0, 0.0, 1.0)),   // =
    (38, KeyPos::new(7.0, 0.0, 1.0)),   // Backspace
    // Row 1 (top letter): Y U I O P [ ] \ (8 keys - backslash on right edge)
    (39, KeyPos::new(0.0, 1.0, 1.0)),   // Y
    (40, KeyPos::new(1.0, 1.0, 1.0)),   // U
    (41, KeyPos::new(2.0, 1.0, 1.0)),   // I
    (42, KeyPos::new(3.0, 1.0, 1.0)),   // O
    (43, KeyPos::new(4.0, 1.0, 1.0)),   // P
    (44, KeyPos::new(5.0, 1.0, 1.0)),   // [
    (45, KeyPos::new(6.0, 1.0, 1.0)),   // ]
    (46, KeyPos::new(7.0, 1.0, 1.0)),   // \ (backslash)
    // Row 2 (home): H J K L ; ' Enter (7 keys - Enter at end)
    (47, KeyPos::new(1.0, 2.0, 1.0)),   // H
    (48, KeyPos::new(2.0, 2.0, 1.0)),   // J
    (49, KeyPos::new(3.0, 2.0, 1.0)),   // K
    (50, KeyPos::new(4.0, 2.0, 1.0)),   // L
    (51, KeyPos::new(5.0, 2.0, 1.0)),   // ;
    (52, KeyPos::new(6.0, 2.0, 1.0)),   // '
    (53, KeyPos::new(7.0, 2.0, 1.0)),   // Enter
    // Row 3 (bottom): N-/, Shift (6 keys) - right-aligned with rows above
    (54, KeyPos::new(2.0, 3.0, 1.0)),   // N
    (55, KeyPos::new(3.0, 3.0, 1.0)),   // M
    (56, KeyPos::new(4.0, 3.0, 1.0)),   // ,
    (57, KeyPos::new(5.0, 3.0, 1.0)),   // .
    (58, KeyPos::new(6.0, 3.0, 1.0)),   // /
    (59, KeyPos::new(7.0, 3.0, 1.0)),   // Shift
    // Row 4 (modifiers): Alt, FN, Meta, Ctrl (4 keys)
    (60, KeyPos::new(4.0, 4.0, 1.0)),   // Alt
    (61, KeyPos::new(5.0, 4.0, 1.0)),   // FN
    (62, KeyPos::new(6.0, 4.0, 1.0)),   // Meta
    (63, KeyPos::new(7.0, 4.0, 1.0)),   // Ctrl
];

/// Right thumb cluster - 4 keys in 2x2 arrangement
const RIGHT_THUMB: &[(usize, KeyPos)] = &[
    (64, KeyPos::new(1.0, 4.0, 1.0)),   // T5 (upper left)
    (65, KeyPos::new(2.0, 4.0, 1.0)),   // T6 (upper right)
    (66, KeyPos::new(1.0, 5.0, 1.0)),   // T7 (lower left)
    (67, KeyPos::new(2.0, 5.0, 1.0)),   // T8 (lower right)
];

/// Map from physical key index (our layout) to Dygma keymap index
/// Based on official Dygma Raise ANSI keymap from Bazecor source
/// Array index = our physical position (0-67), value = Dygma keymap index
/// Keymap uses 16-column grid: keyIndex = row * 16 + col
const PHYSICAL_TO_KEYMAP: &[usize] = &[
    // LEFT HALF (indices 0-31) - 32 keys
    // Row 0: ESC, 1-6 (physical 0-6) → keymap row 0, cols 0-6
    0, 1, 2, 3, 4, 5, 6,
    // Row 1: Tab, Q-T (physical 7-12) → keymap row 1, cols 0-5
    16, 17, 18, 19, 20, 21,
    // Row 2: Caps, A-G (physical 13-18) → keymap row 2, cols 0-5
    32, 33, 34, 35, 36, 37,
    // Row 3: Shift, Z-B (physical 19-24) → keymap 48 for shift, skip 49 (ISO key)
    48, 50, 51, 52, 53, 54,
    // Row 4: Ctrl, Meta, Alt (physical 25-27) → keymap row 4, cols 0-2
    64, 65, 66,
    // Thumb: T1-T4 (physical 28-31) → keymap indices
    67, 68, 70, 71,
    // RIGHT HALF (indices 32-67) - 36 keys
    // Row 0: 7-=, Backspace (physical 32-38) → keymap row 0, cols 9-15
    9, 10, 11, 12, 13, 14, 15,
    // Row 1: Y U I O P [ ] \ (physical 39-46) → keymap row 1, cols 8-15
    24, 25, 26, 27, 28, 29, 30, 47,
    // Row 2: H J K L ; ' Enter (physical 47-53) → keymap row 2, cols 9-15
    41, 42, 43, 44, 45, 46, 31,
    // Row 3: N-/, Shift (physical 54-59) → keymap row 3, cols 10-15
    58, 59, 60, 61, 62, 63,
    // Row 4: Alt, FN, Meta, Ctrl (physical 60-63) → keymap row 4, cols 12-15
    76, 77, 78, 79,
    // Thumb: T5-T8 (physical 64-67) → keymap indices (T5/T7 and T6/T8 swapped)
    74, 75, 72, 73,
];

/// Default labels for base layer (QWERTY) - sequential indices matching layout
const DEFAULT_LABELS: &[&str] = &[
    // Left half (0-31) - 32 keys
    // Row 0: ESC, 1-6 (indices 0-6)
    "ESC", "1", "2", "3", "4", "5", "6",
    // Row 1: Tab, Q-T (indices 7-12)
    "TAB", "Q", "W", "E", "R", "T",
    // Row 2: Caps, A-G (indices 13-18)
    "CAP", "A", "S", "D", "F", "G",
    // Row 3: Shift, Z-B (indices 19-24)
    "SHF", "Z", "X", "C", "V", "B",
    // Row 4: Ctrl, Meta, Alt (indices 25-27)
    "CTL", "MET", "ALT",
    // Left thumb: T1-T4 (indices 28-31)
    "T1", "T2", "T3", "T4",
    // Right half (32-67) - 36 keys
    // Row 0: 7-=, Backspace (indices 32-38)
    "7", "8", "9", "0", "-", "=", "BSP",
    // Row 1: Y U I O P [ ] \ (indices 39-46) - Backslash on right edge
    "Y", "U", "I", "O", "P", "[", "]", "\\",
    // Row 2: H J K L ; ' Enter (indices 47-53) - Enter at end of home row
    "H", "J", "K", "L", ";", "'", "ENT",
    // Row 3: N-/, Shift (indices 54-59)
    "N", "M", ",", ".", "/", "SHF",
    // Row 4: Alt, FN, Meta, Ctrl (indices 60-63)
    "ALT", "FN", "MET", "CTL",
    // Right thumb: T5-T8 (indices 64-67)
    "T5", "T6", "T7", "T8",
];

/// Gap between keyboard halves (in key units)
const SPLIT_GAP: f32 = 2.5;

// ============================================================================
// Kaleidoscope Keycode Conversion
// ============================================================================

/// Convert Kaleidoscope keycode to display label
fn keycode_to_label(code: u16, shifted: bool) -> String {
    // Letters: uppercase when shifted, lowercase otherwise
    if (0x04..=0x1D).contains(&code) {
        let letter = (b'a' + (code - 0x04) as u8) as char;
        return if shifted {
            letter.to_ascii_uppercase().to_string()
        } else {
            letter.to_string()
        };
    }

    // Numbers and common punctuation: handle shift inline
    if (0x1E..=0x38).contains(&code) {
        return if shifted {
            shifted_label(code).unwrap_or_else(|| unshifted_label(code))
        } else {
            unshifted_label(code)
        };
    }

    match code {
        // Transparent/blank
        0 | 65535 => String::new(),

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
        0x64 => "ISO".into(), 0x65 => "APP".into(), 0x66 => "PWR".into(), 0x67 => "KP=".into(),

        // Extended function keys (F13-F24)
        0x68 => "F13".into(), 0x69 => "F14".into(), 0x6A => "F15".into(), 0x6B => "F16".into(),
        0x6C => "F17".into(), 0x6D => "F18".into(), 0x6E => "F19".into(), 0x6F => "F20".into(),
        0x70 => "F21".into(), 0x71 => "F22".into(), 0x72 => "F23".into(), 0x73 => "F24".into(),

        // Modifiers (HID codes 0xE0-0xE7)
        0xE0 => "CTL".into(), 0xE1 => "SHF".into(), 0xE2 => "ALT".into(), 0xE3 => "GUI".into(),
        0xE4 => "CTL".into(), 0xE5 => "SHF".into(), 0xE6 => "ALT".into(), 0xE7 => "GUI".into(),

        // Kaleidoscope modifier keys (high byte encodes modifier)
        // Bazecor modifier offsets: Ctrl=0x0100, Alt=0x0200, AltGr=0x0400, Shift=0x0800, GUI=0x1000
        // 0x01xx = Ctrl + key
        c if (0x0100..0x0200).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "CTL".into() } else { format!("C-{}", keycode_to_label(base, false)) }
        }
        // 0x02xx = Alt + key
        c if (0x0200..0x0400).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "ALT".into() } else { format!("A-{}", keycode_to_label(base, false)) }
        }
        // 0x04xx = AltGr + key
        c if (0x0400..0x0800).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "AGR".into() } else { format!("R-{}", keycode_to_label(base, false)) }
        }
        // 0x08xx = Shift + key
        c if (0x0800..0x1000).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "SHF".into() } else { shifted_label(base).unwrap_or_else(|| format!("S-{}", keycode_to_label(base, false))) }
        }
        // 0x10xx = GUI + key
        c if (0x1000..0x1100).contains(&c) => {
            let base = (c & 0xFF) as u16;
            if base == 0 { "GUI".into() } else { format!("G-{}", keycode_to_label(base, false)) }
        }

        // Kaleidoscope/Bazecor layer keys
        // ShiftToLayer: 0x4429 + layer (hold to activate) - empirically determined from Bazecor
        c if (0x4429..0x443F).contains(&c) => format!(">L{}", c - 0x4429),
        // LockLayer: base 0x4400 (Bazecor: 17408)
        c if (0x4400..0x4410).contains(&c) => format!("=L{}", c - 0x4400),
        // MoveToLayer: base 0x4454 (Bazecor: 17492)
        c if (0x4454..0x4464).contains(&c) => format!("+L{}", c - 0x4454),
        // Extended MoveToLayer range (in case of different encoding)
        c if (0x4449..0x4454).contains(&c) => format!("+L{}", c - 0x4449),
        // Additional layer ranges (legacy/alternate encodings)
        c if (0x4410..0x4420).contains(&c) => format!("=L{}", c - 0x4410),
        c if (0x443F..0x4449).contains(&c) => format!("*L{}", c - 0x443F),  // Unknown layer op

        // OneShot layers: 49153+ (0xC001)
        c if (0xC001..0xC010).contains(&c) => format!("1L{}", c - 0xC001),

        // OneShot modifiers
        0xC011 => "1S".into(),   // OneShot Shift
        0xC012 => "1C".into(),   // OneShot Ctrl
        0xC014 => "1A".into(),   // OneShot Alt
        0xC018 => "1G".into(),   // OneShot GUI

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

        // LED Effect keys (0x4300-0x4302)
        0x4300 => "LED>".into(),  // Next LED effect
        0x4301 => "LED<".into(),  // Previous LED effect
        0x4302 => "LED~".into(),  // Toggle LED effect

        // Media/Consumer keys - HID format (0x00xx)
        0x00E8 => "MUT".into(),
        0x00E9 => "V+".into(),
        0x00EA => "V-".into(),
        0x00B5 => ">>".into(),   // Next track
        0x00B6 => "<<".into(),   // Previous track
        0x00B7 => "STP".into(),  // Stop
        0x00CD => "P/P".into(),  // Play/Pause

        // Media/Consumer keys - Bazecor format (decimal values from Bazecor source)
        0x4CE2 => "MUT".into(),  // 19682 = Mute
        0x58B5 => ">>".into(),   // 22709 = Next track
        0x58B6 => "<<".into(),   // 22710 = Previous track
        0x58B7 => "STP".into(),  // 22711 = Stop
        0x58CD => "P/P".into(),  // 22733 = Play/Pause
        0x5CE9 => "V+".into(),   // 23785 = Volume up
        0x5CEA => "V-".into(),   // 23786 = Volume down
        0x58B8 => "EJT".into(),  // 22712 = Eject
        0x4878 => "CAM".into(),  // 18552 = Camera
        0x5C6F => "BR+".into(),  // 23663 = Brightness up
        0x5C70 => "BR-".into(),  // 23664 = Brightness down
        0x4992 => "CAL".into(),  // 18834 = Calculator
        0x58B9 => "SHF".into(),  // 22713 = Shuffle

        0x4E00..=0x4EFF => "MED".into(),

        // Macro keys (Bazecor: 53852 = 0xD24C base)
        c if (0x5000..0x5100).contains(&c) => format!("M{}", c - 0x5000),
        // Extended macro range for Bazecor
        c if (0xD24C..0xD2CC).contains(&c) => format!("M{}", c - 0xD24C),

        // DualUse modifier keys (Bazecor format: 0xC031-0xC5B1)
        // These are tap-key/hold-modifier combinations
        c if (0xC031..0xC040).contains(&c) => "D/C".into(),  // DualUse Ctrl
        c if (0xC0C1..0xC0D0).contains(&c) => "D/S".into(),  // DualUse Shift
        c if (0xC149..0xC158).contains(&c) => "D/A".into(),  // DualUse Alt
        c if (0xC1D1..0xC1E0).contains(&c) => "D/G".into(),  // DualUse GUI
        c if (0xC5B1..0xC5C0).contains(&c) => "D/R".into(),  // DualUse AltGr

        // DualUse layer keys (Bazecor format: 0xC812-0xCCE2)
        c if (0xC812..0xC8C2).contains(&c) => "DL1".into(),
        c if (0xC8C2..0xC972).contains(&c) => "DL2".into(),
        c if (0xC972..0xCA22).contains(&c) => "DL3".into(),
        c if (0xCA22..0xCAD2).contains(&c) => "DL4".into(),
        c if (0xCAD2..0xCB82).contains(&c) => "DL5".into(),
        c if (0xCB82..0xCC32).contains(&c) => "DL6".into(),
        c if (0xCC32..0xCCE2).contains(&c) => "DL7".into(),
        c if (0xCCE2..0xCD92).contains(&c) => "DL8".into(),

        // TapDance keys (Bazecor: 53267 = 0xD033 base, 64 slots)
        c if (0xD033..0xD073).contains(&c) => format!("T{}", c - 0xD033),

        // Legacy DualUse keys (0x51xx format)
        c if (0x5100..0x5200).contains(&c) => {
            let layer = (c >> 8) & 0xF;
            let key = c & 0xFF;
            if key == 0 {
                format!("DL{}", layer)
            } else {
                keycode_to_label(key as u16, false)
            }
        }


        // Dygma SuperKeys (0xD2CC base, range 0xD2CC-0xD300)
        c if (0xD2CC..0xD300).contains(&c) => format!("S{}", c - 0xD2CC),

        // Dygma SuperKeys extended range (0xD000-0xDFFF) - catch remaining
        c if (0xD000..0xE000).contains(&c) => "SK".into(),

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

/// Get unshifted label for number/punctuation keys (0x1E-0x38)
fn unshifted_label(code: u16) -> String {
    match code {
        0x1E => "1".into(), 0x1F => "2".into(), 0x20 => "3".into(), 0x21 => "4".into(),
        0x22 => "5".into(), 0x23 => "6".into(), 0x24 => "7".into(), 0x25 => "8".into(),
        0x26 => "9".into(), 0x27 => "0".into(),
        0x28 => "ENT".into(), 0x29 => "ESC".into(), 0x2A => "BSP".into(),
        0x2B => "TAB".into(), 0x2C => "SPC".into(),
        0x2D => "-".into(), 0x2E => "=".into(), 0x2F => "[".into(), 0x30 => "]".into(),
        0x31 => "\\".into(), 0x32 => "#".into(), 0x33 => ";".into(), 0x34 => "'".into(),
        0x35 => "`".into(), 0x36 => ",".into(), 0x37 => ".".into(), 0x38 => "/".into(),
        _ => format!("x{:02X}", code),
    }
}

/// Width of each half (8 keys wide: x=0 to x=7 for row 1)
const HALF_WIDTH: f32 = 8.0;

/// Total height (6 rows: 0,1,2,3,4,5)
const TOTAL_HEIGHT: f32 = 6.0;

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

    /// Query active layers - returns all active layer indices from highest to lowest
    /// layer.state returns 32 space-separated values (0=inactive, 1=active)
    fn active_layers(&mut self) -> Option<Vec<u8>> {
        let response = self.command("layer.state")?;

        // Parse "0 0 1 0 0 ..." format - collect all active layers
        let states: Vec<bool> = response
            .split_whitespace()
            .filter_map(|s| s.parse::<u8>().ok())
            .map(|v| v != 0)
            .collect();

        // Return all active layers from highest to lowest (for proper layer stacking)
        let mut active: Vec<u8> = states.iter()
            .enumerate()
            .filter(|(_, &active)| active)
            .map(|(i, _)| i as u8)
            .collect();
        active.reverse();  // Highest first for priority lookup
        Some(active)
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
    let active_layers = Arc::new(AtomicU32::new(1)); // Bitmask: bit N = layer N active, layer 0 always on
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

    // Shared pending modifiers across ALL evdev threads
    // This is critical because Dygma Raise appears as multiple devices (one per half)
    // and synthetic modifiers from one half need to be cancelled by keys from the other
    // Tuple: (label, timestamp, is_shift)
    let pending_mods: Arc<Mutex<Vec<(String, std::time::Instant, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    const MOD_DELAY_MS: u128 = 50; // If another key follows within 50ms, modifier is synthetic

    fn is_modifier(key: evdev::Key) -> bool {
        matches!(key,
            evdev::Key::KEY_LEFTSHIFT | evdev::Key::KEY_RIGHTSHIFT |
            evdev::Key::KEY_LEFTCTRL | evdev::Key::KEY_RIGHTCTRL |
            evdev::Key::KEY_LEFTALT | evdev::Key::KEY_RIGHTALT |
            evdev::Key::KEY_LEFTMETA | evdev::Key::KEY_RIGHTMETA)
    }

    fn is_shift(key: evdev::Key) -> bool {
        matches!(key, evdev::Key::KEY_LEFTSHIFT | evdev::Key::KEY_RIGHTSHIFT)
    }

    /// Get shifted version of evdev key label (for synthetic Shift+key combos)
    fn evdev_shifted_label(key: evdev::Key) -> Option<&'static str> {
        use evdev::Key;
        Some(match key {
            Key::KEY_1 => "!", Key::KEY_2 => "@", Key::KEY_3 => "#", Key::KEY_4 => "$",
            Key::KEY_5 => "%", Key::KEY_6 => "^", Key::KEY_7 => "&", Key::KEY_8 => "*",
            Key::KEY_9 => "(", Key::KEY_0 => ")",
            Key::KEY_MINUS => "_", Key::KEY_EQUAL => "+",
            Key::KEY_LEFTBRACE => "{", Key::KEY_RIGHTBRACE => "}",
            Key::KEY_BACKSLASH => "|",
            Key::KEY_SEMICOLON => ":", Key::KEY_APOSTROPHE => "\"",
            Key::KEY_GRAVE => "~",
            Key::KEY_COMMA => "<", Key::KEY_DOT => ">", Key::KEY_SLASH => "?",
            // Letters become uppercase
            Key::KEY_A => "A", Key::KEY_B => "B", Key::KEY_C => "C", Key::KEY_D => "D",
            Key::KEY_E => "E", Key::KEY_F => "F", Key::KEY_G => "G", Key::KEY_H => "H",
            Key::KEY_I => "I", Key::KEY_J => "J", Key::KEY_K => "K", Key::KEY_L => "L",
            Key::KEY_M => "M", Key::KEY_N => "N", Key::KEY_O => "O", Key::KEY_P => "P",
            Key::KEY_Q => "Q", Key::KEY_R => "R", Key::KEY_S => "S", Key::KEY_T => "T",
            Key::KEY_U => "U", Key::KEY_V => "V", Key::KEY_W => "W", Key::KEY_X => "X",
            Key::KEY_Y => "Y", Key::KEY_Z => "Z",
            _ => return None,
        })
    }

    for mut device in keyboards {
        let heat_clone = Arc::clone(&key_heat);
        let running_clone = Arc::clone(&running);
        let last_key_clone = Arc::clone(&last_key);
        let shift_clone = Arc::clone(&shift_held);
        let pending_mods_clone = Arc::clone(&pending_mods);
        let debug_mode = config.debug;

        let handle = std::thread::spawn(move || {
            use std::os::unix::io::AsRawFd;
            use std::time::Instant;

            let fd = device.as_raw_fd();
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }

            while running_clone.load(Ordering::Relaxed) {
                let now = Instant::now();

                // Promote pending modifiers that have waited long enough (real modifier press)
                if let Ok(mut pending) = pending_mods_clone.lock() {
                    let mut promoted = Vec::new();
                    pending.retain(|(label, time, _is_s)| {
                        if now.duration_since(*time).as_millis() > MOD_DELAY_MS {
                            promoted.push(label.clone());
                            false // Remove from pending
                        } else {
                            true // Keep waiting
                        }
                    });

                    if !promoted.is_empty() {
                        if let Ok(mut heat) = heat_clone.lock() {
                            for label in promoted {
                                heat.insert(label, 1.0);
                            }
                        }
                        // Note: shift_held is now tracked directly from key events, not promotion
                    }
                }

                if let Ok(events) = device.fetch_events() {
                    for ev in events {
                        if let evdev::InputEventKind::Key(key) = ev.kind() {
                            // Track shift state directly from press/release
                            if key == evdev::Key::KEY_LEFTSHIFT || key == evdev::Key::KEY_RIGHTSHIFT {
                                shift_clone.store(ev.value() != 0, Ordering::Relaxed);
                            }

                            if ev.value() == 1 || ev.value() == 2 {
                                let label = evdev_key_to_label(key)
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| evdev_key_raw(key));

                                if is_modifier(key) {
                                    // Queue modifier - wait to see if it's synthetic
                                    if let Ok(mut pending) = pending_mods_clone.lock() {
                                        pending.push((label.clone(), now, is_shift(key)));
                                    }
                                } else {
                                    // Non-modifier key pressed - check if Shift was pending (synthetic)
                                    let had_synthetic_shift = if let Ok(mut pending) = pending_mods_clone.lock() {
                                        let had_shift = pending.iter().any(|(_, _, is_s)| *is_s);
                                        pending.clear();
                                        had_shift
                                    } else {
                                        false
                                    };

                                    // Use shifted label if Shift was synthetic, otherwise use raw label
                                    let heat_label = if had_synthetic_shift {
                                        evdev_shifted_label(key)
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| label.clone())
                                    } else {
                                        label.clone()
                                    };

                                    if let Ok(mut heat) = heat_clone.lock() {
                                        heat.insert(heat_label, 1.0);
                                    }
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
    let mut prev_layer_mask: u32 = 1;  // Track previous layer to detect changes

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
                if let Some(resp) = f.command("layer.state") {
                    if config.debug {
                        debug_info = format!("layer.state: {}", resp);
                    }
                    // Parse layer state into bitmask
                    let mut mask: u32 = 0;
                    for (i, s) in resp.split_whitespace().enumerate() {
                        if i >= 32 { break; }
                        if let Ok(v) = s.parse::<u8>() {
                            if v != 0 {
                                mask |= 1 << i;
                            }
                        }
                    }
                    // Ensure layer 0 is always in the mask (base layer fallback for transparent keys)
                    mask |= 1;
                    // Clear shift state when layer changes (prevents stuck shift)
                    if mask != prev_layer_mask {
                        shift_held.store(false, Ordering::Relaxed);
                        prev_layer_mask = mask;
                    }
                    active_layers.store(mask, Ordering::Relaxed);
                } else if config.debug {
                    debug_info = "layer.state: no response".to_string();
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
        let layer_mask = active_layers.load(Ordering::Relaxed);
        let shifted = shift_held.load(Ordering::Relaxed);

        // Extract active layers from bitmask, highest first (for layer stacking)
        let mut layer_stack: Vec<u8> = (0..32u8)
            .filter(|&i| layer_mask & (1 << i) != 0)
            .collect();
        layer_stack.reverse(); // Highest layer first for priority lookup

        term.clear();

        let w = width as f32;
        let h = height as f32;

        // Calculate scaling based on available space for each half
        let half_char_width = HALF_WIDTH * 3.0;  // Each half in character units
        let available_per_half = (w * 0.45) as f32;  // ~45% of width per half
        let scale_x = available_per_half / half_char_width;
        // Keep vertical scale at 1.0 to avoid gaps between rows
        let scale = scale_x.min(1.5); // Cap horizontal scale, vertical is always 1.0

        let key_char_width = (scale * 3.0) as usize;
        let half_width_chars = (HALF_WIDTH * scale * 3.0) as usize;

        // Edge-align: left half to left edge, right half to right edge
        let layout_height = TOTAL_HEIGHT as usize;  // No vertical scaling
        let left_start_x: usize = 1;  // Small margin from left edge
        let right_start_x = (width as usize).saturating_sub(half_width_chars + 1);  // Small margin from right edge
        let start_y = (height as usize).saturating_sub(layout_height) / 2;

        // Draw layer indicator with connection status
        let top_layer = layer_stack.first().copied().unwrap_or(0);
        let connection_status = if has_focus { "Focus" } else if has_evdev { "evdev" } else { "none" };
        let layer_text = format!("[ Layer {} : {} ]", top_layer + 1, connection_status);
        let layer_x = (width as usize - layer_text.len()) / 2;
        let (layer_color, _) = scheme_color(state.color_scheme, 2, true);
        term.set_str(layer_x as i32, 0, &layer_text, Some(layer_color), true);

        // Debug status line (only in debug mode)
        if config.debug {
            let (status_color, _) = scheme_color(state.color_scheme, 0, false);
            let _last_key_info = last_key.lock().map(|k| k.clone()).unwrap_or_default();
            // Show keycodes for number row positions (1-6) on active layer
            let km_samples = if let Some(ref km) = keymap {
                // Physical positions 1-6 are number row keys
                // PHYSICAL_TO_KEYMAP[1..7] gives keymap indices for "1" through "6"
                let num_keys: Vec<String> = (1..=6)
                    .filter_map(|phys| {
                        let km_idx = PHYSICAL_TO_KEYMAP.get(phys).copied()?;
                        // Get keycode from top active layer
                        for &layer in &layer_stack {
                            if let Some(layer_keys) = km.get(layer as usize) {
                                if let Some(&keycode) = layer_keys.get(km_idx) {
                                    if keycode != 0 && keycode != 65535 {
                                        return Some(format!("{}:{:04X}", phys, keycode));
                                    }
                                }
                            }
                        }
                        None
                    })
                    .collect();
                format!("NumRow: {} shf:{}", num_keys.join(" "), shifted)
            } else {
                "km: None".to_string()
            };
            let debug_status = format!("{} | {} | {}", keymap_status, km_samples, debug_info);
            term.set_str(1, height as i32 - 1, &debug_status, Some(status_color), false);
        }

        // Draw left half (aligned to left edge)
        draw_half(
            &mut term,
            LEFT_MAIN,
            LEFT_THUMB,
            left_start_x,
            start_y + 1,
            scale,
            key_char_width,
            &heat_snapshot,
            state.color_scheme,
            keymap.as_ref(),
            &layer_stack,
            shifted,
        );

        // Draw right half (aligned to right edge)
        draw_half(
            &mut term,
            RIGHT_MAIN,
            RIGHT_THUMB,
            right_start_x,
            start_y + 1,
            scale,
            key_char_width,
            &heat_snapshot,
            state.color_scheme,
            keymap.as_ref(),
            &layer_stack,
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
    layers: &[u8],  // Active layers from highest to lowest for stacking
    shifted: bool,
) {
    for (idx, pos) in main_keys.iter().chain(thumb_keys.iter()) {
        // Use signed math to handle negative offsets (for right half stagger)
        let x = (base_x as i32 + (pos.x * scale * 3.0) as i32).max(0) as usize;
        let y = base_y + pos.y as usize;  // No vertical scaling - rows always 1 char apart
        let w = ((pos.w * scale * 3.0) as usize).max(1);  // 3 char label width

        // Get label from keymap with layer stacking (highest to lowest)
        let label: String = if let Some(km) = keymap {
            let keymap_idx = PHYSICAL_TO_KEYMAP.get(*idx).copied().unwrap_or(255);
            if keymap_idx != 255 {
                // Try each active layer from highest to lowest
                let mut found_label = String::new();
                for &layer in layers {
                    let layer_idx = (layer as usize).min(km.len().saturating_sub(1));
                    if let Some(layer_keys) = km.get(layer_idx) {
                        if let Some(&keycode) = layer_keys.get(keymap_idx) {
                            // 0 and 65535 are transparent - try next layer
                            if keycode != 0 && keycode != 65535 {
                                let label = keycode_to_label(keycode, shifted);
                                if !label.is_empty() {
                                    found_label = label;
                                    break;
                                }
                            }
                        }
                    }
                }
                if found_label.is_empty() {
                    // All layers transparent - use default
                    DEFAULT_LABELS.get(*idx).copied().unwrap_or("").to_string()
                } else {
                    found_label
                }
            } else {
                DEFAULT_LABELS.get(*idx).copied().unwrap_or("").to_string()
            }
        } else {
            // No keymap - use defaults with shift handling for letters
            let default = DEFAULT_LABELS.get(*idx).copied().unwrap_or("");
            if default.len() == 1 && default.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
                if shifted {
                    default.to_string()
                } else {
                    default.to_lowercase()
                }
            } else {
                default.to_string()
            }
        };

        // Skip empty labels
        if label.is_empty() {
            continue;
        }

        // Get heat for this key (match by uppercase label for consistency)
        let heat_key = label.to_uppercase();
        let key_heat = heat.get(&heat_key).copied().unwrap_or(0.0);

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
