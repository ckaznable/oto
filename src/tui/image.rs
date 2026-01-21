use anyhow::{Result, anyhow};
use ratatui::{prelude::*, widgets::Block};
use std::{
    ops::{Deref, DerefMut}, path::PathBuf, sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    }, thread::JoinHandle
};

use mini_moka::sync::Cache;
use ratatui_image::{
    picker::Picker,
    protocol::Protocol, thread::{ResizeRequest, ResizeResponse},
};

// 5mb cache
const LRU_MAX_CAP: u64 = 1024 * 1024 * 5;

pub type ImageCahce = Cache<String, Arc<Protocol>>;

#[derive(Clone)]
pub struct ImageLruStore {
    cache: ImageCahce,
}

impl Default for ImageLruStore {
    fn default() -> Self {
        let cache = Cache::builder()
            .weigher(|_key, value: &Arc<Protocol>| value.alloc() as u32)
            .max_capacity(LRU_MAX_CAP)
            .build();

        Self { cache }
    }
}

impl Deref for ImageLruStore {
    type Target = ImageCahce;

    fn deref(&self) -> &Self::Target {
        &self.cache
    }
}

impl DerefMut for ImageLruStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cache
    }
}

#[derive(Clone, Copy, Hash)]
pub enum UnCachedImage {
    Cover,
}

pub enum ProtocolLruData {
    Cahced {
        key: String,
        path: PathBuf,
        width: u16,
        height: u16,
        area: Rect,
    },
    UnCached {
        image: UnCachedImage,
        req: ResizeRequest,
        path: PathBuf,
    },
}

impl ProtocolLruData {
    pub fn transform(self, picker: &Picker) -> ProtocolLruResult {
        todo!()
    }
}

pub enum ProtocolLruResult {
    Cached(String),
    UnCached(UnCachedImage, ResizeResponse),
}

pub struct LruProtocolFactory<F> {
    cache: ImageLruStore,
    handle: Option<JoinHandle<Result<()>>>,
    on_cached: Option<F>,
    tx: Sender<ProtocolLruData>,
    rx: Option<Receiver<ProtocolLruData>>,
    picker: Option<Picker>,
}

impl<F> LruProtocolFactory<F>
where
    F: Fn(ProtocolLruResult) + Send + 'static,
{
    pub fn spawn(&mut self) -> Result<()> {
        if self.handle.is_some() {
            return Err(anyhow!("thread already spawned"));
        }

        let picker = self.picker.take().unwrap();
        let rx = self.rx.take().unwrap();
        let on_cached = self.on_cached.take();
        let handle = std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Err(_) => break Ok(()),
                    Ok(req) => {
                        let result = req.transform(&picker);
                        if let Some(ref f) = on_cached {
                            f(result);
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
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();
        let picker = Picker::from_query_stdio().ok();
        let cache = ImageLruStore::default();

        Ok(Self {
            tx,
            rx: Some(rx),
            handle: None,
            cache,
            on_cached: None,
            picker,
        })
    }

    pub fn new_protocol(&self) -> LruProtocol {
        LruProtocol {
            tx: self.tx.clone(),
            status: LruProtocolStatus::Init,
            inner: None,
            cache: self.cache.clone(),
        }
    }
}

#[derive(Copy, Clone, Default)]
pub enum LruProtocolStatus {
    #[default]
    Init,
    Waiting,
    Ready,
    Err,
}

pub struct LruProtocol {
    tx: Sender<ProtocolLruData>,
    status: LruProtocolStatus,
    inner: Option<Arc<Protocol>>,
    cache: ImageLruStore,
}

impl LruProtocol {
    fn try_fill_from_cache(&mut self) {
        if matches!(self.status, LruProtocolStatus::Ready|LruProtocolStatus::Err) {
            return;
        }

        todo!()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) -> bool {
        if self.inner.is_none() {
            self.try_fill_from_cache();
        }

        if let Some(ref p) = self.inner {
            ratatui_image::Image::new(p).render(area, buf);
            return true;
        }

        false
    }
}

pub struct CachedImage {
    pub bg: Color,
}

impl CachedImage {
    pub fn new(bg: Color) -> Self {
        Self { bg }
    }
}

impl StatefulWidget for CachedImage {
    type State = LruProtocol;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        if !state.render(area, buf) {
            Block::new()
                .style(Style::default().bg(self.bg))
                .render(area, buf);
        }
    }
}
