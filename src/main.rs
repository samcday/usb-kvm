#![deny(clippy::all)]
#![forbid(unsafe_code)]

mod display;
mod gadget;
mod hid;
mod keyboard;
mod mouse;

use std::path::Path;
use anyhow::Context;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::window::WindowBuilder;
use crate::gadget::{GadgetEvent, IpcCommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gadget: Option<String>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    if let Some(path) = args.gadget {
        gadget::run(path)
    } else {
        run()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AppEvent {
    DisplayFrameArrived,
    Gadget(GadgetEvent),
}

fn run() -> anyhow::Result<()> {
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

    let gadget = gadget::spawn(event_loop.create_proxy()).context("failed to spawn gadget")?;

    let mut display = display::Display::new(event_loop.create_proxy(), &window);
    let mut kb = keyboard::Keyboard::new();
    let mut mouse = mouse::Mouse::new();

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::NewEvents(StartCause::Init) => {
                    window.request_redraw();
                }
                Event::UserEvent(app_event) => match app_event {
                    AppEvent::Gadget(gadget_event) => match gadget_event {
                        GadgetEvent::Registered(path) => {
                            println!("gadget registered at {} yey", path);
                            display.setup(path).unwrap();
                            gadget.send(IpcCommand::Bind).unwrap();
                        }
                        GadgetEvent::Disconnected => {
                            // TODO: maybe automatically restart? or inform user?
                            panic!("gadget process died onoes");
                        }
                        GadgetEvent::Bound => {
                            println!("gadget bound");
                        }
                    }
                    AppEvent::DisplayFrameArrived => {
                        window.request_redraw()
                    },
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
                        kb.handle_key(key_event, &gadget);
                    }
                    WindowEvent::Touch(touch) => {
                        mouse.handle_touch(touch, &gadget);
                    }
                    _ => {}
                },
                _ => {}
            }
        })
        .unwrap();

    Ok(())
}

pub fn wait_for_path<T: AsRef<Path>>(path: T) {
    while std::fs::metadata(&path).is_err() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
