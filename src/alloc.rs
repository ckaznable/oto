#[macro_export]
macro_rules! arena_alloc {
    ($arena:expr, $($arg:tt)*) => {
        {
            use std::fmt::Write;
            if let Err(e) = write!(&mut $arena.buffer, $($arg)*) {
                log::error!("{e}");
                0
            } else {
                $arena.update()
            }
        }
    };
}

#[derive(Default)]
pub struct StringArena {
    pub buffer: String,
    offsets: Vec<(usize, usize)>,
    index: usize,
}

impl StringArena {
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(1024),
            offsets: Vec::with_capacity(32),
            ..Default::default()
        }
    }

    pub fn update(&mut self) -> usize {
        self.offsets.push((self.index, self.buffer.len()));
        self.index = self.buffer.len();
        self.offsets.len() - 1
    }

    pub fn get(&self, id: usize) -> &str {
        self.offsets
            .get(id)
            .map(|&(start, end)| &self.buffer[start..end])
            .unwrap_or("")
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.offsets.clear();
        self.index = 0;
    }
}
