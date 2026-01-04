//! On-screen keyboard visualization with global key monitoring via evdev

use crate::config::FractalConfig;
use crate::evdev_util::ReconnectingDevice;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::style::Color;
use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Keyboard layout constants (US QWERTY) - (normal_label, shifted_label, width)
const KB_ROW_F: &[(&str, &str, f32)] = &[
    ("Esc", "Esc", 1.0), ("", "", 0.5), ("F1", "F1", 1.0), ("F2", "F2", 1.0), ("F3", "F3", 1.0), ("F4", "F4", 1.0),
    ("", "", 0.25), ("F5", "F5", 1.0), ("F6", "F6", 1.0), ("F7", "F7", 1.0), ("F8", "F8", 1.0),
    ("", "", 0.25), ("F9", "F9", 1.0), ("F10", "F10", 1.0), ("F11", "F11", 1.0), ("F12", "F12", 1.0),
];
const KB_ROW_NUM: &[(&str, &str, f32)] = &[
    ("`", "~", 1.0), ("1", "!", 1.0), ("2", "@", 1.0), ("3", "#", 1.0), ("4", "$", 1.0), ("5", "%", 1.0),
    ("6", "^", 1.0), ("7", "&", 1.0), ("8", "*", 1.0), ("9", "(", 1.0), ("0", ")", 1.0),
    ("-", "_", 1.0), ("=", "+", 1.0), ("Bksp", "Bksp", 1.5),
];
const KB_ROW_TOP: &[(&str, &str, f32)] = &[
    ("Tab", "Tab", 1.5), ("q", "Q", 1.0), ("w", "W", 1.0), ("e", "E", 1.0), ("r", "R", 1.0), ("t", "T", 1.0),
    ("y", "Y", 1.0), ("u", "U", 1.0), ("i", "I", 1.0), ("o", "O", 1.0), ("p", "P", 1.0),
    ("[", "{", 1.0), ("]", "}", 1.0), ("\\", "|", 1.5),
];
const KB_ROW_HOME: &[(&str, &str, f32)] = &[
    ("Caps", "Caps", 1.75), ("a", "A", 1.0), ("s", "S", 1.0), ("d", "D", 1.0), ("f", "F", 1.0), ("g", "G", 1.0),
    ("h", "H", 1.0), ("j", "J", 1.0), ("k", "K", 1.0), ("l", "L", 1.0), (";", ":", 1.0),
    ("'", "\"", 1.0), ("Enter", "Enter", 2.25),
];
const KB_ROW_SHIFT: &[(&str, &str, f32)] = &[
    ("Shift", "Shift", 2.25), ("z", "Z", 1.0), ("x", "X", 1.0), ("c", "C", 1.0), ("v", "V", 1.0), ("b", "B", 1.0),
    ("n", "N", 1.0), ("m", "M", 1.0), (",", "<", 1.0), (".", ">", 1.0), ("/", "?", 1.0), ("Shift", "Shift", 2.75),
];
const KB_ROW_BOTTOM: &[(&str, &str, f32)] = &[
    ("Ctrl", "Ctrl", 1.5), ("Meta", "Meta", 1.0), ("Alt", "Alt", 1.25), ("Space", "Space", 6.25),
    ("Alt", "Alt", 1.25), ("Meta", "Meta", 1.0), ("Menu", "Menu", 1.0), ("Ctrl", "Ctrl", 1.5),
];

/// Map evdev key code to display label
fn evdev_key_to_label(key: evdev::Key) -> Option<&'static str> {
    use evdev::Key;
    Some(match key {
        Key::KEY_ESC => "Esc",
        Key::KEY_1 => "1", Key::KEY_2 => "2", Key::KEY_3 => "3", Key::KEY_4 => "4", Key::KEY_5 => "5",
        Key::KEY_6 => "6", Key::KEY_7 => "7", Key::KEY_8 => "8", Key::KEY_9 => "9", Key::KEY_0 => "0",
        Key::KEY_MINUS => "-", Key::KEY_EQUAL => "=", Key::KEY_BACKSPACE => "Bksp",
        Key::KEY_TAB => "Tab",
        Key::KEY_Q => "Q", Key::KEY_W => "W", Key::KEY_E => "E", Key::KEY_R => "R", Key::KEY_T => "T",
        Key::KEY_Y => "Y", Key::KEY_U => "U", Key::KEY_I => "I", Key::KEY_O => "O", Key::KEY_P => "P",
        Key::KEY_LEFTBRACE => "[", Key::KEY_RIGHTBRACE => "]", Key::KEY_BACKSLASH => "\\",
        Key::KEY_CAPSLOCK => "Caps",
        Key::KEY_A => "A", Key::KEY_S => "S", Key::KEY_D => "D", Key::KEY_F => "F", Key::KEY_G => "G",
        Key::KEY_H => "H", Key::KEY_J => "J", Key::KEY_K => "K", Key::KEY_L => "L",
        Key::KEY_SEMICOLON => ";", Key::KEY_APOSTROPHE => "'", Key::KEY_ENTER => "Enter",
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => "Shift",
        Key::KEY_Z => "Z", Key::KEY_X => "X", Key::KEY_C => "C", Key::KEY_V => "V", Key::KEY_B => "B",
        Key::KEY_N => "N", Key::KEY_M => "M",
        Key::KEY_COMMA => ",", Key::KEY_DOT => ".", Key::KEY_SLASH => "/",
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => "Ctrl",
        Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => "Meta",
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => "Alt",
        Key::KEY_SPACE => "Space",
        Key::KEY_GRAVE => "`",
        Key::KEY_F1 => "F1", Key::KEY_F2 => "F2", Key::KEY_F3 => "F3", Key::KEY_F4 => "F4",
        Key::KEY_F5 => "F5", Key::KEY_F6 => "F6", Key::KEY_F7 => "F7", Key::KEY_F8 => "F8",
        Key::KEY_F9 => "F9", Key::KEY_F10 => "F10", Key::KEY_F11 => "F11", Key::KEY_F12 => "F12",
        Key::KEY_COMPOSE => "Menu",
        _ => return None,
    })
}

/// Run the keyboard visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Default to electric (cyan/white)

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Key press tracking with fade-out
    let key_heat: Arc<Mutex<HashMap<String, f32>>> = Arc::new(Mutex::new(HashMap::new()));
    let shift_held = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));

    // Find keyboard devices
    let keyboards = crate::evdev_util::find_keyboard_devices();
    let has_evdev = !keyboards.is_empty();

    // Spawn evdev listener threads for each keyboard
    let mut handles = Vec::new();
    for device in keyboards {
        let heat_clone = Arc::clone(&key_heat);
        let shift_clone = Arc::clone(&shift_held);
        let running_clone = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            let mut reader = ReconnectingDevice::new(device);

            while running_clone.load(Ordering::Relaxed) {
                reader.poll_events(|ev| {
                    if let evdev::InputEventKind::Key(key) = ev.kind() {
                        if matches!(key, evdev::Key::KEY_LEFTSHIFT | evdev::Key::KEY_RIGHTSHIFT) {
                            shift_clone.store(ev.value() != 0, Ordering::Relaxed);
                        }
                        if ev.value() == 1 || ev.value() == 2 {
                            if let Some(label) = evdev_key_to_label(key) {
                                if let Ok(mut heat) = heat_clone.lock() {
                                    heat.insert(label.to_string(), 1.0);
                                }
                            }
                        }
                    }
                });
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    // Build keyboard rows from const data (F-keys only in debug mode)
    let rows: Vec<&[(&str, &str, f32)]> = if config.debug {
        vec![KB_ROW_F, KB_ROW_NUM, KB_ROW_TOP, KB_ROW_HOME, KB_ROW_SHIFT, KB_ROW_BOTTOM]
    } else {
        vec![KB_ROW_NUM, KB_ROW_TOP, KB_ROW_HOME, KB_ROW_SHIFT, KB_ROW_BOTTOM]
    };

    // Key dimensions (compact mode)
    let key_width: f32 = 3.0;
    let key_height: usize = 1;

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        let w = width as usize;
        let h = height as usize;

        // Handle input (color scheme changes, speed, quit)
        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        // Decay heat values
        if let Ok(mut heat) = key_heat.lock() {
            for v in heat.values_mut() {
                *v = (*v - state.speed * 3.0).max(0.0);
            }
            heat.retain(|_, v| *v > 0.0);
        }

        term.clear();

        // Calculate keyboard vertical position (centered)
        let total_height = rows.len() * key_height + rows.len();
        let start_y = ((h - total_height) / 2).max(1);

        // Draw keyboard
        let heat_snapshot: HashMap<String, f32> = key_heat.lock().map(|h| h.clone()).unwrap_or_default();
        let is_shifted = shift_held.load(Ordering::Relaxed);

        for (row_idx, row) in rows.iter().enumerate() {
            let y = start_y + row_idx * (key_height + 1);

            // Calculate this row's total width for centering
            let row_key_count = row.iter().filter(|(l, _, _)| !l.is_empty()).count();
            let row_width_units: f32 = row.iter().map(|(_, _, w)| w).sum();
            let row_total_width = (row_width_units * key_width) as usize + row_key_count.saturating_sub(1);
            let mut x = ((w.saturating_sub(row_total_width)) / 2).max(1);

            for (normal_label, shifted_label, width_mult) in *row {
                if normal_label.is_empty() {
                    x += (key_width * width_mult) as usize;
                    continue;
                }

                let key_w = (key_width * width_mult) as usize;
                // Heat lookup - evdev labels match exactly for special keys, uppercase for letters
                let heat_key = match *normal_label {
                    "Ctrl" | "Alt" | "Meta" | "Shift" | "Caps" | "Tab" | "Enter" | "Bksp" |
                    "Esc" | "Space" | "Menu" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" |
                    "F7" | "F8" | "F9" | "F10" | "F11" | "F12" => normal_label.to_string(),
                    _ => normal_label.to_uppercase(),
                };
                let heat = heat_snapshot.get(&heat_key).copied().unwrap_or(0.0);

                // Choose label based on shift state, with display overrides
                let base_label = if is_shifted { *shifted_label } else { *normal_label };
                let display_label = match base_label {
                    "Meta" => "M",  // Display Meta key as just "M"
                    other => other,
                };

                let (color, bold) = if heat > 0.7 {
                    (Color::White, true)
                } else if heat > 0.3 {
                    scheme_color(state.color_scheme, 3, true)
                } else if heat > 0.0 {
                    scheme_color(state.color_scheme, 2, false)
                } else {
                    scheme_color(state.color_scheme, 0, false)
                };

                // Draw compact key (label with padding, no brackets)
                if y < h {
                    // Center the label within the key width
                    let truncated: String = display_label.chars().take(key_w).collect();
                    let label_start = x + (key_w.saturating_sub(truncated.len())) / 2;
                    for (i, ch) in truncated.chars().enumerate() {
                        term.set((label_start + i) as i32, y as i32, ch, Some(color), bold);
                    }
                }

                x += key_w + 1;  // Add 1 char padding between keys
            }
        }

        // Debug status bar (only in debug mode)
        if config.debug {
            let status = if has_evdev { "[GLOBAL]" } else { "[LOCAL]" };
            let status_text = format!("{} (q to quit)", status);
            let status_x = ((w as f32 - status_text.len() as f32) / 2.0).max(0.0) as usize;
            let (status_color, _) = scheme_color(state.color_scheme, 1, false);
            for (i, ch) in status_text.chars().enumerate() {
                term.set((status_x + i) as i32, 0, ch, Some(status_color), false);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    // Signal threads to stop and wait for them
    running.store(false, Ordering::Relaxed);
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
