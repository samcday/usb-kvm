#![deny(clippy::all)]
#![forbid(unsafe_code)]

mod display;
mod gadget;
mod hid;
mod keyboard;
mod mouse;
use clap::Parser;
use tempfile::tempdir_in;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::window::WindowBuilder;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gadget: Option<String>,
}

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

#[derive(Debug)]
pub enum AppEvent {
    DisplayFrameArrived,
}

fn run() {
    let _ffs_dir = tempdir_in(std::env::var("XDG_RUNTIME_DIR").unwrap());

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
        .build()
        .unwrap();
    let window = {
        WindowBuilder::new()
            .with_title("Hello Pixels")
            .with_maximized(true)
            .build(&event_loop)
            .unwrap()
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut display =
        display::Display::new("TODO".to_string(), event_loop.create_proxy(), &window).unwrap();
    let mut kb = keyboard::Keyboard::new();
    let mut mouse = mouse::Mouse::new();

    event_loop
        .run(move |event, elwt| match event {
            Event::UserEvent(app_event) => match app_event {
                AppEvent::DisplayFrameArrived => window.request_redraw(),
            },
            Event::WindowEvent {
                event: window_event,
                ..
            } => match window_event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::RedrawRequested => {
                    display.render();
                }
                WindowEvent::Resized(size) => {
                    display.resize(size.width, size.height);
                }
                WindowEvent::ModifiersChanged(mods_event) => {
                    kb.handle_modifiers(mods_event);
                }
                WindowEvent::KeyboardInput {
                    event: key_event, ..
                } => {
                    kb.handle_key(key_event);
                }
                WindowEvent::Touch(touch) => {
                    mouse.handle_touch(touch);
                }
                _ => {}
            },
            _ => {}
        })
        .unwrap();
}
