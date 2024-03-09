#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use error_iter::ErrorIter as _;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use serde::ser::{Serialize, SerializeTuple, Serializer};
use usb_gadget::{Class, Config, default_udc, Gadget, Id, remove_all, Strings};
use usb_gadget::function::hid::Hid;
use usbd_hid_macros::gen_hid_descriptor;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::WindowBuilder;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const BOX_SIZE: i16 = 64;

/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    box_x: i16,
    box_y: i16,
    velocity_x: i16,
    velocity_y: i16,
}

pub trait SerializedDescriptor {
    fn desc() -> &'static [u8];
}
pub trait AsInputReport: Serialize {}

/// KeyboardReport describes a report and its companion descriptor that can be
/// used to send keyboard button presses to a host and receive the status of the
/// keyboard LEDs.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
            #[packed_bits 8] #[item_settings data,variable,absolute] modifier=input;
        };
        (usage_min = 0x00, usage_max = 0xFF) = {
            #[item_settings constant,variable,absolute] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits 5] #[item_settings data,variable,absolute] leds=output;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xDD) = {
            #[item_settings data,array,absolute] keycodes=input;
        };
    }
)]
#[allow(dead_code)]
pub struct KeyboardReport {
    pub modifier: u8,
    pub reserved: u8,
    pub leds: u8,
    pub keycodes: [u8; 6],
}

/// MouseReport describes a report and its companion descriptor than can be used
/// to send mouse movements and button presses to a host.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = MOUSE) = {
        (collection = PHYSICAL, usage = POINTER) = {
            (usage_page = BUTTON, usage_min = BUTTON_1, usage_max = BUTTON_8) = {
                #[packed_bits 8] #[item_settings data,variable,absolute] buttons=input;
            };
            (usage_page = GENERIC_DESKTOP,) = {
                (usage = X,) = {
                    #[item_settings data,variable,relative] x=input;
                };
                (usage = Y,) = {
                    #[item_settings data,variable,relative] y=input;
                };
                (usage = WHEEL,) = {
                    #[item_settings data,variable,relative] wheel=input;
                };
            };
            (usage_page = CONSUMER,) = {
                (usage = AC_PAN,) = {
                    #[item_settings data,variable,relative] pan=input;
                };
            };
        };
    }
)]
#[allow(dead_code)]
pub struct MouseReport {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
    pub wheel: i8, // Scroll down (negative) or up (positive) this many units
    pub pan: i8,   // Scroll left (negative) or right (positive) this many units
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello Pixels")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };
    let mut world = World::new();

    event_loop.set_control_flow(ControlFlow::Wait);

    let udc = default_udc().expect("no UDC found");
    remove_all().expect("failed to clear gadgets");

    let mut builder = Hid::builder();
    builder.protocol = 1;
    builder.report_len = 8;
    builder.report_desc = KeyboardReport::desc().to_vec();
    let (kbhid, kbhandle) = builder.build();

    let mut builder = Hid::builder();
    builder.protocol = 2;
    builder.report_len = 5;
    builder.report_desc = MouseReport::desc().to_vec();
    let (mousehid, mousehandle) = builder.build();

    let _reg = Gadget::new(Class::new(0, 0, 0), Id::new(666, 666), Strings::new("Test", "FakeKeeb", "666"))
        .with_config(Config::new("kbmouse").with_function(kbhandle).with_function(mousehandle))
        .bind(&udc)
        .expect("gadget binding failed");

    let (kb_major, kb_minor) = kbhid.device().unwrap();
    let mut kb = File::options().append(true).open(PathBuf::from(format!("/dev/char/{}:{}", kb_major, kb_minor))).expect("failed to open kb dev");
    let (mouse_major, mouse_minor) = mousehid.device().unwrap();
    let mut mouse = File::options().append(true).open(PathBuf::from(format!("/dev/char/{}:{}", mouse_major, mouse_minor))).expect("failed to open mouse dev");

    let mut kbbuf: [u8; 8] = [0; 8];
    let mut kbreport = KeyboardReport{
        modifier: 0,
        reserved: 0,
        leds: 0,
        keycodes: [0, 0, 0, 0, 0, 0],
    };

    event_loop.run(move |event, elwt| {
        match event {
            Event::AboutToWait => {
                world.update();
                window.request_redraw();
            }
            Event::WindowEvent { event: window_event, .. } => {
                match window_event {
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        // Draw the current frame
                        world.draw(pixels.frame_mut());
                        if let Err(err) = pixels.render() {
                            log_error("pixels.render", err);
                            elwt.exit();
                        }
                    }
                    WindowEvent::Resized(size) => {
                        if let Err(err) = pixels.resize_surface(size.width, size.height) {
                            log_error("pixels.resize_surface", err);
                            elwt.exit();
                        }
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        let mut kbchanged = false;
                        if let Some(code) = if key_event.repeat { None } else { keyboard_usage(key_event.clone()) } {
                            println!("reee: {:?}", key_event);
                            for report_code in &mut kbreport.keycodes {
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
                            println!("sending kb report: {:?}", kbreport);
                            ssmarshal::serialize(&mut kbbuf, &kbreport).expect("report serialization");
                            kb.write_all(&kbbuf).expect("keyboard report write failed");
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }).unwrap();
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}

impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new() -> Self {
        Self {
            box_x: 24,
            box_y: 16,
            velocity_x: 1,
            velocity_y: 1,
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {
        if self.box_x <= 0 || self.box_x + BOX_SIZE > WIDTH as i16 {
            self.velocity_x *= -1;
        }
        if self.box_y <= 0 || self.box_y + BOX_SIZE > HEIGHT as i16 {
            self.velocity_y *= -1;
        }

        self.box_x += self.velocity_x;
        self.box_y += self.velocity_y;
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            let inside_the_box = x >= self.box_x
                && x < self.box_x + BOX_SIZE
                && y >= self.box_y
                && y < self.box_y + BOX_SIZE;

            let rgba = if inside_the_box {
                [0x5e, 0x48, 0xe8, 0xff]
            } else {
                [0x48, 0xb2, 0xe8, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
    }
}

// https://usb.org/sites/default/files/hut1_3_0.pdf page 88
fn keyboard_usage(key_event: KeyEvent) -> Option<u8> {
    if let PhysicalKey::Code(key_code) = key_event.physical_key {
        match key_code {
            KeyCode::KeyA => Some(0x04),
            KeyCode::KeyB => Some(0x05),
            KeyCode::KeyC => Some(0x06),
            KeyCode::KeyD => Some(0x07),
            KeyCode::KeyE => Some(0x08),
            KeyCode::KeyF => Some(0x09),
            KeyCode::KeyG => Some(0x0A),
            KeyCode::KeyH => Some(0x0B),
            KeyCode::KeyI => Some(0x0C),
            KeyCode::KeyJ => Some(0x0D),
            KeyCode::KeyK => Some(0x0E),
            KeyCode::KeyL => Some(0x0F),
            KeyCode::KeyM => Some(0x10),
            KeyCode::KeyN => Some(0x11),
            KeyCode::KeyO => Some(0x12),
            KeyCode::KeyP => Some(0x13),
            KeyCode::KeyQ => Some(0x14),
            KeyCode::KeyR => Some(0x15),
            KeyCode::KeyS => Some(0x16),
            KeyCode::KeyT => Some(0x17),
            KeyCode::KeyU => Some(0x18),
            KeyCode::KeyV => Some(0x19),
            KeyCode::KeyW => Some(0x1A),
            KeyCode::KeyX => Some(0x1B),
            KeyCode::KeyY => Some(0x1C),
            KeyCode::KeyZ => Some(0x1D),
            KeyCode::Digit1 => Some(0x1E),
            KeyCode::Digit2 => Some(0x1F),
            KeyCode::Digit3 => Some(0x20),
            KeyCode::Digit4 => Some(0x21),
            KeyCode::Digit5 => Some(0x22),
            KeyCode::Digit6 => Some(0x23),
            KeyCode::Digit7 => Some(0x24),
            KeyCode::Digit8 => Some(0x25),
            KeyCode::Digit9 => Some(0x26),
            KeyCode::Digit0 => Some(0x27),
            KeyCode::Enter => Some(0x28),
            KeyCode::Escape => Some(0x29),
            KeyCode::Backspace => Some(0x2A),
            KeyCode::Tab => Some(0x2B),
            KeyCode::Space => Some(0x2C),
            KeyCode::Minus => Some(0x2D),
            KeyCode::Equal => Some(0x2E),
            KeyCode::BracketLeft => Some(0x2F),
            KeyCode::BracketRight => Some(0x30),
            KeyCode::Backslash => Some(0x31),
            // KeyCode:: => Some(0x32),
            KeyCode::Semicolon => Some(0x33),
            KeyCode::Quote => Some(0x34),
            KeyCode::Backquote => Some(0x35),
            KeyCode::Comma => Some(0x36),
            KeyCode::Period => Some(0x37),
            KeyCode::Slash => Some(0x38),
            KeyCode::CapsLock => Some(0x39),
            KeyCode::F1 => Some(0x3A),
            KeyCode::F2 => Some(0x3B),
            KeyCode::F3 => Some(0x3C),
            KeyCode::F4 => Some(0x3D),
            KeyCode::F5 => Some(0x3E),
            KeyCode::F6 => Some(0x3F),
            KeyCode::F7 => Some(0x40),
            KeyCode::F8 => Some(0x41),
            KeyCode::F9 => Some(0x42),
            KeyCode::F10 => Some(0x43),
            KeyCode::F11 => Some(0x44),
            KeyCode::F12 => Some(0x45),
            KeyCode::PrintScreen => Some(0x46),
            KeyCode::ScrollLock => Some(0x47),
            KeyCode::Pause => Some(0x48),
            KeyCode::Insert => Some(0x49),
            KeyCode::Home => Some(0x4A),
            KeyCode::PageUp => Some(0x4B),
            KeyCode::Delete => Some(0x4C),
            KeyCode::End => Some(0x4D),
            KeyCode::PageDown => Some(0x4E),
            KeyCode::ArrowRight => Some(0x4F),
            KeyCode::ArrowLeft => Some(0x50),
            KeyCode::ArrowDown => Some(0x51),
            KeyCode::ArrowUp => Some(0x52),
            KeyCode::NumLock => Some(0x53),
            KeyCode::NumpadDivide => Some(0x54),
            KeyCode::NumpadMultiply => Some(0x55),
            KeyCode::NumpadSubtract => Some(0x56),
            KeyCode::NumpadAdd => Some(0x57),
            KeyCode::NumpadEnter => Some(0x58),
            KeyCode::Numpad1 => Some(0x59),
            KeyCode::Numpad2 => Some(0x5A),
            KeyCode::Numpad3 => Some(0x5B),
            KeyCode::Numpad4 => Some(0x5C),
            KeyCode::Numpad5 => Some(0x5D),
            KeyCode::Numpad6 => Some(0x5E),
            KeyCode::Numpad7 => Some(0x5F),
            KeyCode::Numpad8 => Some(0x60),
            KeyCode::Numpad9 => Some(0x61),
            KeyCode::Numpad0 => Some(0x62),
            KeyCode::NumpadDecimal => Some(0x63),
            // KeyCode::Numpad => Some(0x64),
            // KeyCode::Numpad => Some(0x65),
            KeyCode::Power => Some(0x66),
            KeyCode::NumpadEqual => Some(0x67),
            KeyCode::F13 => Some(0x68),
            KeyCode::F14 => Some(0x69),
            KeyCode::F15 => Some(0x6A),
            KeyCode::F16 => Some(0x6B),
            KeyCode::F17 => Some(0x6C),
            KeyCode::F18 => Some(0x6D),
            KeyCode::F19 => Some(0x6E),
            KeyCode::F20 => Some(0x6F),
            KeyCode::F21 => Some(0x70),
            KeyCode::F22 => Some(0x71),
            KeyCode::F23 => Some(0x72),
            KeyCode::F24 => Some(0x73),
            // KeyCode:: => Some(0x74),
            KeyCode::Help => Some(0x75),
            KeyCode::ContextMenu => Some(0x76),
            KeyCode::Select => Some(0x77),
            KeyCode::MediaStop => Some(0x78),
            KeyCode::Again => Some(0x79),
            KeyCode::Undo => Some(0x7A),
            KeyCode::Cut => Some(0x7B),
            KeyCode::Copy => Some(0x7C),
            KeyCode::Paste => Some(0x7D),
            KeyCode::Find => Some(0x7E),
            KeyCode::AudioVolumeMute => Some(0x7F),
            KeyCode::AudioVolumeUp => Some(0x80),
            KeyCode::AudioVolumeDown => Some(0x81),
            // KeyCode:: => Some(0x82),
            // KeyCode:: => Some(0x83),
            // KeyCode:: => Some(0x84),
            KeyCode::NumpadComma => Some(0x85),
            // KeyCode:: => Some(0x86),
            // KeyCode:: => Some(0x87),
            // KeyCode:: => Some(0x88),
            // KeyCode:: => Some(0x89),
            // KeyCode:: => Some(0x8A),
            // KeyCode:: => Some(0x8B),
            // KeyCode:: => Some(0x8C),
            // KeyCode:: => Some(0x8D),
            // KeyCode:: => Some(0x8E),
            // KeyCode:: => Some(0x8F),
            // KeyCode:: => Some(0x9),
            _ => None,
        }
    } else {
        None
    }
}

/*
fn keyboard_usage(key_event: KeyEvent) -> Option<u8> {
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
*/
