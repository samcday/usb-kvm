use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};
use crate::hid;

pub struct Keyboard {
    report: hid::KeyboardReport,
    pub report_buf: Vec<u8>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self{
            report_buf: vec![0; 8],
            report: hid::KeyboardReport{
                modifier: 0,
                reserved: 0,
                leds: 0,
                keycodes: [0, 0, 0, 0, 0, 0],
            },
        }
    }

    pub fn handle_input(&mut self, key_event: KeyEvent) {
        let mut kbchanged = false;
        if let Some(code) = if key_event.repeat { None } else { keyboard_usage(key_event.clone()) } {
            for report_code in &mut self.report.keycodes {
                if *report_code == code {
                    if key_event.state == ElementState::Released {
                        *report_code = 0;
                        kbchanged = true;
                    }
                    break;
                } else if *report_code == 0 && key_event.state == ElementState::Pressed {
                    *report_code = code;
                    kbchanged = true;
                    break;
                }
            }
        }
        if kbchanged {
            ssmarshal::serialize(self.report_buf.as_mut_slice(), &self.report).expect("report serialization");
            // kb.write_all(&kbbuf).expect("keyboard report write failed");
        }
    }
}

fn keyboard_usage(key_event: KeyEvent) -> Option<u8> {
    // Map logical keys to USB keyboard usage codes
    // https://usb.org/sites/default/files/hut1_3_0.pdf page 88
    // I originally implemented this using the scancodes since I thought that might make more sense
    // for folks with non-US keyboard layouts.
    // ... Turns out squeekboard scancodes are interpreted completely nonsensically. No idea where
    // the issue lies (winit? Wayland? Some other crate somewhere?). But I figure if someone presses
    // "w" on their OSK they didn't mean the "End" key, which is what was being reported...
    // The implication here is the keyboard layout on the host machine will need to be US.
    match key_event.logical_key {
        Key::Character(str) => {
            str.to_lowercase().chars().next().map(|char| {
                match char {
                    char @ 'a'..='z' => Some(4 + (char as u32 - ('a' as u32)) as u8),
                    char @ '1'..='9' => Some(0x1E + (char as u32 - ('1' as u32)) as u8),
                    '!' => Some(0x1E),
                    '@' => Some(0x1F),
                    '#' => Some(0x20),
                    '$' => Some(0x21),
                    '%' => Some(0x22),
                    '^' => Some(0x23),
                    '&' => Some(0x24),
                    '*' => Some(0x25),
                    '(' => Some(0x26),
                    ')' | '0'=> Some(0x27),
                    '_' | '-' => Some(0x2D),
                    '=' | '+' => Some(0x2E),
                    '[' | '{' => Some(0x2F),
                    ']' | '}' => Some(0x30),
                    '\\' | '|' => Some(0x31),
                    _ => None,
                }
            }).flatten()
        }
        Key::Named(named) => {
            match named {
                NamedKey::Enter => Some(0x28),
                NamedKey::Escape => Some(0x29),
                NamedKey::Backspace => Some(0x2A),
                NamedKey::Tab => Some(0x2B),
                NamedKey::Space => Some(0x2C),
                _ => None,
            }
        }
        _ => { None }
    }
}
