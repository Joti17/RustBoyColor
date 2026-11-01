use std::time::{Duration, Instant};

pub struct FpsTracker {
    last_update: Instant,
    frames: u32,
    fps: u32,
}

impl FpsTracker {
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            frames: 0,
            fps: 0,
        }
    }

    pub fn update(&mut self) -> u32 {
        self.frames += 1;

        let elapsed = self.last_update.elapsed();

        if elapsed >= Duration::from_secs(1) {
            self.fps = (self.frames as f64 / elapsed.as_secs_f64()) as u32;
            self.frames = 0;
            self.last_update = Instant::now();
        }

        self.fps
    }

    pub fn get_fps(&self) -> u32 {
        self.fps
    }
}