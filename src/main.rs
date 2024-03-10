#![deny(clippy::all)]
#![forbid(unsafe_code)]

mod gadget;
mod keyboard;
mod mouse;
mod display;
mod hid;

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use pixels::{Error, Pixels, SurfaceTexture};
use serde::ser::{Serialize, SerializeTuple, Serializer};
use usb_gadget::{Class, Config, default_udc, Gadget, Id, remove_all, Strings};
use usb_gadget::function::hid::Hid;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyEvent, StartCause, TouchPhase, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowBuilder;
use clap::Parser;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gadget: Option<String>,
}

const WIDTH: u32 = 768;
const HEIGHT: u32 = 1024;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    if let Some(path) = args.gadget {
        gadget::run(path);
    } else {
        run();
    }
}

fn run() {
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

    event_loop.set_control_flow(ControlFlow::Wait);

    let running = Arc::new(AtomicBool::new(true));
    let pixels = Arc::new(Mutex::new(pixels));

    let running2 = running.clone();
    let pixels2 = pixels.clone();

    // std::thread::spawn(move || {
    //     while running.load(Ordering::Relaxed) {
    //         if let Ok(Some(event)) = gud.event(Duration::from_millis(100)) {
    //             match event {
    //                 gud_gadget::Event::GetDescriptorRequest(req) => {
    //                     req.send_descriptor(WIDTH, HEIGHT, WIDTH, HEIGHT).expect("failed to send descriptor");
    //                 },
    //                 gud_gadget::Event::GetDisplayModesRequest(req) => {
    //                     req.send_modes(&[gud_gadget::DisplayMode {
    //                         clock: WIDTH * HEIGHT * 60 / 1000,
    //                         hdisplay: WIDTH as u16,
    //                         htotal: WIDTH as u16,
    //                         hsync_end: WIDTH as u16,
    //                         hsync_start: WIDTH as u16,
    //                         vtotal: HEIGHT as u16,
    //                         vdisplay: HEIGHT as u16,
    //                         vsync_end: HEIGHT as u16,
    //                         vsync_start: HEIGHT as u16,
    //                         flags: 0,
    //                     }]).expect("failed to send modes");
    //                 },
    //                 gud_gadget::Event::Buffer(info) => {
    //                     gud_data.recv_buffer(info, pixels.lock().unwrap().frame_mut(), (WIDTH * 4) as usize).expect("recv_buffer failed");
    //                 }
    //             }
    //         }
    //     }
    // });

    let mut kb = keyboard::Keyboard::new();
    let mut mouse = mouse::Mouse::new();

    event_loop.run(move |event, elwt| {
        match event {
            Event::AboutToWait => {
                // TODO: nuke once user events are setup and GUD thread can notify on new frame
                window.request_redraw();
            }
            Event::WindowEvent { event: window_event, .. } => {
                match window_event {
                    WindowEvent::CloseRequested => {
                        running2.store(false, Ordering::SeqCst);
                        elwt.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        if let Err(err) = pixels2.lock().unwrap().render() {
                            error!("pixels.render {}", err);
                            elwt.exit();
                        }
                    }
                    WindowEvent::Resized(size) => {
                        if let Err(err) = pixels2.lock().unwrap().resize_surface(size.width, size.height) {
                            error!("pixels.resize_surface {}", err);
                            elwt.exit();
                        }
                    }
                    WindowEvent::ModifiersChanged(mods) => {
                        // println!("modz: {:?}", mods);
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        kb.handle_input(key_event);
                    }
                    WindowEvent::Touch(touch) => {
                        mouse.handle_touch(touch);
                    }
                    _ => {}
                }

            }
            _ => {}
        }
    }).unwrap();
}
