#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use error_iter::ErrorIter as _;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use serde::ser::{Serialize, SerializeTuple, Serializer};
use usb_gadget::{Class, Config, default_udc, Gadget, Id, remove_all, Strings};
use usb_gadget::function::hid::Hid;
use usbd_hid_macros::gen_hid_descriptor;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::raw_window_handle::HasDisplayHandle;
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

    // TODO: something more robust than this.
    // Would probably be better to `mknod` the device files in a temp dir under app control.
    sleep(Duration::from_millis(100));

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
