use crate::{AppEvent, hid};
use crate::hid::SerializedDescriptor;
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, exit};
use std::sync::{Arc, Mutex};
use anyhow::Context;
use ipc_channel::ipc;
use ipc_channel::ipc::{IpcError, IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use usb_gadget::function::hid::Hid;
use usb_gadget::{default_udc, remove_all, Class, Config, Gadget, Strings, RegGadget};
use usb_gadget::function::custom::Custom;
use winit::event_loop::EventLoopProxy;

pub fn run(channel_name: String) -> anyhow::Result<()> {
    let uid: u32 = std::env::var("PKEXEC_UID")
        .context("failed to get PKEXEC_UID")?
        .parse()
        .context("failed to parse PKEXEC_UID")?;
    debug!("unprivilged uid is {:?}", uid);

    info!("connecting to channel {}", channel_name);
    let (remote_send, local_receive) = ipc::channel()?;
    let (local_send, remote_receive) = ipc::channel()?;
    IpcSender::connect(channel_name)?.send(IpcHandshake {
        sender: remote_send,
        receiver: remote_receive,
    })?;

    let udc = default_udc().context("failed to get default UDC")?;
    remove_all().context("failed to clear gadgets")?;

    let mut builder = Hid::builder();
    builder.protocol = 1;
    builder.report_len = 8;
    builder.report_desc = hid::KeyboardReport::desc().to_vec();
    let (kb_hid, kb_handle) = builder.build();

    let mut builder = Hid::builder();
    builder.protocol = 2;
    builder.report_len = 5;
    builder.report_desc = hid::MouseReport::desc().to_vec();
    let (mouse_hid, mouse_handle) = builder.build();

    let mut builder = Custom::builder();
    builder.ffs_no_init = true;
    builder.ffs_uid = Some(uid);
    let (mut gud, gud_handle) = builder.build();

    let reg = Gadget::new(
        Class::interface_specific(),
        gud_gadget::OPENMOKO_GUD_ID,
        Strings::new("usb-kvm", "usb-kvm", "123"),
    )
        .with_config(
            Config::new("usb-kvm")
                .with_function(kb_handle)
                .with_function(mouse_handle)
                .with_function(gud_handle)
        )
        .register()
        .context("failed to register gadget")?;

    local_send.send(GadgetEvent::Registered(gud.ffs_dir()?.to_str().unwrap().to_string()))?;

    let reg = Arc::new(Mutex::new(Some(reg)));

    {
        let reg = reg.clone();
        ctrlc::set_handler(move || {
            info!("received interrupt/kill signal, cleaning up");
            cleanup(reg.clone());
        }).unwrap();
    }

    // Only command we should receive at this point is to bind the UDC.
    match local_receive.recv()? {
        IpcCommand::Bind => {}
        v => panic!("unexpected IPC command {:?}", v)
    }

    reg.lock().unwrap().as_mut().unwrap().bind(Some(&udc)).context("failed to bind gadget")?;

    local_send.send(GadgetEvent::Bound)?;

    let kb_dev = {
        let (major, minor) = kb_hid.device().unwrap();
        PathBuf::from(format!(
            "/dev/char/{}:{}",
            major, minor
        ))
    };
    let mouse_dev = {
        let (major, minor) = mouse_hid.device().unwrap();
        PathBuf::from(format!(
            "/dev/char/{}:{}",
            major, minor
        ))
    };

    loop {
        match local_receive.recv()? {
            IpcCommand::MouseReport(report) => std::fs::write(&mouse_dev, report)?,
            IpcCommand::KeyboardReport(report) => std::fs::write(&kb_dev, report)?,
            v => panic!("unexpected IPC command {:?}", v)
        }
    }

    //
    //
    // //
    cleanup(reg.clone());
    Ok(())
}

fn cleanup(reg: Arc<Mutex<Option<RegGadget>>>) {
    if let Some(reg) = reg.lock().unwrap().take() {
        reg.remove().unwrap();
    }
    exit(0);
}

pub struct GadgetProcess {
    process: Child,
    sender: IpcSender<IpcCommand>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcCommand {
    Bind,
    KeyboardReport(Vec<u8>),
    MouseReport([u8; 5]),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GadgetEvent {
    Disconnected,
    Registered(String),
    Bound,
}

#[derive(Debug, Serialize, Deserialize)]
struct IpcHandshake {
    pub receiver: IpcReceiver<GadgetEvent>,
    pub sender: IpcSender<IpcCommand>,
}

/// Start the privileged gadget process, setup IPC channel.
/// Events from the gadget process will be pumped into the main winit event loop.
pub fn spawn(events: EventLoopProxy<AppEvent>) -> anyhow::Result<GadgetProcess> {
    let (ipc, channel_name) = IpcOneShotServer::<IpcHandshake>::new()?;
    let arg0 = std::env::args().next().unwrap();

    info!("spawning gadget process '{}' on ipc channel {}", arg0, channel_name);

    let process = std::process::Command::new("pkexec")
        .arg(&arg0)
        .arg("--gadget")
        .arg(channel_name)
        .spawn().context(format!("failed to start process '{}'", arg0))?;

    loop {
        process.
    }

    let (_, IpcHandshake { receiver, sender }) = ipc.accept().context("ipc handshake")?;

    std::thread::spawn(move || {
        loop {
            match receiver.recv() {
                Ok(event) => if let Err(err) = events.send_event(AppEvent::Gadget(event)) {
                    error!("failed to propagate gadget event: {}", err)
                }
                Err(err) => match err {
                    IpcError::Disconnected => {
                        events.send_event(AppEvent::Gadget(GadgetEvent::Disconnected)).unwrap();
                        return;
                    }
                    _ => error!("ipc receive failed: {}", err)
                }
            }
        }
    });

    Ok(GadgetProcess {
        process,
        sender,
    })
}

impl GadgetProcess {
    pub fn send(&self, msg: IpcCommand) -> anyhow::Result<()> {
        Ok(self.sender.send(msg).context("failed to send IPC message to gadget process")?)
    }
}