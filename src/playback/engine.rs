use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Clone, serde::Serialize)]
pub struct PlaybackTickEvent {
    pub frame: u64,
    pub is_playing: bool,
}

pub struct PlaybackEngine {
    is_playing: Arc<AtomicBool>,
    current_frame: Arc<AtomicU64>,
    seek_requested: Arc<AtomicBool>,
    fps: Arc<Mutex<f64>>,
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    loop_range: Arc<Mutex<Option<(u64, u64)>>>,
    max_frame: Arc<AtomicU64>,
    on_frame_update: Arc<Mutex<Option<Arc<dyn Fn(PlaybackTickEvent) + Send + Sync>>>>,
}

impl PlaybackEngine {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_frame: Arc::new(AtomicU64::new(0)),
            seek_requested: Arc::new(AtomicBool::new(false)),
            fps: Arc::new(Mutex::new(30.0)),
            task_handle: Arc::new(Mutex::new(None)),
            loop_range: Arc::new(Mutex::new(None)),
            max_frame: Arc::new(AtomicU64::new(0)),
            on_frame_update: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_on_frame_update<F>(&self, callback: F)
    where
        F: Fn(PlaybackTickEvent) + Send + Sync + 'static,
    {
        let mut guard = self.on_frame_update.lock().await;
        *guard = Some(Arc::new(callback));
    }

    async fn emit_tick(&self, frame: u64, is_playing: bool) {
        let guard = self.on_frame_update.lock().await;
        if let Some(cb) = &*guard {
            cb(PlaybackTickEvent { frame, is_playing });
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn get_current_frame(&self) -> u64 {
        self.current_frame.load(Ordering::SeqCst)
    }

    pub fn set_current_frame(&self, frame: u64) {
        self.current_frame.store(frame, Ordering::SeqCst);
    }

    pub async fn set_fps(&self, new_fps: f64) {
        let mut guard = self.fps.lock().await;
        *guard = if new_fps > 0.0 { new_fps } else { 30.0 };
    }

    pub fn set_max_frame(&self, max: u64) {
        self.max_frame.store(max, Ordering::SeqCst);
    }

    pub fn get_max_frame(&self) -> u64 {
        self.max_frame.load(Ordering::SeqCst)
    }

    pub async fn get_loop_range(&self) -> Option<(u64, u64)> {
        *self.loop_range.lock().await
    }

    pub async fn set_loop_range(&self, in_frame: Option<u64>, out_frame: Option<u64>) {
        let mut seek_to: Option<u64> = None;
        {
            let mut guard = self.loop_range.lock().await;
            match (in_frame, out_frame) {
                (Some(start), Some(end)) if end > start => {
                    *guard = Some((start, end));
                    let curr = self.current_frame.load(Ordering::SeqCst);
                    if curr < start || curr > end {
                        seek_to = Some(start);
                    }
                }
                _ => {
                    *guard = None;
                }
            }
        }
        if let Some(target) = seek_to {
            self.seek(target).await;
        }
    }

    pub async fn play(&self) {
        if self.is_playing.load(Ordering::SeqCst) {
            return;
        }

        self.is_playing.store(true, Ordering::SeqCst);
        self.seek_requested.store(false, Ordering::SeqCst);
        let is_playing_clone = self.is_playing.clone();
        let current_frame_clone = self.current_frame.clone();
        let seek_requested_clone = self.seek_requested.clone();
        let loop_range_clone = self.loop_range.clone();
        let max_frame_clone = self.max_frame.clone();
        let fps_val = *self.fps.lock().await;

        let on_frame_update_clone = self.on_frame_update.clone();

        let mut handle_guard = self.task_handle.lock().await;
        if let Some(old_handle) = handle_guard.take() {
            old_handle.abort();
        }

        let join_handle = tokio::spawn(async move {
            let mut start_instant = std::time::Instant::now();
            let mut start_frame = current_frame_clone.load(Ordering::SeqCst);
            let mut frame_count: u64 = 0;

            while is_playing_clone.load(Ordering::SeqCst) {
                if seek_requested_clone.swap(false, Ordering::SeqCst) {
                    start_frame = current_frame_clone.load(Ordering::SeqCst);
                    start_instant = std::time::Instant::now();
                    frame_count = 0;
                }

                frame_count += 1;
                let target_time = start_instant + Duration::from_secs_f64(frame_count as f64 / fps_val);
                let now = std::time::Instant::now();
                if target_time > now {
                    tokio::time::sleep(target_time - now).await;
                }

                if !is_playing_clone.load(Ordering::SeqCst) {
                    break;
                }

                if seek_requested_clone.swap(false, Ordering::SeqCst) {
                    start_frame = current_frame_clone.load(Ordering::SeqCst);
                    start_instant = std::time::Instant::now();
                    frame_count = 0;
                    continue;
                }

                let elapsed = start_instant.elapsed().as_secs_f64();
                let mut actual_frame = start_frame + (elapsed * fps_val).round() as u64;

                let loop_opt = *loop_range_clone.lock().await;
                if let Some((loop_in, loop_out)) = loop_opt {
                    if actual_frame >= loop_out || actual_frame < loop_in {
                        actual_frame = loop_in;
                        current_frame_clone.store(loop_in, Ordering::SeqCst);
                        start_frame = loop_in;
                        start_instant = std::time::Instant::now();
                        frame_count = 0;
                    } else {
                        current_frame_clone.store(actual_frame, Ordering::SeqCst);
                    }
                } else {
                    let max_f = max_frame_clone.load(Ordering::SeqCst);
                    if max_f > 0 && actual_frame >= max_f {
                        current_frame_clone.store(max_f, Ordering::SeqCst);
                        is_playing_clone.store(false, Ordering::SeqCst);
                        
                        let guard = on_frame_update_clone.lock().await;
                        if let Some(cb) = &*guard {
                            cb(PlaybackTickEvent { frame: max_f, is_playing: false });
                        }
                        
                        info!("PlaybackEngine: Fim das mídias atingido (frame {}). Reprodução parada por padrão.", max_f);
                        break;
                    }
                    current_frame_clone.store(actual_frame, Ordering::SeqCst);
                }

                let guard = on_frame_update_clone.lock().await;
                if let Some(cb) = &*guard {
                    cb(PlaybackTickEvent { frame: actual_frame, is_playing: true });
                }
            }
        });

        *handle_guard = Some(join_handle);
        info!("PlaybackEngine: Reprodução iniciada @ {:.2} FPS", fps_val);
    }

    pub async fn pause(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.seek_requested.store(false, Ordering::SeqCst);

        let mut handle_guard = self.task_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }

        let frame = self.current_frame.load(Ordering::SeqCst);
        self.emit_tick(frame, false).await;
        info!("PlaybackEngine: Pausado no frame {}", frame);
    }

    pub async fn toggle(&self) {
        if self.is_playing.load(Ordering::SeqCst) {
            self.pause().await;
        } else {
            self.play().await;
        }
    }

    pub async fn seek(&self, mut target_frame: u64) {
        {
            let guard = self.loop_range.lock().await;
            if let Some((loop_in, loop_out)) = *guard {
                target_frame = target_frame.clamp(loop_in, loop_out);
            }
        }
        self.current_frame.store(target_frame, Ordering::SeqCst);
        self.seek_requested.store(true, Ordering::SeqCst);
        let is_play = self.is_playing.load(Ordering::SeqCst);

        self.emit_tick(target_frame, is_play).await;
    }

    pub async fn step_frame(&self, direction: i64) {
        if self.is_playing.load(Ordering::Relaxed) {
            self.pause().await;
        }

        let curr = self.current_frame.load(Ordering::Relaxed) as i64;
        let target = (curr + direction).max(0) as u64;
        self.seek(target).await;
    }
}