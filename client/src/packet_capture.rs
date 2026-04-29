use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

use jni::objects::GlobalRef;

const CAPACITY: usize = 512;

static INSTANCE: OnceLock<Arc<Mutex<PacketCapture>>> = OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Out,
    In,
}

pub struct CapturedPacket {
    pub id: u64,
    pub direction: Direction,
    pub elapsed: f64,
    pub type_name: String,
    pub cancelled: bool,
    pub raw: GlobalRef,
    pub fields: Option<Vec<(String, String)>>,
}

pub struct PacketCapture {
    buf: VecDeque<CapturedPacket>,
    next_id: u64,
    start: Instant,
    pub enabled: bool,
    pub paused: bool,
    pub show_out: bool,
    pub show_in: bool,
    pub search: String,
    pub selected_id: Option<u64>,
}

pub fn get() -> Arc<Mutex<PacketCapture>> {
    INSTANCE
        .get_or_init(|| Arc::new(Mutex::new(PacketCapture::new())))
        .clone()
}

pub fn push_out(type_name: String, raw: GlobalRef, cancelled: bool) {
    if let Ok(mut cap) = get().try_lock() {
        cap.push(Direction::Out, type_name, raw, cancelled);
    }
}

pub fn push_in(type_name: String, raw: GlobalRef) {
    if let Ok(mut cap) = get().try_lock() {
        cap.push(Direction::In, type_name, raw, false);
    }
}

impl PacketCapture {
    fn new() -> Self {
        PacketCapture {
            buf: VecDeque::with_capacity(CAPACITY),
            next_id: 0,
            start: Instant::now(),
            enabled: true,
            paused: false,
            show_out: true,
            show_in: true,
            search: String::new(),
            selected_id: None,
        }
    }

    pub fn push(&mut self, direction: Direction, type_name: String, raw: GlobalRef, cancelled: bool) {
        if !self.enabled || self.paused {
            return;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.buf.len() >= CAPACITY {
            self.buf.pop_front();
        }
        self.buf.push_back(CapturedPacket {
            id,
            direction,
            elapsed: self.start.elapsed().as_secs_f64(),
            type_name,
            cancelled,
            raw,
            fields: None,
        });
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.selected_id = None;
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn visible_ids(&self) -> Vec<u64> {
        let q = self.search.to_lowercase();
        self.buf
            .iter()
            .rev()
            .filter(|p| {
                let dir_ok = match p.direction {
                    Direction::Out => self.show_out,
                    Direction::In => self.show_in,
                };
                dir_ok && (q.is_empty() || p.type_name.to_lowercase().contains(&q))
            })
            .map(|p| p.id)
            .collect()
    }

    pub fn get(&self, id: u64) -> Option<&CapturedPacket> {
        self.buf.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut CapturedPacket> {
        self.buf.iter_mut().find(|p| p.id == id)
    }
}

/// Strip package prefix and replace `$` with `.` for inner classes.
pub fn short_name(full: &str) -> String {
    full.rsplit('.').next().unwrap_or(full).replace('$', ".")
}
