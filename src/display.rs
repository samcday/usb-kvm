use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::AppEvent;
use gud_gadget::{Event, PixelDataEndpoint};
use pixels::{Pixels, SurfaceTexture};
use tracing::error;
use usb_gadget::Class;
use usb_gadget::function::custom::{Custom, Interface};
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

const WIDTH: u32 = 768;
const HEIGHT: u32 = 1024;

pub struct Display {
    pixels: Arc<Mutex<Pixels>>,
    events: EventLoopProxy<AppEvent>,
}

impl Display {
    pub fn new(
        events: EventLoopProxy<AppEvent>,
        window: &Window,
    ) -> Self {
        let pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
        };

        let pixels = Arc::new(Mutex::new(pixels));

        Self {
            pixels,
            events,
        }
    }

    pub fn setup(&mut self, ffs_dir: String) -> anyhow::Result<()> {
        let (gud_data, gud_data_ep) = PixelDataEndpoint::new();
        let gud_func = Custom::builder()
            .with_interface(
                Interface::new(Class::vendor_specific(Class::VENDOR_SPECIFIC, 0), "gud").with_endpoint(gud_data_ep),
            )
            .existing(ffs_dir)?;

        {
            let pixels = self.pixels.clone();
            let events = self.events.clone();
            std::thread::spawn(move || {
                run(gud_func, gud_data, pixels, events);
            });
        }

        Ok(())
    }

    pub fn render(&mut self) {
        if let Err(err) = self.pixels.lock().unwrap().render() {
            // TODO: properly handle this
            error!("pixels.render {}", err);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Err(err) = self.pixels.lock().unwrap().resize_surface(width, height) {
            error!("pixels.resize_surface {}", err);
        }
    }
}

fn run(mut gud_func: Custom, mut gud_data: PixelDataEndpoint, pixels: Arc<Mutex<Pixels>>, events: EventLoopProxy<AppEvent>) {
    loop {
        if let Ok(Some(event)) = gud_func.event_timeout(Duration::from_millis(100)) {
            println!("yee: {:?}", event);
            if let Ok(Some(gud_event)) = gud_gadget::event(event) {
                println!("yee: {:?}", gud_event);
                match gud_event {
                    Event::GetDescriptor(req) => {
                        req.send_descriptor(WIDTH, HEIGHT, WIDTH, HEIGHT).expect("failed to send descriptor");
                    }
                    Event::GetPixelFormats(req) => {
                        req.send_pixel_formats(&[gud_gadget::GUD_PIXEL_FORMAT_XRGB8888]).unwrap()
                    }
                    Event::GetDisplayModes(req) => {
                        req.send_modes(&[gud_gadget::DisplayMode {
                            clock: WIDTH * HEIGHT * 60 / 1000,
                            hdisplay: WIDTH as u16,
                            htotal: WIDTH as u16,
                            hsync_end: WIDTH as u16,
                            hsync_start: WIDTH as u16,
                            vtotal: HEIGHT as u16,
                            vdisplay: HEIGHT as u16,
                            vsync_end: HEIGHT as u16,
                            vsync_start: HEIGHT as u16,
                            flags: 0,
                        }]).expect("failed to send modes");
                    }
                    Event::Buffer(info) => {
                        gud_data.recv_buffer(info, pixels.lock().unwrap().frame_mut(), (WIDTH * 4) as usize, 4).expect("recv_buffer failed");
                        events.send_event(AppEvent::DisplayFrameArrived).unwrap();
                    }
                }
            }
        }
    }
}
