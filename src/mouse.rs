use crate::hid;
use std::fs::File;
use std::io::Write;
use winit::dpi::PhysicalPosition;
use winit::event::{Touch, TouchPhase};

pub struct Mouse {
    report: hid::MouseReport,
    pub report_buf: Vec<u8>,
    active_touch: Option<(u64, PhysicalPosition<f64>)>,
    fifo: File,
}

impl Mouse {
    pub fn new(fifo: File) -> Self {
        Self {
            report: hid::MouseReport {
                x: 0,
                y: 0,
                buttons: 0,
                pan: 0,
                wheel: 0,
            },
            report_buf: vec![0; 5],
            active_touch: None,
            fifo,
        }
    }

    pub fn handle_touch(&mut self, touch: Touch) {
        match touch.phase {
            TouchPhase::Started => {
                if self.active_touch.is_none() {
                    self.active_touch = Some((touch.id, touch.location));
                }
            }
            TouchPhase::Cancelled | TouchPhase::Ended => {
                if let Some((id, _)) = self.active_touch {
                    if id == touch.id {
                        self.active_touch = None;
                    }
                }
            }
            TouchPhase::Moved => {
                if let Some((id, old_pos)) = self.active_touch {
                    if id == touch.id {
                        self.report.x = (touch.location.x - old_pos.x) as i8;
                        self.report.y = (touch.location.y - old_pos.y) as i8;
                        ssmarshal::serialize(self.report_buf.as_mut_slice(), &self.report)
                            .expect("report serialization");
                        self.fifo
                            .write_all(&self.report_buf)
                            .expect("mouse report write failed");
                        self.active_touch = Some((id, touch.location));
                    }
                }
            }
        }
    }
}
