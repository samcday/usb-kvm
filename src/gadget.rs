use std::fs::File;
use std::path::PathBuf;
use usb_gadget::{Class, Config, default_udc, Gadget, Id, remove_all, Strings};
use usb_gadget::function::hid::Hid;
use crate::hid;
use crate::hid::SerializedDescriptor;

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

    // let (mut gud, mut gud_data, gud_handle) = gud_gadget::Function::new();

    let _reg = Gadget::new(Class::new(0, 0, 0), Id::new(0x1d50, 0x614d), Strings::new("usb-kvm", "usb-kvm", "666"))
        .with_config(Config::new("usb-kvm")
                         .with_function(kbhandle)
                         .with_function(mousehandle)
                     // .with_function(gud_handle)
        )
        .bind(&udc)
        .expect("gadget binding failed");


    let (kb_major, kb_minor) = kbhid.device().unwrap();
    // let mut kb = File::options().append(true).open(PathBuf::from(format!("/dev/char/{}:{}", kb_major, kb_minor))).expect("failed to open kb dev");
    let (mouse_major, mouse_minor) = mousehid.device().unwrap();
    let mut mouse = File::options().append(true).open(PathBuf::from(format!("/dev/char/{}:{}", mouse_major, mouse_minor))).expect("failed to open mouse dev");

}
