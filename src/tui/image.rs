use anyhow::{anyhow, Result};
use ratatui::{prelude::*, widgets::Block};
use std::{
    io::Cursor,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::JoinHandle,
};

use mini_moka::sync::Cache;
use ratatui_image::{
    picker::Picker,
    protocol::{Protocol, StatefulProtocol},
    Resize, ResizeEncodeRender,
};

use crate::util::cover_from_path;

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
        protocol: Option<StatefulProtocol>,
        resize: Resize,
        area: Rect,
        path: PathBuf,
    },
}

#[allow(unused_variables)]
impl ProtocolLruData {
    pub fn encode(self, picker: &Picker) -> Result<ProtocolLruResult> {
        match self {
            ProtocolLruData::Cahced {
                key,
                path,
                width,
                height,
                area,
            } => todo!(),
            ProtocolLruData::UnCached {
                image,
                protocol,
                resize,
                area,
                path,
            } => {
                let Some(mut protocol) = protocol.or_else(|| {
                    log::info!("encode {path:?} to terminal protocol");

                    let bytes = cover_from_path(&path)?;
                    log::info!("get {} image bytes from {path:?}", bytes.len());

                    let dyn_img = image::ImageReader::new(Cursor::new(&bytes))
                        .with_guessed_format()
                        .ok()?
                        .decode()
                        .ok()?
                        .resize(450, 450, image::imageops::FilterType::CatmullRom);

                    Some(picker.new_resize_protocol(dyn_img))
                }) else {
                    return Err(anyhow!("get resize protocol failed"));
                };

                protocol.last_encoding_result();
                protocol.resize_encode(&resize, area);

                log::debug!("resize encode done");
                Ok(ProtocolLruResult::UnCached(image, protocol))
            }
        }
    }
}

pub enum ProtocolLruResult {
    Cached(String),
    UnCached(UnCachedImage, StatefulProtocol),
}

pub struct LruProtocolFactory<F> {
    cache: ImageLruStore,
    handle: Option<JoinHandle<Result<()>>>,
    on_cached: Option<F>,
    tx: Sender<ProtocolLruData>,
    rx: Option<Receiver<ProtocolLruData>>,
}

impl<F> LruProtocolFactory<F>
where
    F: Fn(ProtocolLruResult) + Send + 'static,
{
    pub fn spawn(&mut self, picker: Picker) -> Result<()> {
        if self.handle.is_some() {
            return Err(anyhow!("thread already spawned"));
        }

        let rx = self.rx.take().unwrap();
        let on_cached = self.on_cached.take();
        let handle = std::thread::spawn(move || loop {
            match rx.recv() {
                Err(_) => break Ok(()),
                Ok(req) => {
                    let result = req.encode(&picker);
                    if let Some(ref f) = on_cached {
                        match result {
                            Ok(result) => f(result),
                            Err(e) => log::error!("{e:?}"),
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
        let cache = ImageLruStore::default();

        Ok(Self {
            tx,
            rx: Some(rx),
            handle: None,
            cache,
            on_cached: None,
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

    pub fn new_uncached_protocol(&self, path: PathBuf) -> UnCachedProtocol {
        UnCachedProtocol {
            inner: None,
            tx: self.tx.clone(),
            path,
            loading: false,
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

#[allow(dead_code)]
pub struct LruProtocol {
    tx: Sender<ProtocolLruData>,
    status: LruProtocolStatus,
    inner: Option<Arc<Protocol>>,
    cache: ImageLruStore,
}

impl LruProtocol {
    fn try_fill_from_cache(&mut self) {
        match self.status {
            LruProtocolStatus::Ready | LruProtocolStatus::Err => (),
            _ => {
                todo!()
            }
        }
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

pub struct UnCachedProtocol {
    inner: Option<StatefulProtocol>,
    tx: Sender<ProtocolLruData>,
    path: PathBuf,
    loading: bool,
}

impl UnCachedProtocol {
    pub fn update_resized_protocol(&mut self, completed: StatefulProtocol) {
        self.inner.replace(completed);
    }

    pub fn reload(&mut self) {
        self.loading = false;
    }
}

impl ResizeEncodeRender for UnCachedProtocol {
    fn needs_resize(&self, resize: &Resize, area: Rect) -> Option<Rect> {
        if let Some(ref protocol) = self.inner {
            return protocol.needs_resize(resize, area);
        }

        if self.loading {
            return None;
        }

        Some(area)
    }

    fn resize_encode(&mut self, resize: &Resize, area: Rect) {
        if !self.loading {
            self.tx
                .send(ProtocolLruData::UnCached {
                    image: UnCachedImage::Cover,
                    protocol: self.inner.take(),
                    resize: resize.clone(),
                    area,
                    path: self.path.clone(),
                })
                .ok();

            self.loading = true;
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let _ = self
            .inner
            .as_mut()
            .map(|protocol| protocol.render(area, buf));
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
