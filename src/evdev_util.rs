//! Evdev utilities with automatic device reconnection.

use evdev::Device;
use std::os::unix::io::AsRawFd;

/// Sets a device to non-blocking mode.
pub fn set_nonblocking(device: &Device) {
    let fd = device.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
}

/// Finds all keyboard devices.
pub fn find_keyboard_devices() -> Vec<Device> {
    let mut keyboards = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("event") {
                    if let Ok(device) = Device::open(&path) {
                        // Check if device has key events (is a keyboard)
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

/// Wrapper for evdev device with automatic reconnection support.
pub struct ReconnectingDevice {
    device: Device,
    physical_path: Option<String>,
    consecutive_errors: u32,
    needs_reconnect: bool,
}

impl ReconnectingDevice {
    /// Create a new reconnecting device wrapper.
    pub fn new(device: Device) -> Self {
        let physical_path = device.physical_path().map(|p| p.to_string());
        set_nonblocking(&device);
        Self {
            device,
            physical_path,
            consecutive_errors: 0,
            needs_reconnect: false,
        }
    }

    /// Try to fetch events, handling reconnection automatically.
    /// Returns None if no events available or device is reconnecting.
    /// The callback is invoked for each event.
    pub fn poll_events<F>(&mut self, mut callback: F)
    where
        F: FnMut(&evdev::InputEvent),
    {
        // Handle reconnection at start (before borrowing device)
        if self.needs_reconnect {
            self.needs_reconnect = false;
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Try to find device by physical path first
            if let Some(ref path) = self.physical_path {
                for new_dev in find_keyboard_devices() {
                    if new_dev.physical_path().map(|p| p.to_string()).as_ref() == Some(path) {
                        self.device = new_dev;
                        set_nonblocking(&self.device);
                        self.consecutive_errors = 0;
                        break;
                    }
                }
            } else if let Some(new_dev) = find_keyboard_devices().into_iter().next() {
                // Fallback to first keyboard
                self.device = new_dev;
                set_nonblocking(&self.device);
                self.consecutive_errors = 0;
            }
        }

        match self.device.fetch_events() {
            Ok(events) => {
                self.consecutive_errors = 0;
                for ev in events {
                    callback(&ev);
                }
            }
            Err(e) => {
                // EAGAIN/EWOULDBLOCK are normal for non-blocking reads
                if e.raw_os_error() != Some(libc::EAGAIN)
                    && e.raw_os_error() != Some(libc::EWOULDBLOCK)
                {
                    self.consecutive_errors += 1;
                    if self.consecutive_errors > 50 {
                        self.needs_reconnect = true;
                    }
                }
            }
        }
    }
}
