use anyhow::{Result, anyhow};
use ratatui::layout::Rect;
use std::{
    path::PathBuf, sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    }, thread::JoinHandle
};

use mini_moka::sync::Cache;
use ratatui_image::{ResizeEncodeRender, picker::Picker, protocol::Protocol};

// 5mb cache
const LRU_MAX_CAP: u64 = 1024 * 1024 * 5;

pub struct ImageLruStore {
    cache: Cache<String, Arc<Protocol>>,
    picker: Picker,
}

impl ImageLruStore {
    pub fn new(picker: Picker) -> Self {
        let cache = Cache::builder()
            .weigher(|_key, value: &Arc<Protocol>| value.alloc() as u32)
            .max_capacity(LRU_MAX_CAP)
            .build();

        Self { cache, picker }
    }
}

pub struct ResizeRequest {
    key: String,
    path: PathBuf,
    width: u16,
    height: u16,
    area: Rect,
}

pub struct LruProtocolFactory<F> {
    handle: Option<JoinHandle<Result<()>>>,
    on_cached: Option<F>,
    tx: Sender<ResizeRequest>,
    rx: Option<Receiver<ResizeRequest>>,
}

impl<F> Default for LruProtocolFactory<F> {
    fn default() -> Self {
        let (tx, rx) = channel();

        Self {
            tx,
            rx: Some(rx),
            handle: None,
            on_cached: None,
        }
    }
}

impl<F> LruProtocolFactory<F>
where
    F: Fn() + Send + Sync + 'static,
{
    pub fn spawn(&mut self) -> Result<()> {
        if self.handle.is_some() {
            return Err(anyhow!("thread already spawned"));
        }

        let rx = self.rx.take().unwrap();
        let on_cached = self.on_cached.take();
        let handle = std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Err(_) => break Ok(()),
                    Ok(req) => {
                        if let Some(ref f) = on_cached {
                            f();
                        }
                    }
                }
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    pub fn on_cached(&mut self, f: F) {
        self.on_cached = Some(f);
    }
}

impl<F> LruProtocolFactory<F> {
    pub fn new_protocol(&self) -> LruProtocol {
        LruProtocol {
            tx: self.tx.clone(),
            status: LruProtocolStatus::Waiting,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub enum LruProtocolStatus {
    #[default]
    Waiting,
    Ready,
    Err,
}

pub struct LruProtocol {
    tx: Sender<ResizeRequest>,
    status: LruProtocolStatus,
}

impl ResizeEncodeRender for LruProtocol {
    fn resize_encode(&mut self, resize: &ratatui_image::Resize, area: ratatui::prelude::Rect) {
        todo!()
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        todo!()
    }

    fn needs_resize(
        &self,
        resize: &ratatui_image::Resize,
        area: ratatui::prelude::Rect,
    ) -> Option<ratatui::prelude::Rect> {
        todo!()
    }
}
