use crate::AppEvent;
use gud_gadget::PixelDataEndpoint;
use pixels::{Pixels, SurfaceTexture};
use tracing::error;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

const WIDTH: u32 = 768;
const HEIGHT: u32 = 1024;

pub struct Display {
    gud_data: PixelDataEndpoint,
    // gud_func: Custom,
    pixels: Pixels,
    events: EventLoopProxy<AppEvent>,
}

impl Display {
    pub fn new(
        _ffs_dir: String,
        events: EventLoopProxy<AppEvent>,
        window: &Window,
    ) -> anyhow::Result<Self> {
        let pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
        };

        let (gud_data, _gud_data_ep) = PixelDataEndpoint::new();

        // let gud_func = Custom::builder()
        //     .with_interface(
        //         Interface::new(Class::interface_specific(), "gud").with_endpoint(gud_data_ep),
        //     )
        //     .existing(ffs_dir)?;

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

        Ok(Self {
            gud_data,
            // gud_func,
            pixels,
            events,
        })
    }

    pub fn render(&mut self) {
        if let Err(err) = self.pixels.render() {
            // TODO: properly handle this
            error!("pixels.render {}", err);
        }
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if let Err(err) = self.pixels.resize_surface(width, height) {
            error!("pixels.resize_surface {}", err);
        }
    }
}
