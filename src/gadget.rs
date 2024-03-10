use crate::hid;
use crate::hid::SerializedDescriptor;
use std::fs::File;
use std::path::PathBuf;
use usb_gadget::function::hid::Hid;
use usb_gadget::{default_udc, remove_all, Class, Config, Gadget, Strings};

pub fn run(_dir: String) {
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

    let (_kb_major, _kb_minor) = kbhid.device().unwrap();
    // let mut kb = File::options().append(true).open(PathBuf::from(format!("/dev/char/{}:{}", kb_major, kb_minor))).expect("failed to open kb dev");
    let (mouse_major, mouse_minor) = mousehid.device().unwrap();
    let _mouse = File::options()
        .append(true)
        .open(PathBuf::from(format!(
            "/dev/char/{}:{}",
            mouse_major, mouse_minor
        )))
        .expect("failed to open mouse dev");
}
