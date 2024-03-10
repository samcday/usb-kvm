use crate::hid;
use crate::hid::SerializedDescriptor;
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Child;
use tracing::info;
use usb_gadget::function::hid::Hid;
use usb_gadget::{default_udc, remove_all, Class, Config, Gadget, Strings};

pub fn run(dir: String) {
    let udc = default_udc().expect("no UDC found");
    remove_all().expect("failed to clear gadgets");

    let mut builder = Hid::builder();
    builder.protocol = 1;
    builder.report_len = 8;
    builder.report_desc = hid::KeyboardReport::desc().to_vec();
    let (kbhid, kbhandle) = builder.build();

    let mut builder = Hid::builder();
    builder.protocol = 2;
    builder.report_len = 5;
    builder.report_desc = hid::MouseReport::desc().to_vec();
    let (mousehid, mousehandle) = builder.build();

    let _reg = Gadget::new(
        Class::interface_specific(),
        gud_gadget::OPENMOKO_GUD_ID,
        Strings::new("usb-kvm", "usb-kvm", ""),
    )
    .with_config(
        Config::new("usb-kvm")
            .with_function(kbhandle)
            .with_function(mousehandle), // .with_function(gud_handle)
    )
    .bind(&udc)
    .expect("gadget binding failed");

    let (kb_major, kb_minor) = kbhid.device().unwrap();
    let (mouse_major, mouse_minor) = mousehid.device().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1000));

    std::thread::scope(|s| {
        s.spawn(|| {
            let mut mouse_dev = {
                File::options()
                    .append(true)
                    .open(PathBuf::from(format!(
                        "/dev/char/{}:{}",
                        mouse_major, mouse_minor
                    )))
                    .expect("failed to open mouse dev")
            };

            let mut path = PathBuf::from(&dir);
            path.push("mouse.pipe");
            let mut fifo = File::open(path).unwrap();

            std::io::copy(&mut fifo, &mut mouse_dev).unwrap();
        });
        s.spawn(|| {
            let mut kb_dev = {
                File::options()
                    .append(true)
                    .open(PathBuf::from(format!(
                        "/dev/char/{}:{}",
                        kb_major, kb_minor
                    )))
                    .expect("failed to open kb dev")
            };

            let mut path = PathBuf::from(&dir);
            path.push("kb.pipe");
            let mut fifo = File::open(path).unwrap();

            std::io::copy(&mut fifo, &mut kb_dev).unwrap();
        });
    });
}

pub struct GadgetProcess {
    process: Child,
    kb_fifo: PathBuf,
    mouse_fifo: PathBuf,
}

impl GadgetProcess {
    pub fn kb_fifo(&self) -> anyhow::Result<File> {
        Ok(File::options().write(true).open(&self.kb_fifo)?)
    }

    pub fn mouse_fifo(&self) -> anyhow::Result<File> {
        Ok(File::options().write(true).open(&self.mouse_fifo)?)
    }
}

pub fn spawn(dir: &Path) -> anyhow::Result<GadgetProcess> {
    info!("spawning gadget process in dir {}", dir.display());

    let kb_fifo = prepare_fifo(dir, "kb.pipe")?;
    let mouse_fifo = prepare_fifo(dir, "mouse.pipe")?;

    let process = std::process::Command::new("pkexec")
        .arg(std::env::args().next().unwrap())
        .arg("--gadget")
        .arg(dir)
        .spawn()?;

    Ok(GadgetProcess {
        process,
        kb_fifo,
        mouse_fifo,
    })
}

fn prepare_fifo<T: ToString>(dir: &Path, name: T) -> anyhow::Result<PathBuf> {
    let mut fifo_path = dir.to_path_buf();
    fifo_path.push(name.to_string());
    match std::fs::remove_file(&fifo_path) {
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        v => v,
    }?;
    nix::unistd::mkfifo(&fifo_path, nix::sys::stat::Mode::S_IRWXU)?;
    Ok(fifo_path)
}
